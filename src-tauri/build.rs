use std::fs;

const EMBED_KEYS: [&str; 5] = [
    "ACTIVATION_MASTER_KEY",
    "ACTIVATION_REMOTE_TOKEN",
    "ACTIVATION_REMOTE_OWNER",
    "ACTIVATION_REMOTE_REPO",
    "ACTIVATION_REMOTE_TAG",
];

fn main() {
    println!("cargo:rerun-if-changed=.env");
    embed_env_from_dotenv(".env");
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
