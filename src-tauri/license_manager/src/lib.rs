use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::distr::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const NONCE_LEN: usize = 12;

#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("invalid encryption key length, expected 32 bytes")]
    InvalidKeyLength,
    #[error("failed to decode key material: {0}")]
    KeyDecode(String),
    #[error("encryption failure")]
    Encrypt,
    #[error("decryption failure")]
    Decrypt,
    #[error("storage IO error: {0}")]
    Io(String),
    #[error("serialization error: {0}")]
    Serde(String),
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid license payload: {0}")]
    InvalidPayload(String),
}

impl From<std::io::Error> for LicenseError {
    fn from(value: std::io::Error) -> Self {
        LicenseError::Io(value.to_string())
    }
}

impl From<serde_json::Error> for LicenseError {
    fn from(value: serde_json::Error) -> Self {
        LicenseError::Serde(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct EncryptedPayload {
    pub content: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ActivationCodeBook {
    pub codes: Vec<String>,
}

impl ActivationCodeBook {
    pub fn contains(&self, code: &str) -> bool {
        self.codes.iter().any(|stored| stored == code)
    }

    pub fn remove(&mut self, code: &str) -> bool {
        if let Some(pos) = self.codes.iter().position(|stored| stored == code) {
            self.codes.remove(pos);
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct LicenseManager {
    cipher: Aes256Gcm,
}

#[derive(Clone)]
pub struct ActivationRepository {
    storage_path: PathBuf,
    manager: LicenseManager,
}

#[derive(Debug)]
pub struct BootstrapArtifacts {
    pub plaintext_codes_path: PathBuf,
    pub encrypted_store_path: PathBuf,
    pub encrypted_codes_path: PathBuf,
}

#[derive(Debug)]
pub enum VerificationResult {
    Success,
    AlreadyUsed,
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseClaims {
    pub license_id: String,
    pub machine_id: String,
    pub customer: Option<String>,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub features: Vec<String>,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedLicense {
    pub payload: LicenseClaims,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationList {
    pub version: u64,
    pub generated_at: u64,
    #[serde(default)]
    pub revoked: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRevocationList {
    pub payload: RevocationList,
    pub signature: String,
}

impl LicenseManager {
    pub fn from_key_str(key: &str) -> Result<Self, LicenseError> {
        let key_bytes = decode_key_material(key)?;
        let Ok(cipher) = Aes256Gcm::new_from_slice(&key_bytes) else {
            return Err(LicenseError::InvalidKeyLength);
        };
        Ok(Self { cipher })
    }

    pub fn encrypt_bytes(&self, data: &[u8]) -> Result<Vec<u8>, LicenseError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let cipher_text = self
            .cipher
            .encrypt(&nonce, data)
            .map_err(|_| LicenseError::Encrypt)?;
        let mut combined = Vec::with_capacity(NONCE_LEN + cipher_text.len());
        combined.extend_from_slice(&nonce);
        combined.extend_from_slice(&cipher_text);
        Ok(combined)
    }

    pub fn decrypt_bytes(&self, payload: &[u8]) -> Result<Vec<u8>, LicenseError> {
        if payload.len() < NONCE_LEN {
            return Err(LicenseError::Decrypt);
        }
        let (nonce_bytes, cipher_text) = payload.split_at(NONCE_LEN);
        let nonce_array: [u8; NONCE_LEN] =
            nonce_bytes.try_into().map_err(|_| LicenseError::Decrypt)?;
        let nonce = Nonce::from(nonce_array);
        let plain = self
            .cipher
            .decrypt(&nonce, cipher_text)
            .map_err(|_| LicenseError::Decrypt)?;
        Ok(plain)
    }

    pub fn encrypt_to_base64(&self, data: &[u8]) -> Result<String, LicenseError> {
        Ok(BASE64_ENGINE.encode(self.encrypt_bytes(data)?))
    }

    pub fn decrypt_from_base64(&self, encoded: &str) -> Result<Vec<u8>, LicenseError> {
        let payload = BASE64_ENGINE
            .decode(encoded.as_bytes())
            .map_err(|err| LicenseError::Io(err.to_string()))?;
        self.decrypt_bytes(&payload)
    }

    pub fn encrypt_code(&self, code: &str) -> Result<String, LicenseError> {
        self.encrypt_to_base64(code.as_bytes())
    }

    pub fn decrypt_code(&self, encrypted: &str) -> Result<String, LicenseError> {
        let bytes = self.decrypt_from_base64(encrypted)?;
        String::from_utf8(bytes).map_err(|err| LicenseError::Io(err.to_string()))
    }
}

impl ActivationRepository {
    pub fn new<P: Into<PathBuf>>(path: P, key: &str) -> Result<Self, LicenseError> {
        let manager = LicenseManager::from_key_str(key)?;
        Ok(Self {
            storage_path: path.into(),
            manager,
        })
    }

    pub fn manager(&self) -> &LicenseManager {
        &self.manager
    }

    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }

    pub fn load(&self) -> Result<ActivationCodeBook, LicenseError> {
        if !self.storage_path.exists() {
            return Ok(ActivationCodeBook::default());
        }

        let encrypted = fs::read_to_string(&self.storage_path)?;
        if encrypted.trim().is_empty() {
            return Ok(ActivationCodeBook::default());
        }
        let raw = self.manager.decrypt_from_base64(encrypted.trim())?;
        Ok(serde_json::from_slice(&raw)?)
    }

    pub fn save(&self, book: &ActivationCodeBook) -> Result<(), LicenseError> {
        let json = serde_json::to_vec(book)?;
        let encoded = self.manager.encrypt_to_base64(&json)?;
        let mut file = File::create(&self.storage_path)?;
        file.write_all(encoded.as_bytes())?;
        Ok(())
    }

    pub fn verify_and_consume(
        &self,
        encrypted_code: &str,
    ) -> Result<VerificationResult, LicenseError> {
        let decrypted = self.manager.decrypt_code(encrypted_code)?;
        let mut book = self.load()?;
        if !book.contains(&decrypted) {
            return Ok(VerificationResult::NotFound);
        }
        if !book.remove(&decrypted) {
            return Ok(VerificationResult::AlreadyUsed);
        }
        self.save(&book)?;
        Ok(VerificationResult::Success)
    }
}

pub fn generate_unique_codes(count: usize, length: usize) -> Vec<String> {
    let mut codes = HashSet::with_capacity(count);
    let mut rng = rand::rng();
    while codes.len() < count {
        let candidate: String = (0..length)
            .map(|_| rng.sample(Alphanumeric) as char)
            .map(|c| match c {
                '-' | '_' => 'X',
                other => other.to_ascii_uppercase(),
            })
            .collect();
        codes.insert(candidate);
    }
    codes.into_iter().collect()
}

pub fn bootstrap_activation_storage<P: AsRef<Path>>(
    directory: P,
    key: &str,
    count: usize,
    length: usize,
) -> Result<BootstrapArtifacts, LicenseError> {
    let dir = directory.as_ref();
    fs::create_dir_all(dir)?;

    let plaintext_path = dir.join("activation_codes.json");
    let encrypted_store_path = dir.join("activation_codes.enc");
    let client_codes_path = dir.join("activation_codes_client.txt");

    let codes = generate_unique_codes(count, length);
    let book = ActivationCodeBook {
        codes: codes.clone(),
    };

    let mut plain_file = File::create(&plaintext_path)?;
    plain_file.write_all(serde_json::to_string_pretty(&book)?.as_bytes())?;

    let manager = LicenseManager::from_key_str(key)?;
    let encoded_payload = manager.encrypt_to_base64(serde_json::to_vec(&book)?.as_slice())?;
    let mut encrypted_file = File::create(&encrypted_store_path)?;
    encrypted_file.write_all(encoded_payload.as_bytes())?;

    let mut client_file = File::create(&client_codes_path)?;
    for code in codes {
        let encrypted_code = manager.encrypt_code(&code)?;
        writeln!(client_file, "{encrypted_code}")?;
    }

    Ok(BootstrapArtifacts {
        plaintext_codes_path: plaintext_path,
        encrypted_store_path,
        encrypted_codes_path: client_codes_path,
    })
}

pub fn generate_signing_keypair() -> (String, String) {
    let seed = random_32_bytes();
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    (
        BASE64_ENGINE.encode(signing_key.to_bytes()),
        BASE64_ENGINE.encode(verifying_key.to_bytes()),
    )
}

pub fn create_machine_id(raw_machine_signature: &str) -> String {
    let digest = Sha256::digest(raw_machine_signature.as_bytes());
    hex::encode(digest)
}

pub fn sign_license(private_key: &str, claims: LicenseClaims) -> Result<String, LicenseError> {
    validate_claims(&claims)?;
    let signing_key = signing_key_from_str(private_key)?;
    let signature = sign_payload(&signing_key, &claims)?;
    let signed = SignedLicense {
        payload: claims,
        signature,
    };
    Ok(serde_json::to_string_pretty(&signed)?)
}

pub fn verify_signed_license(public_key: &str, signed: &str) -> Result<LicenseClaims, LicenseError> {
    let verifying_key = verifying_key_from_str(public_key)?;
    let parsed: SignedLicense = serde_json::from_str(signed)?;
    validate_claims(&parsed.payload)?;
    verify_payload(&verifying_key, &parsed.payload, &parsed.signature)?;
    Ok(parsed.payload)
}

pub fn sign_revocation_list(
    private_key: &str,
    revocations: RevocationList,
) -> Result<String, LicenseError> {
    let signing_key = signing_key_from_str(private_key)?;
    let signature = sign_payload(&signing_key, &revocations)?;
    let signed = SignedRevocationList {
        payload: revocations,
        signature,
    };
    Ok(serde_json::to_string_pretty(&signed)?)
}

pub fn verify_signed_revocation_list(
    public_key: &str,
    signed: &str,
) -> Result<RevocationList, LicenseError> {
    let verifying_key = verifying_key_from_str(public_key)?;
    let parsed: SignedRevocationList = serde_json::from_str(signed)?;
    verify_payload(&verifying_key, &parsed.payload, &parsed.signature)?;
    Ok(parsed.payload)
}

pub fn new_license_claims(
    license_id: String,
    machine_id: String,
    customer: Option<String>,
    expires_at: Option<u64>,
    features: Vec<String>,
) -> LicenseClaims {
    LicenseClaims {
        license_id,
        machine_id,
        customer,
        issued_at: now_unix_seconds(),
        expires_at,
        features,
        version: 1,
    }
}

pub fn new_revocation_list(revoked: Vec<String>, version: u64) -> RevocationList {
    RevocationList {
        version,
        generated_at: now_unix_seconds(),
        revoked,
    }
}

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sign_payload<T: Serialize>(signing_key: &SigningKey, payload: &T) -> Result<String, LicenseError> {
    let bytes = serde_json::to_vec(payload)?;
    let signature = signing_key.sign(&bytes);
    Ok(BASE64_ENGINE.encode(signature.to_bytes()))
}

fn verify_payload<T: Serialize>(
    verifying_key: &VerifyingKey,
    payload: &T,
    signature: &str,
) -> Result<(), LicenseError> {
    let bytes = serde_json::to_vec(payload)?;
    let signature_bytes = BASE64_ENGINE
        .decode(signature.as_bytes())
        .map_err(|err| LicenseError::KeyDecode(err.to_string()))?;
    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| LicenseError::InvalidSignature)?;
    let signature = Signature::from_bytes(&signature_array);
    verifying_key
        .verify(&bytes, &signature)
        .map_err(|_| LicenseError::InvalidSignature)
}

fn validate_claims(claims: &LicenseClaims) -> Result<(), LicenseError> {
    if claims.license_id.trim().is_empty() {
        return Err(LicenseError::InvalidPayload("license_id is required".into()));
    }
    if claims.machine_id.trim().is_empty() {
        return Err(LicenseError::InvalidPayload("machine_id is required".into()));
    }
    if let Some(expires_at) = claims.expires_at {
        if expires_at < claims.issued_at {
            return Err(LicenseError::InvalidPayload(
                "expires_at must be greater than issued_at".into(),
            ));
        }
    }
    Ok(())
}

fn signing_key_from_str(input: &str) -> Result<SigningKey, LicenseError> {
    let key_bytes = decode_key_material(input)?;
    Ok(SigningKey::from_bytes(&key_bytes))
}

fn verifying_key_from_str(input: &str) -> Result<VerifyingKey, LicenseError> {
    let key_bytes = decode_key_material(input)?;
    VerifyingKey::from_bytes(&key_bytes).map_err(|_| LicenseError::InvalidKeyLength)
}

fn random_32_bytes() -> [u8; 32] {
    let mut rng = rand::rng();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);
    bytes
}

fn decode_key_material(input: &str) -> Result<[u8; 32], LicenseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(LicenseError::InvalidKeyLength);
    }

    let looks_like_hex =
        trimmed.len().is_multiple_of(2) && trimmed.chars().all(|c| c.is_ascii_hexdigit());

    if looks_like_hex {
        if let Ok(bytes) = hex::decode(trimmed) {
            if let Ok(array) = slice_to_array(bytes) {
                return Ok(array);
            }
        }
    }

    if let Ok(bytes) = BASE64_ENGINE.decode(trimmed.as_bytes()) {
        if let Ok(array) = slice_to_array(bytes) {
            return Ok(array);
        }
    }

    if trimmed.len() == 32 {
        let bytes = trimmed.as_bytes();
        if let Ok(array) = slice_to_array(bytes.to_vec()) {
            return Ok(array);
        }
    }

    Err(LicenseError::KeyDecode(
        "unsupported key encoding".to_string(),
    ))
}

fn slice_to_array(bytes: Vec<u8>) -> Result<[u8; 32], LicenseError> {
    if bytes.len() != 32 {
        return Err(LicenseError::InvalidKeyLength);
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}
