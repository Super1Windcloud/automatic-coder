use dirs::home_dir;
use sha2::{Digest, Sha256};
use std::env;

fn main() {
    println!("{}", compute_machine_id());
}

fn compute_machine_id() -> String {
    license_manager::create_machine_id(&collect_machine_signature())
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
