#![allow(clippy::let_and_return)]

use std::sync::Mutex;
use reqwest::Url;
use tauri::{App, AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::utils::is_dev;
use crate::{app_info, app_warn};
use confy::load as load_config;
use serde::{Deserialize, Serialize};

pub const DEFAULT_VLM_MODEL: &str = "zai-org/GLM-4.5V";
pub const DEFAULT_OPENAI_COMPAT_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_OPENAI_COMPAT_MODEL: &str = "gpt-4o";

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

fn sanitize_openai_compat_model(model: &str) -> String {
    let model = model.trim();
    if model.is_empty() {
        DEFAULT_OPENAI_COMPAT_MODEL.to_string()
    } else {
        model.to_string()
    }
}

fn sanitize_openai_compat_base_url(base_url: &str) -> String {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return DEFAULT_OPENAI_COMPAT_BASE_URL.to_string();
    }

    if let Ok(parsed) = Url::parse(base_url) {
        let path = parsed.path().trim_end_matches('/');
        if path.is_empty() || path == "/" {
            return format!("{base_url}/v1");
        }
    }

    base_url.to_string()
}

#[cfg(test)]
mod tests {
    use super::sanitize_openai_compat_base_url;

    #[test]
    fn sanitize_openai_compat_base_url_defaults_when_empty() {
        assert_eq!(
            sanitize_openai_compat_base_url(""),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn sanitize_openai_compat_base_url_appends_v1_for_domain_root() {
        assert_eq!(
            sanitize_openai_compat_base_url("https://www.aizhiwen.top"),
            "https://www.aizhiwen.top/v1"
        );
    }

    #[test]
    fn sanitize_openai_compat_base_url_preserves_existing_path() {
        assert_eq!(
            sanitize_openai_compat_base_url("https://www.aizhiwen.top/v1"),
            "https://www.aizhiwen.top/v1"
        );
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct PreferencesConfig {
    direction_enum: DirectionEnum,
    code_language: String,
    prompt: String,
    pub page_opacity: f64,
    pub background_broadcast: bool,
    pub vlm_key: String,
    pub vlm_model: String,
    pub custom_openai_enabled: bool,
    pub custom_openai_api_key: String,
    pub custom_openai_base_url: String,
    pub custom_openai_model: String,
}

impl Default for PreferencesConfig {
    fn default() -> Self {
        Self {
            direction_enum: DirectionEnum::default(),
            code_language: "TypeScript".into(),
            prompt: String::new(),
            page_opacity: 1.0,
            background_broadcast: false,
            vlm_key: String::new(),
            vlm_model: DEFAULT_VLM_MODEL.into(),
            custom_openai_enabled: false,
            custom_openai_api_key: String::new(),
            custom_openai_base_url: DEFAULT_OPENAI_COMPAT_BASE_URL.into(),
            custom_openai_model: DEFAULT_OPENAI_COMPAT_MODEL.into(),
        }
    }
}

#[derive(Default, Debug)]
pub struct AppState {
    pub(crate) prompt: Mutex<String>,
    pub(crate) capture_position: Mutex<DirectionEnum>,
    pub(crate) page_opacity: Mutex<f64>,
    pub(crate) background_broadcast: Mutex<bool>,
    pub(crate) vlm_model: Mutex<String>,
    pub(crate) custom_openai_enabled: Mutex<bool>,
    pub(crate) custom_openai_base_url: Mutex<String>,
    pub(crate) custom_openai_model: Mutex<String>,
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
            let mut opacity_guard = state.page_opacity.lock().unwrap();
            *opacity_guard = sanitize_page_opacity(preferences.page_opacity);
        }
        {
            let mut model_guard = state.vlm_model.lock().unwrap();
            *model_guard = sanitize_vlm_model(&preferences.vlm_model);
        }
        {
            let mut enabled_guard = state.custom_openai_enabled.lock().unwrap();
            *enabled_guard = preferences.custom_openai_enabled;
        }
        {
            let mut base_url_guard = state.custom_openai_base_url.lock().unwrap();
            *base_url_guard =
                sanitize_openai_compat_base_url(&preferences.custom_openai_base_url);
        }
        {
            let mut model_guard = state.custom_openai_model.lock().unwrap();
            *model_guard = sanitize_openai_compat_model(&preferences.custom_openai_model);
        }
        {
            let mut background_broadcast_guard = state.background_broadcast.lock().unwrap();
            *background_broadcast_guard = preferences.background_broadcast;
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

fn sanitize_page_opacity(opacity: f64) -> f64 {
    opacity.clamp(0.2, 1.0)
}

fn persist_page_opacity_value(opacity: f64) -> Result<(), String> {
    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences")
        .map_err(|err| format!("加载配置失败: {err}"))?;
    cfg.page_opacity = sanitize_page_opacity(opacity);
    confy::store("interview-coder-config", "preferences", cfg)
        .map_err(|err| format!("保存配置失败: {err}"))
}

pub fn persist_page_opacity(app_handle: &AppHandle, opacity: f64) -> Result<f64, String> {
    let opacity = sanitize_page_opacity(opacity);
    let state: State<AppState> = app_handle.state();
    *state
        .page_opacity
        .lock()
        .map_err(|_| "透明度状态锁获取失败".to_string())? = opacity;

    persist_page_opacity_value(opacity)?;
    app_handle
        .emit("page-opacity-changed", opacity)
        .map_err(|err| format!("广播透明度变更失败: {err}"))?;

    Ok(opacity)
}

#[tauri::command]
pub fn set_page_opacity(app_handle: AppHandle, opacity: f64) -> Result<f64, String> {
    persist_page_opacity(&app_handle, opacity)
}

fn persist_background_broadcast_value(enabled: bool) -> Result<(), String> {
    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences")
        .map_err(|err| format!("加载配置失败: {err}"))?;
    cfg.background_broadcast = enabled;
    confy::store("interview-coder-config", "preferences", cfg)
        .map_err(|err| format!("保存配置失败: {err}"))
}

pub fn persist_background_broadcast(app_handle: &AppHandle, enabled: bool) -> Result<bool, String> {
    let state: State<AppState> = app_handle.state();
    *state
        .background_broadcast
        .lock()
        .map_err(|_| "后台播音状态锁获取失败".to_string())? = enabled;

    persist_background_broadcast_value(enabled)?;
    app_handle
        .emit("background-broadcast-changed", enabled)
        .map_err(|err| format!("广播后台播音变更失败: {err}"))?;

    Ok(enabled)
}

#[tauri::command]
pub fn set_background_broadcast(app_handle: AppHandle, enabled: bool) -> Result<bool, String> {
    persist_background_broadcast(&app_handle, enabled)
}

#[tauri::command]
pub fn set_vlm_key(key: String) {
    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences").unwrap();
    cfg.vlm_key = key.trim().to_string();
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
    let target = alternate_vlm_model(guard.as_str()).to_string();
    *guard = target.clone();
    persist_vlm_model_value(&target)?;

    Ok(target)
}

pub fn persist_custom_openai_enabled(app_handle: &AppHandle, enabled: bool) -> Result<bool, String> {
    let state: State<AppState> = app_handle.state();
    *state
        .custom_openai_enabled
        .lock()
        .map_err(|_| "自定义 OpenAI 开关状态锁获取失败".to_string())? = enabled;

    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences")
        .map_err(|err| format!("加载配置失败: {err}"))?;
    cfg.custom_openai_enabled = enabled;
    confy::store("interview-coder-config", "preferences", cfg)
        .map_err(|err| format!("保存配置失败: {err}"))?;

    Ok(enabled)
}

#[tauri::command]
pub fn set_custom_openai_enabled(app_handle: AppHandle, enabled: bool) -> Result<bool, String> {
    persist_custom_openai_enabled(&app_handle, enabled)
}

#[tauri::command]
pub fn save_custom_openai_config(
    app_handle: AppHandle,
    api_key: String,
    base_url: String,
    model: String,
) -> Result<(), String> {
    let normalized_base_url = sanitize_openai_compat_base_url(&base_url);
    let normalized_model = sanitize_openai_compat_model(&model);
    let state: State<AppState> = app_handle.state();
    *state
        .custom_openai_base_url
        .lock()
        .map_err(|_| "自定义 OpenAI 接口地址状态锁获取失败".to_string())? =
        normalized_base_url.clone();
    *state
        .custom_openai_model
        .lock()
        .map_err(|_| "自定义 OpenAI 模型状态锁获取失败".to_string())? = normalized_model.clone();

    let mut cfg: PreferencesConfig = confy::load("interview-coder-config", "preferences")
        .map_err(|err| format!("加载配置失败: {err}"))?;
    cfg.custom_openai_api_key = api_key.trim().to_string();
    cfg.custom_openai_base_url = normalized_base_url;
    cfg.custom_openai_model = normalized_model;
    confy::store("interview-coder-config", "preferences", cfg)
        .map_err(|err| format!("保存配置失败: {err}"))
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
    webview_window.set_always_on_top(true).unwrap();
    webview_window.show().unwrap();
}

pub fn open_custom_openai_selector(app_handle: &AppHandle) {
    if app_handle
        .get_webview_window("custom_openai_selector")
        .is_some()
    {
        app_handle
            .get_webview_window("custom_openai_selector")
            .unwrap()
            .set_always_on_top(true)
            .unwrap();
        app_handle
            .get_webview_window("custom_openai_selector")
            .unwrap()
            .set_focus()
            .unwrap();

        return;
    }

    let url = if is_dev() {
        WebviewUrl::App("openai.html".into())
    } else {
        WebviewUrl::App("openai/openai.html".into())
    };

    let webview_window = WebviewWindowBuilder::new(app_handle, "custom_openai_selector", url)
        .inner_size(600.0, 560.0)
        .build()
        .unwrap();

    webview_window.center().unwrap();
    webview_window.set_focus().unwrap();
    webview_window.set_content_protected(true).unwrap();
    webview_window.set_decorations(false).unwrap();
    webview_window.set_skip_taskbar(true).unwrap();
    webview_window.set_enabled(true).unwrap();
    webview_window.set_always_on_top(true).unwrap();
    webview_window.show().unwrap();
}

#[test]
fn output_config_path() {
    use confy::get_configuration_file_path;

    let result = get_configuration_file_path("interview-coder-config", "preferences").unwrap();
    app_info!("config", "config path: {}", result.to_str().unwrap());
}
