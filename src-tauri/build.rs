use dirs::home_dir;
use sha2::{Digest, Sha256};
use std::{env, fs};

const EMBED_KEYS: [&str; 9] = [
    "ACTIVATION_MASTER_KEY",
    "ACTIVATION_REMOTE_TOKEN",
    "ACTIVATION_REMOTE_OWNER",
    "ACTIVATION_REMOTE_REPO",
    "ACTIVATION_REMOTE_TAG",
    "CAPTURE_SKIP_SAVE",
    "LICENSE_PRIVATE_KEY",
    "LICENSE_PUBLIC_KEY",
    "LICENSE_REVOCATION_URL",
];

fn main() {
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-env-changed=INTERVIEW_CODER_HOST_MACHINE_ID");
    println!("cargo:rerun-if-env-changed=COMPUTERNAME");
    println!("cargo:rerun-if-env-changed=HOSTNAME");
    println!("cargo:rerun-if-env-changed=PROCESSOR_IDENTIFIER");
    println!("cargo:rerun-if-env-changed=HOME");
    println!("cargo:rerun-if-env-changed=USERPROFILE");
    embed_env_from_dotenv(".env");
    embed_host_machine_id();
    tauri_build::build();
}

fn embed_env_from_dotenv(path: &str) {
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !EMBED_KEYS.contains(&key) {
            continue;
        }
        let value = value.trim().trim_matches('"');
        println!("cargo:rustc-env={}={}", key, value);
    }
}

fn embed_host_machine_id() {
    let machine_id = env::var("INTERVIEW_CODER_HOST_MACHINE_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(compute_machine_id);
    let machine_id = machine_id.trim();
    if machine_id.is_empty() {
        return;
    }
    println!("cargo:rustc-env=HOST_MACHINE_ID={}", machine_id);
}

fn compute_machine_id() -> String {
    let signature = collect_machine_signature();
    let digest = Sha256::digest(signature.as_bytes());
    hex::encode(digest)
}

fn collect_machine_signature() -> String {
    let mut parts = Vec::new();

    let hostname = whoami::hostname().unwrap_or_default();
    if !hostname.is_empty() {
        parts.push(hostname);
    }

    let username = whoami::username().unwrap_or_default();
    if !username.is_empty() {
        parts.push(username);
    }

    let platform = whoami::platform().to_string();
    if !platform.is_empty() {
        parts.push(platform);
    }

    let arch = whoami::cpu_arch().to_string();
    if !arch.is_empty() {
        parts.push(arch);
    }

    let distro = whoami::distro().unwrap_or_default();
    if !distro.is_empty() {
        parts.push(distro);
    }

    if let Some(home) = home_dir()
        && !home.as_os_str().is_empty()
    {
        parts.push(home.display().to_string());
    }

    if let Ok(machine) = env::var("COMPUTERNAME").or_else(|_| env::var("HOSTNAME"))
        && !machine.is_empty()
    {
        parts.push(machine);
    }

    if let Ok(identifier) = env::var("PROCESSOR_IDENTIFIER")
        && !identifier.is_empty()
    {
        parts.push(identifier);
    }

    let mut hasher = Sha256::new();
    hasher.update(parts.join("|").as_bytes());
    hex::encode(hasher.finalize())
}
