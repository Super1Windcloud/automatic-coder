#![allow(clippy::let_and_return)]

use std::sync::Mutex;
use tauri::{App, AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::utils::is_dev;
use crate::{app_info, app_warn};
use confy::load as load_config;
use serde::{Deserialize, Serialize};

pub const DEFAULT_VLM_MODEL: &str = "zai-org/GLM-4.5V";
pub fn alternate_vlm_model(current: &str) -> &'static str {
    let _ = current;
    DEFAULT_VLM_MODEL
}

fn sanitize_vlm_model(model: &str) -> String {
    const ALLOWED_VLM_MODELS: &[&str] = &[
        DEFAULT_VLM_MODEL,
        "Qwen/Qwen3-VL-235B-A22B-Instruct",
        "Qwen/Qwen3.5-122B-A10B",
        "Qwen/Qwen3.5-397B-A17B",
        "Pro/moonshotai/Kimi-K2.5",
    ];

    if ALLOWED_VLM_MODELS.contains(&model) {
        model.to_string()
    } else {
        DEFAULT_VLM_MODEL.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct PreferencesConfig {
    direction_enum: DirectionEnum,
    code_language: String,
    prompt: String,
    pub vlm_key: String,
    pub vlm_model: String,
}

impl Default for PreferencesConfig {
    fn default() -> Self {
        Self {
            direction_enum: DirectionEnum::default(),
            code_language: "TypeScript".into(),
            prompt: String::new(),
            vlm_key: String::new(),
            vlm_model: DEFAULT_VLM_MODEL.into(),
        }
    }
}

#[derive(Default, Debug)]
pub struct AppState {
    pub(crate) prompt: Mutex<String>,
    pub(crate) capture_position: Mutex<DirectionEnum>,
    pub(crate) vlm_model: Mutex<String>,
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
        {
            let mut model_guard = state.vlm_model.lock().unwrap();
            *model_guard = sanitize_vlm_model(&preferences.vlm_model);
        }
        if preferences.prompt.is_empty() {
            open_language_selector(app.handle())
        } else {
            *state.prompt.lock().unwrap() = preferences.prompt;
        }
    } else {
        app_warn!("config", "failed to load preferences");
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
pub fn set_vlm_model(state: State<AppState>, model: String) {
    if model.is_empty() {
        return;
    }
    let model = sanitize_vlm_model(&model);
    {
        *state.vlm_model.lock().unwrap() = model.clone();
    }
    persist_vlm_model_value(&model).unwrap();
}

pub fn persist_vlm_model(app_handle: &AppHandle, model: &str) -> Result<(), String> {
    let model = sanitize_vlm_model(model);
    let state: State<AppState> = app_handle.state();
    *state
        .vlm_model
        .lock()
        .map_err(|_| "模型状态锁获取失败".to_string())? = model.clone();
    persist_vlm_model_value(&model)
}

fn persist_vlm_model_value(model: &str) -> Result<(), String> {
    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences")
        .map_err(|err| format!("加载配置失败: {err}"))?;
    cfg.vlm_model = sanitize_vlm_model(model);
    confy::store("interview-coder-config", "preferences", cfg)
        .map_err(|err| format!("保存配置失败: {err}"))
}

pub fn toggle_vlm_model(app_handle: &AppHandle) -> Result<String, String> {
    let state: State<AppState> = app_handle.state();
    let mut guard = state
        .vlm_model
        .lock()
        .map_err(|_| "模型状态锁获取失败".to_string())?;
    let target = alternate_vlm_model(guard.as_str());
    *guard = target.to_string();
    persist_vlm_model_value(target)?;

    Ok(target.to_string())
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
    app_info!(
        "config",
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
    app_info!("config", "current prompt updated");
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
        app_handle
            .get_webview_window("code_language_selector")
            .unwrap()
            .set_focus()
            .unwrap();

        return;
    }

    let url = if is_dev() {
        WebviewUrl::App("select.html".into())
    } else {
        WebviewUrl::App("select/select.html".into())
    };

    let webview_window = WebviewWindowBuilder::new(app_handle, "code_language_selector", url)
        .inner_size(600.0, 800.0)
        .build()
        .unwrap();

    webview_window.center().unwrap();
    webview_window.set_focus().unwrap();
    webview_window.set_content_protected(true).unwrap();
    webview_window.set_decorations(false).unwrap();
    webview_window.set_skip_taskbar(true).unwrap();
    webview_window.set_enabled(true).unwrap();
    webview_window.set_always_on_top(false).unwrap();
    webview_window.show().unwrap();
}

#[test]
fn output_config_path() {
    use confy::get_configuration_file_path;

    let result = get_configuration_file_path("interview-coder-config", "preferences").unwrap();
    app_info!("config", "config path: {}", result.to_str().unwrap());
}
