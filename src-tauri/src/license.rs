use dirs::home_dir;
use dotenv::{dotenv, from_filename};
use license_manager::{ActivationRepository, LicenseError, VerificationResult};
use reqwest::{Client, multipart};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::{self, Write};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{env, fs};
use tauri::{
    AppHandle, Emitter, Manager, Position, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tempfile::NamedTempFile;

use crate::{
    system::register_activation_shortcut,
    utils::{is_dev, write_some_log},
};
const REMOTE_ACTIVATION_FILE: &str = "activation_codes.enc";
const ACTIVATION_STATUS_FILE: [&str; 2] = [
    "activation_status_fingerprint",
    "cd0621ec3d0ffce82ce0a435ebf5bf25caae51fa4c0d7ba055869b59a93b6585",
];

pub struct LicenseState {
    inner: Mutex<Option<LicenseInner>>,
    enabled: AtomicBool,
}

impl Default for LicenseState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
            enabled: AtomicBool::new(false),
        }
    }
}

struct LicenseInner {
    activation_key: String,
    remote: RemoteActivationConfig,
    status_roots: Vec<PathBuf>,
    activated: bool,
    client: Client,
}

#[derive(Debug, Clone, Default)]
pub struct ActivationStatus {
    pub activated: bool,
}

impl LicenseState {
    pub fn initialize(
        &self,
        bootstrap: ActivationBootstrap,
        status_roots: Vec<PathBuf>,
        activated: bool,
    ) {
        let mut guard = self.inner.lock().unwrap();
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("InterviewCoder/activation-refresh")
            .build()
            .expect("failed to construct activation HTTP client");
        *guard = Some(LicenseInner {
            activation_key: bootstrap.activation_key,
            remote: bootstrap.remote,
            status_roots,
            activated,
            client,
        });
        self.enabled.store(true, Ordering::SeqCst);
    }

    pub fn is_ready(&self) -> bool {
        self.inner.lock().unwrap().is_some()
    }

    pub fn disable(&self) {
        self.enabled.store(false, Ordering::SeqCst);
        *self.inner.lock().unwrap() = None;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn is_activated(&self) -> Result<bool, LicenseError> {
        let guard = self.inner.lock().unwrap();
        let Some(inner) = guard.as_ref() else {
            return Ok(false);
        };
        Ok(inner.activated)
    }

    pub async fn verify_and_consume(
        &self,
        encrypted_code: &str,
    ) -> Result<VerificationResult, ActivationFlowError> {
        let (status_roots, activation_key, remote, client) = {
            let mut guard = self.inner.lock().unwrap();
            let Some(inner) = guard.as_mut() else {
                return Err(ActivationFlowError::Disabled(
                    "license system has not been initialised".into(),
                ));
            };
            if inner.activated {
                return Ok(VerificationResult::Success);
            }
            (
                inner.status_roots.clone(),
                inner.activation_key.clone(),
                inner.remote.clone(),
                inner.client.clone(),
            )
        };

        let locker = RemoteActivationLock::acquire(remote, client).await?;
        let repository = ActivationRepository::new(locker.path(), &activation_key)?;
        let verification = repository.verify_and_consume(encrypted_code)?;

        if matches!(verification, VerificationResult::Success) {
            locker.finalize_success().await?;
            persist_status(&status_roots, encrypted_code)?;
            if let Some(inner) = self.inner.lock().unwrap().as_mut() {
                inner.activated = true;
            }
        }

        Ok(verification)
    }
}

#[derive(Debug, Serialize)]
pub struct ActivationAttemptPayload {
    pub success: bool,
    pub status: String,
    pub activated: bool,
}
#[derive(Clone)]
pub struct ActivationBootstrap {
    pub activation_key: String,
    pub remote: RemoteActivationConfig,
}

#[derive(Clone)]
pub struct RemoteActivationConfig {
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub token: String,
}

#[tauri::command]
pub fn get_activation_status(state: State<LicenseState>) -> Result<bool, String> {
    if !state.is_enabled() {
        return Ok(true);
    }
    if !state.is_ready() {
        return Ok(false);
    }
    state.is_activated().map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn submit_activation_code(
    app: AppHandle,
    state: State<'_, LicenseState>,
    encrypted_code: String,
) -> Result<ActivationAttemptPayload, String> {
    if encrypted_code.trim().is_empty() {
        return Ok(ActivationAttemptPayload {
            success: false,
            status: "empty_code".into(),
            activated: false,
        });
    }
    if !state.is_enabled() {
        return Ok(ActivationAttemptPayload {
            success: true,
            status: "disabled".into(),
            activated: true,
        });
    }
    if !state.is_ready() {
        return Ok(ActivationAttemptPayload {
            success: false,
            status: "pending_initialisation".into(),
            activated: false,
        });
    }

    match state.verify_and_consume(encrypted_code.trim()).await {
        Ok(VerificationResult::Success) => {
            register_activation_shortcut(&app);
            if let Some(window) = app.get_webview_window("activation_gate") {
                let _ = window.hide();
                let _ = window.close();
            }
            if let Some(main) = app.get_webview_window("main") {
                reveal_main_window(main);
            }
            let _ = app.emit("activation_granted", true);
            Ok(ActivationAttemptPayload {
                success: true,
                status: "success".into(),
                activated: true,
            })
        }
        Ok(VerificationResult::AlreadyUsed) => Ok(ActivationAttemptPayload {
            success: false,
            status: "already_used".into(),
            activated: false,
        }),
        Ok(VerificationResult::NotFound) => Ok(ActivationAttemptPayload {
            success: false,
            status: "not_found".into(),
            activated: false,
        }),
        Err(err) => Err(err.to_string()),
    }
}

pub fn prepare_activation_repository(
    _app_handle: &AppHandle,
) -> Result<Option<ActivationBootstrap>, LicenseError> {
    hydrate_activation_env();
    let Some(key) = env::var("ACTIVATION_MASTER_KEY").ok() else {
        println!("activation system disabled: missing ACTIVATION_MASTER_KEY");
        return Ok(None);
    };

    if key.trim().is_empty() {
        println!("activation system disabled: empty ACTIVATION_MASTER_KEY");
        return Ok(None);
    }

    let owner = env::var("ACTIVATION_REMOTE_OWNER")
        .or_else(|_| env::var("GITHUB_OWNER"))
        .unwrap_or_else(|_| "Super1Windcloud".to_string());
    let repo = env::var("ACTIVATION_REMOTE_REPO")
        .or_else(|_| env::var("GITHUB_REPO"))
        .unwrap_or_else(|_| "automatic-coder".to_string());
    let tag = env::var("ACTIVATION_REMOTE_TAG")
        .or_else(|_| env::var("GITHUB_RELEASE_TAG"))
        .unwrap_or_else(|_| "v1.0.0".to_string());
    let token = env::var("ACTIVATION_REMOTE_TOKEN")
        .or_else(|_| env::var("GITHUB_TOKEN"))
        .unwrap_or_default();

    if token.trim().is_empty() {
        println!("activation system disabled: missing GITHUB_TOKEN/ACTIVATION_REMOTE_TOKEN");
        return Ok(None);
    }

    Ok(Some(ActivationBootstrap {
        activation_key: key,
        remote: RemoteActivationConfig {
            owner,
            repo,
            tag,
            token,
        },
    }))
}

fn hydrate_activation_env() {
    let _ = from_filename("src-tauri/.env");
    let _ = dotenv();
}

pub fn open_activation_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("activation_gate") {
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = window.show();
        return;
    }

    let url = if is_dev() {
        WebviewUrl::App("activation.html".into())
    } else {
        WebviewUrl::App("activation/activation.html".into())
    };

    let window = WebviewWindowBuilder::new(app_handle, "activation_gate", url)
        .inner_size(800.0, 600.0)
        .resizable(true)
        .minimizable(false)
        .maximizable(false)
        .closable(true)
        .decorations(false)
        .visible(true)
        .skip_taskbar(false)
        .build()
        .expect("failed to create activation window");

    let _ = window.center();
    let _ = window.set_focus();
    let _ = window.set_always_on_top(false);
    let _ = window.set_content_protected(false);
}

fn reveal_main_window(window: WebviewWindow) {
    let _ = window.set_position(Position::Logical((100.0, 50.0).into()));
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.show();
}

pub fn show_main_window_now(app_handle: &AppHandle) {
    if let Some(main) = app_handle.get_webview_window("main") {
        reveal_main_window(main);
    }
}

pub fn load_activation_status(roots: &[PathBuf]) -> ActivationStatus {
    let mut first = true;
    if !roots.is_empty()
        && roots
            .iter()
            .all(|root| find_status_file(root, &mut first).is_some())
    {
        return ActivationStatus { activated: true };
    }
    ActivationStatus::default()
}

pub fn persist_status(roots: &[PathBuf], activation_code: &str) -> Result<(), LicenseError> {
    let fingerprint = derive_activation_fingerprint(activation_code);
    let mut first = true;
    for root in roots {
        let target_dir = root.join(&fingerprint);
        if !target_dir.exists() {
            fs::create_dir_all(&target_dir)?;
        }
        let path = if first {
            first = false;
            target_dir.join(ACTIVATION_STATUS_FILE[1])
        } else {
            target_dir.join(ACTIVATION_STATUS_FILE[0])
        };
        fs::write(path, &fingerprint)?;
    }
    Ok(())
}

fn derive_activation_fingerprint(activation_code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(activation_code.trim().as_bytes());
    hasher.update(collect_machine_signature().as_bytes());
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{:02x}", byte).expect("writing to String cannot fail");
    }
    output
}

fn collect_machine_signature() -> String {
    let mut parts = Vec::new();

    let hostname = whoami::fallible::hostname().unwrap();
    if !hostname.is_empty() {
        parts.push(hostname);
    }

    let username = whoami::username();
    if !username.is_empty() {
        parts.push(username);
    }

    let platform = whoami::platform().to_string();
    if !platform.is_empty() {
        parts.push(platform);
    }

    let arch = whoami::arch().to_string();
    if !arch.is_empty() {
        parts.push(arch);
    }

    let distro = whoami::distro();
    if !distro.is_empty() {
        parts.push(distro);
    }

    if let Some(home) = home_dir() {
        if !home.as_os_str().is_empty() {
            parts.push(home.display().to_string());
        }
    }

    if let Ok(machine) = env::var("COMPUTERNAME").or_else(|_| env::var("HOSTNAME")) {
        if !machine.is_empty() {
            parts.push(machine);
        }
    }

    if let Ok(identifier) = env::var("PROCESSOR_IDENTIFIER") {
        if !identifier.is_empty() {
            parts.push(identifier);
        }
    }

    parts.join("|")
}

fn find_status_file(root: &Path, first: &mut bool) -> Option<PathBuf> {
    if !root.exists() {
        println!("status root does not exist: {}", root.display());
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.len() != 64 || !name_str.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let candidate = if *first {
            *first = false;
            entry.path().join(ACTIVATION_STATUS_FILE[1])
        } else {
            entry.path().join(ACTIVATION_STATUS_FILE[0])
        };
        if candidate.exists() {
            println!("found status file: {}", candidate.display());
            return Some(candidate);
        } else {
            println!("not found status file: {}", candidate.display());
        }
    }

    None
}

#[derive(Debug)]
pub enum ActivationFlowError {
    Disabled(String),
    License(LicenseError),
    Remote(String),
}

impl fmt::Display for ActivationFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActivationFlowError::Disabled(msg) => write!(f, "{msg}"),
            ActivationFlowError::License(err) => write!(f, "{err}"),
            ActivationFlowError::Remote(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<LicenseError> for ActivationFlowError {
    fn from(value: LicenseError) -> Self {
        ActivationFlowError::License(value)
    }
}

impl From<reqwest::Error> for ActivationFlowError {
    fn from(value: reqwest::Error) -> Self {
        ActivationFlowError::Remote(value.to_string())
    }
}

impl From<std::io::Error> for ActivationFlowError {
    fn from(value: std::io::Error) -> Self {
        ActivationFlowError::Remote(value.to_string())
    }
}

const GITHUB_API_BASE: &str = "https://api.github.com";

struct RemoteActivationLock {
    config: RemoteActivationConfig,
    client: Client,
    release_id: u64,
    asset_id: u64,
    file_name: String,
    upload_url: String,
    file: NamedTempFile,
}

impl RemoteActivationLock {
    async fn acquire(
        config: RemoteActivationConfig,
        client: Client,
    ) -> Result<Self, ActivationFlowError> {
        write_some_log("acquiring remote activation payload");
        let release = fetch_release_by_tag(&client, &config).await?;
        let asset = fetch_activation_asset(&client, &config, release.id).await?;
        let payload = download_activation_payload(&client, &config, asset.id).await?;
        let mut file = NamedTempFile::new()?;
        file.write_all(&payload)?;
        Ok(Self {
            config,
            client,
            release_id: release.id,
            asset_id: asset.id,
            file_name: asset.name,
            upload_url: release.upload_url,
            file,
        })
    }

    fn path(&self) -> &Path {
        self.file.path()
    }

    async fn finalize_success(self) -> Result<(), ActivationFlowError> {
        write_some_log("refreshing remote activation payload");
        let RemoteActivationLock {
            config,
            client,
            release_id: _,
            asset_id,
            file_name,
            upload_url,
            file,
        } = self;

        delete_activation_asset(&client, &config, asset_id).await?;
        let path = file.path().to_path_buf();
        upload_activation_payload(&client, &config, &upload_url, &file_name, &path).await
    }
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    id: u64,
    upload_url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct GithubAsset {
    id: u64,
    name: String,
}

async fn fetch_release_by_tag(
    client: &Client,
    config: &RemoteActivationConfig,
) -> Result<GithubRelease, ActivationFlowError> {
    let url = format!(
        "{GITHUB_API_BASE}/repos/{}/{}/releases/tags/{}",
        config.owner, config.repo, config.tag
    );
    let response = client
        .get(url)
        .header("Authorization", format!("token {}", config.token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "InterviewCoder")
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ActivationFlowError::Remote(format!(
            "failed to fetch release ({status}): {body}"
        )));
    }
    response.json::<GithubRelease>().await.map_err(Into::into)
}

async fn fetch_activation_asset(
    client: &Client,
    config: &RemoteActivationConfig,
    release_id: u64,
) -> Result<GithubAsset, ActivationFlowError> {
    let url = format!(
        "{GITHUB_API_BASE}/repos/{}/{}/releases/{release_id}/assets",
        config.owner, config.repo
    );
    let response = client
        .get(url)
        .header("Authorization", format!("token {}", config.token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "InterviewCoder")
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ActivationFlowError::Remote(format!(
            "failed to list release assets ({status}): {body}"
        )));
    }
    let assets = response.json::<Vec<GithubAsset>>().await?;
    if assets.is_empty() {
        return Err(ActivationFlowError::Remote(
            "no activation payload available on remote release".into(),
        ));
    }

    if let Some(found) = assets
        .iter()
        .find(|asset| asset.name == REMOTE_ACTIVATION_FILE)
        .cloned()
    {
        return Ok(found);
    }

    Ok(assets[0].clone())
}

async fn download_activation_payload(
    client: &Client,
    config: &RemoteActivationConfig,
    asset_id: u64,
) -> Result<Vec<u8>, ActivationFlowError> {
    let url = format!(
        "{GITHUB_API_BASE}/repos/{}/{}/releases/assets/{asset_id}",
        config.owner, config.repo
    );
    let response = client
        .get(url)
        .header("Authorization", format!("token {}", config.token))
        .header("Accept", "application/octet-stream")
        .header("User-Agent", "InterviewCoder")
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ActivationFlowError::Remote(format!(
            "failed to download activation payload ({status}): {body}"
        )));
    }
    Ok(response.bytes().await?.to_vec())
}

async fn delete_activation_asset(
    client: &Client,
    config: &RemoteActivationConfig,
    asset_id: u64,
) -> Result<(), ActivationFlowError> {
    let url = format!(
        "{GITHUB_API_BASE}/repos/{}/{}/releases/assets/{asset_id}",
        config.owner, config.repo
    );
    let response = client
        .delete(url)
        .header("Authorization", format!("token {}", config.token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "InterviewCoder")
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ActivationFlowError::Remote(format!(
            "failed to delete activation payload ({status}): {body}"
        )));
    }
    Ok(())
}

async fn upload_activation_payload(
    client: &Client,
    config: &RemoteActivationConfig,
    upload_url: &str,
    file_name: &str,
    path: &Path,
) -> Result<(), ActivationFlowError> {
    // upload_url is usually like "https://uploads.github.com/repos/owner/repo/releases/id/assets{?name,label}"
    let base_url = upload_url.split('{').next().unwrap();
    let url = format!("{}?name={}", base_url, file_name);

    let bytes = fs::read(path)?;
    let response = client
        .post(url)
        .header("Authorization", format!("token {}", config.token))
        .header("Content-Type", "application/octet-stream")
        .header("User-Agent", "InterviewCoder")
        .body(bytes)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ActivationFlowError::Remote(format!(
            "failed to upload activation payload ({status}): {body}"
        )));
    }
    Ok(())
}
