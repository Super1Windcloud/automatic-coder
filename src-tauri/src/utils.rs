use crate::config::PreferencesConfig;
use confy::load as load_config;
use std::env;
use std::fs::{self, OpenOptions};
use tauri::{AppHandle, Manager};

#[macro_export]
macro_rules! app_info {
    ($scope:expr, $($arg:tt)*) => {
        tauri_plugin_log::log::info!("[{}] {}", $scope, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! app_warn {
    ($scope:expr, $($arg:tt)*) => {
        tauri_plugin_log::log::warn!("[{}] {}", $scope, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! app_error {
    ($scope:expr, $($arg:tt)*) => {
        tauri_plugin_log::log::error!("[{}] {}", $scope, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! app_debug {
    ($scope:expr, $($arg:tt)*) => {
        tauri_plugin_log::log::debug!("[{}] {}", $scope, format!($($arg)*))
    };
}

pub fn get_env_key(key_name: &str) -> String {
    if let Ok(value) = env::var(key_name) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    if let Some(value) = get_embedded_env_key(key_name) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let result: PreferencesConfig = load_config("interview-coder-config", "preferences").unwrap();

    if result.vlm_key.is_empty() {
        app_warn!("utils", "环境变量 {} 未设置，请设置后重试", key_name);
        String::new()
    } else {
        #[cfg(target_os = "macos")]
        app_info!("utils", "环境变量 {} 已设置", key_name);
        result.vlm_key.to_string()
    }
}

fn get_embedded_env_key(key_name: &str) -> Option<&'static str> {
    match key_name {
        "SiliconflowVLM" => option_env!("SiliconflowVLM"),
        _ => None,
    }
}

pub fn get_custom_openai_config() -> (bool, String, String, String) {
    let result: PreferencesConfig = load_config("interview-coder-config", "preferences").unwrap();
    (
        result.custom_openai_enabled,
        result.custom_openai_api_key,
        result.custom_openai_base_url,
        result.custom_openai_model,
    )
}

pub fn toggle_webview_devtools(app: &AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    if !window.is_devtools_open() {
        window.open_devtools();
    } else {
        window.close_devtools();
    }
}

pub fn is_dev() -> bool {
    cfg!(debug_assertions)
}

pub fn clear_app_log(app: &AppHandle) -> Result<(), String> {
    let log_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|err| format!("failed to resolve app local data dir: {err}"))?
        .join("logs");
    fs::create_dir_all(&log_dir).map_err(|err| format!("failed to prepare log dir: {err}"))?;

    let log_path = log_dir.join("Interview-Coder.log");
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(|err| format!("failed to clear log file {}: {err}", log_path.display()))?;
    Ok(())
}

#[test]
fn test_get_env_key() {
    app_info!("utils", "is_dev: {:?}", is_dev());
}
