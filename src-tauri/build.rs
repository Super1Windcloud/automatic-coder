use std::fs;

const EMBED_KEYS: [&str; 8] = [
    "ACTIVATION_MASTER_KEY",
    "ACTIVATION_REMOTE_TOKEN",
    "ACTIVATION_REMOTE_OWNER",
    "ACTIVATION_REMOTE_REPO",
    "ACTIVATION_REMOTE_TAG",
    "CAPTURE_SKIP_SAVE",
    "LICENSE_PUBLIC_KEY",
    "LICENSE_REVOCATION_URL",
];

fn main() {
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-env-changed=INTERVIEW_CODER_HOST_BUILD");
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
        let embed_private_key = std::env::var("INTERVIEW_CODER_HOST_BUILD")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let should_embed = EMBED_KEYS.contains(&key)
            || (embed_private_key && key == "LICENSE_PRIVATE_KEY");
        if !should_embed {
            continue;
        }
        let value = value.trim().trim_matches('"');
        println!("cargo:rustc-env={}={}", key, value);
    }
}
