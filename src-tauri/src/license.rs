use license_manager::{ActivationRepository, LicenseError, VerificationResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::{env, fs};
use tauri::{AppHandle, Manager, Position, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::utils::{is_dev, write_some_log};

const ACTIVATION_STORAGE_FILE: &str = "activation_codes.enc";
const ACTIVATION_ASSET_CANDIDATES: [&str; 1] = ["assets/activation_codes.enc"];
const ACTIVATION_MISSING_MESSAGE: &str = "校验密码不存在";

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
    repository: ActivationRepository,
    status_path: PathBuf,
    activated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActivationStatus {
    pub activated: bool,
}

impl LicenseState {
    pub fn initialize(
        &self,
        repository: ActivationRepository,
        status_path: PathBuf,
        activated: bool,
    ) {
        let mut guard = self.inner.lock().unwrap();
        *guard = Some(LicenseInner {
            repository,
            status_path,
            activated,
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

    pub fn verify_and_consume(
        &self,
        encrypted_code: &str,
    ) -> Result<VerificationResult, LicenseError> {
        let mut guard = self.inner.lock().unwrap();
        let Some(inner) = guard.as_mut() else {
            return Err(LicenseError::Io(
                "license system has not been initialised".to_string(),
            ));
        };
        if inner.activated {
            return Ok(VerificationResult::Success);
        }

        let result = inner.repository.verify_and_consume(encrypted_code)?;
        if matches!(result, VerificationResult::Success) {
            inner.activated = true;
            persist_status(&inner.status_path, inner.activated)?;
        }
        Ok(result)
    }
}

#[derive(Debug, Serialize)]
pub struct ActivationAttemptPayload {
    pub success: bool,
    pub status: String,
    pub activated: bool,
}

fn copy_activation_asset_to(
    app_handle: &AppHandle,
    storage_path: &Path,
) -> Result<(), LicenseError> {
    let resolver = app_handle.path();
    let mut copied = false;

    for candidate in ACTIVATION_ASSET_CANDIDATES {
        if let Some(asset) = app_handle.asset_resolver().get(candidate.to_string()) {
            fs::write(storage_path, asset.bytes)?;
            write_some_log(&format!(
                "activation asset {candidate} copied via resolver to {}",
                storage_path.display()
            ));
            copied = true;
            break;
        }

        if let Ok(resource_dir) = resolver.resource_dir() {
            let candidate_path = resource_dir.join(candidate);
            if candidate_path.exists() {
                fs::copy(&candidate_path, storage_path)?;
                fs::remove_file(&candidate_path)?;
                write_some_log(&format!(
                    "activation asset {candidate} copied from resource_dir ({}) to {}",
                    candidate_path.display(),
                    storage_path.display()
                ));
                copied = true;
                break;
            }
        }

        let dev_candidate = env::current_dir()
            .map(|cwd| cwd.join("src-tauri").join("assets").join(candidate))
            .ok();
        if let Some(dev_path) = dev_candidate {
            if dev_path.exists() {
                fs::copy(&dev_path, storage_path)?;
                write_some_log(&format!(
                    "activation asset {candidate} copied from dev assets ({}) to {}",
                    dev_path.display(),
                    storage_path.display()
                ));
                copied = true;
                break;
            }
        }
    }

    if !copied {
        let message = ACTIVATION_MISSING_MESSAGE.to_string();
        println!("{message}");
        write_some_log(&message);
        app_handle
            .dialog()
            .message(message)
            .title("激活失败")
            .kind(MessageDialogKind::Error)
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
        process::exit(1);
    }

    Ok(())
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
pub fn submit_activation_code(
    app: AppHandle,
    state: State<LicenseState>,
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

    match state.verify_and_consume(encrypted_code.trim()) {
        Ok(VerificationResult::Success) => {
            if let Some(window) = app.get_webview_window("activation_gate") {
                let _ = window.hide();
                let _ = window.close();
            }
            if let Some(main) = app.get_webview_window("main") {
                reveal_main_window(main);
            }
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
    app_handle: &AppHandle,
) -> Result<Option<ActivationRepository>, LicenseError> {
    let Some(key) = env::var("ACTIVATION_MASTER_KEY").ok() else {
        println!("activation system disabled: missing ACTIVATION_MASTER_KEY");
        return Ok(None);
    };

    if key.trim().is_empty() {
        println!("activation system disabled: empty ACTIVATION_MASTER_KEY");
        return Ok(None);
    }

    let resolver = app_handle.path();
    let data_dir = resolver.app_data_dir().map_err(|err| {
        LicenseError::Io(format!("unable to determine app data directory: {err}"))
    })?;

    if !data_dir.exists() {
        fs::create_dir_all(&data_dir)?;
    }

    let storage_path = data_dir.join(ACTIVATION_STORAGE_FILE);
    if !storage_path.exists() {
        copy_activation_asset_to(app_handle, &storage_path)?;
    }

    if !storage_path.exists() {
        let message = format!("激活存储文件缺失: {}", storage_path.display());
        println!("{message}");
        write_some_log(&message);
        app_handle
            .dialog()
            .message(ACTIVATION_MISSING_MESSAGE)
            .title("激活失败")
            .kind(MessageDialogKind::Error)
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
        process::exit(1);
    }

    write_some_log(&format!(
        "activation storage resolved at {}",
        storage_path.display()
    ));

    let mut repository = ActivationRepository::new(&storage_path, &key)?;

    if let Err(err) = repository.load() {
        match err {
            LicenseError::Io(msg) | LicenseError::Serde(msg) => {
                let repair_log = format!(
                    "activation storage corrupted ({msg}); attempting restore from packaged asset"
                );
                println!("{repair_log}");
                write_some_log(&repair_log);

                if storage_path.exists() {
                    fs::remove_file(&storage_path)?;
                }

                copy_activation_asset_to(app_handle, &storage_path)?;
                repository = ActivationRepository::new(&storage_path, &key)?;
                repository.load().map_err(|inner| {
                    let failure = format!("activation storage restore failed: {inner}");
                    println!("{failure}");
                    write_some_log(&failure);
                    inner
                })?;
            }
            other => return Err(other),
        }
    }

    Ok(Some(repository))
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

pub fn load_activation_status(status_path: &Path) -> ActivationStatus {
    if !status_path.exists() {
        return ActivationStatus::default();
    }

    match fs::read_to_string(status_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ActivationStatus::default(),
    }
}

pub fn persist_status(path: &Path, activated: bool) -> Result<(), LicenseError> {
    let status = ActivationStatus { activated };
    let content = serde_json::to_string_pretty(&status)?;
    fs::write(path, content)?;
    Ok(())
}
