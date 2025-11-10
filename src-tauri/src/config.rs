use std::sync::Mutex;
use tauri::{App, AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::utils::is_dev;
use confy::load as load_config;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct PreferencesConfig {
    direction_enum: DirectionEnum,
    code_language: String,
    prompt: String,
    pub vlm_key: String,
}
#[derive(Default, Debug)]
pub struct AppState {
    pub(crate) prompt: Mutex<String>,
    pub(crate) capture_position: Mutex<DirectionEnum>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DirectionEnum {
    #[default]
    LeftHalf,
    RightHalf,
    UpHalf,
    DownHalf,
    FullScreen,
}

pub fn load_preferences(app: &mut App) {
    let result = load_config("interview-coder-config", "preferences");
    // 修改键值
    if let Ok(config) = result {
        let preferences: PreferencesConfig = config;
        let state: State<AppState> = app.state();
        *state.capture_position.lock().unwrap() = preferences.direction_enum;
        if preferences.prompt.is_empty() {
            open_language_selector(app.handle())
        } else {
            *state.prompt.lock().unwrap() = preferences.prompt;
        }
    } else {
        println!("Failed to load preferences");
        open_language_selector(app.handle())
    }
}

#[tauri::command]
pub fn get_store_config() -> String {
    let result: PreferencesConfig = load_config("interview-coder-config", "preferences").unwrap();
    let result_str = serde_json::to_string(&result).unwrap();
    result_str
}

#[tauri::command]
pub fn set_vlm_key(key: String) {
    if key.is_empty() {
        return;
    }
    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences").unwrap();
    cfg.vlm_key = key;
    confy::store("interview-coder-config", "preferences", cfg).unwrap();
}

#[tauri::command]
pub fn set_capture_position(state: State<AppState>, position: String) {
    let position = match position.as_str() {
        "lefthalf" => DirectionEnum::LeftHalf,
        "righthalf" => DirectionEnum::RightHalf,
        "uphalf" => DirectionEnum::UpHalf,
        "downhalf" => DirectionEnum::DownHalf,
        "fullscreen" => DirectionEnum::FullScreen,
        _ => DirectionEnum::LeftHalf,
    };
    *state.capture_position.lock().unwrap() = position;
    println!(
        "current capture position: {:?}",
        state.capture_position.lock().unwrap()
    );

    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences").unwrap();
    cfg.direction_enum = position;
    confy::store("interview-coder-config", "preferences", cfg).unwrap();
}

#[tauri::command]
pub fn set_selected_language_prompt(state: State<AppState>, window: tauri::Window, prompt: String) {
    *state.prompt.lock().unwrap() = prompt.clone();
    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences").unwrap();
    cfg.prompt = prompt;
    confy::store("interview-coder-config", "preferences", cfg).unwrap();
    println!("current prompt: {}", state.prompt.lock().unwrap());
    if let Some(window) = window.get_webview_window("code_language_selector") {
        window.hide().unwrap();
        window.close().unwrap();
    }
}

#[tauri::command]
pub fn set_selected_language(code_language: String) {
    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences").unwrap();
    cfg.code_language = code_language;
    confy::store("interview-coder-config", "preferences", cfg).unwrap();
}

pub fn open_language_selector(app_handle: &AppHandle) {
    if app_handle
        .get_webview_window("code_language_selector")
        .is_some()
    {
        app_handle
            .get_webview_window("code_language_selector")
            .unwrap()
            .set_always_on_top(true)
            .unwrap();
        return;
    }

    let url = if is_dev() {
        WebviewUrl::App("select.html".into())
    } else {
        WebviewUrl::App("select/select.html".into())
    };

    let webview_window = WebviewWindowBuilder::new(app_handle, "code_language_selector", url)
        .inner_size(800.0, 600.0)
        .build()
        .unwrap();

    webview_window.center().unwrap();
    webview_window.set_focus().unwrap();
    webview_window.set_content_protected(true).unwrap();
    webview_window.set_decorations(false).unwrap();
    webview_window.set_skip_taskbar(true).unwrap();
    webview_window.set_enabled(true).unwrap();
    webview_window.show().unwrap();
}

#[test]
fn output_config_path() {
    use confy::get_configuration_file_path;

    let result = get_configuration_file_path("interview-coder-config", "preferences").unwrap();
    println!("Config path: {}", result.to_str().unwrap());
}
