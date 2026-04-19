use dirs::home_dir;
use dotenv::{dotenv, from_filename};
use license_manager::{create_machine_id, verify_signed_license, verify_signed_revocation_list};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs};
use tauri::{
    AppHandle, Emitter, Manager, Position, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tokio::time::sleep;

use crate::{
    app_debug, app_error, app_warn,
    config::{open_language_selector, preferences_require_onboarding},
    system::register_activation_shortcut,
    utils::is_dev,
};

const LICENSE_FILE_NAME: &str = "license.json";
const REVOCATION_CACHE_FILE_NAME: &str = "revocations.json";
const HOST_ISSUED_LICENSES_FILE_NAME: &str = "issued_licenses.json";
const DEFAULT_REVOCATION_SYNC_INTERVAL_SECS: u64 = 900;
const HOST_MACHINE_ID: &str =
    "e387c89a24daa5544b9d4b795f8a28af5b23b5c080839e07558e7311aac14a11";

pub struct LicenseState {
    inner: Mutex<Option<LicenseInner>>,
    enabled: AtomicBool,
    pub(crate) monitor_started: AtomicBool,
}

impl Default for LicenseState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
            enabled: AtomicBool::new(false),
            monitor_started: AtomicBool::new(false),
        }
    }
}

struct LicenseInner {
    public_key: String,
    revocation_url: String,
    cache_dir: PathBuf,
    activated: bool,
    sync_interval_seconds: u64,
    client: Client,
}

#[derive(Debug, Clone, Default)]
pub struct ActivationStatus {
    pub activated: bool,
}

#[derive(Debug, Serialize)]
pub struct ActivationAttemptPayload {
    pub success: bool,
    pub status: String,
    pub activated: bool,
}

#[derive(Debug, Serialize)]
pub struct HostManagementContextPayload {
    pub public_key: String,
    pub revocation_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssuedLicensePreviewPayload {
    pub issued_at: u64,
    pub license_id: String,
    pub machine_id: String,
    pub customer: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevocationDiagnosticItemPayload {
    pub index: usize,
    pub ok: bool,
    pub version: Option<u64>,
    pub generated_at: Option<u64>,
    pub revoked_count: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevocationDiagnosticPayload {
    pub path: String,
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub items: Vec<RevocationDiagnosticItemPayload>,
}

#[derive(Clone)]
pub struct LicenseBootstrap {
    pub public_key: String,
    pub revocation_url: String,
    pub sync_interval_seconds: u64,
}

#[derive(Clone)]
struct LicenseRuntimeContext {
    public_key: String,
    revocation_url: String,
    cache_dir: PathBuf,
    client: Client,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct IssuedLicenseRecord {
    issued_at: u64,
    license_id: String,
    machine_id: String,
    customer: Option<String>,
    expires_at: Option<u64>,
    #[serde(default)]
    revoked: bool,
    signed_license: serde_json::Value,
}

#[derive(Debug)]
enum LicenseValidationError {
    InvalidFormat,
    InvalidSignature,
    MachineMismatch,
    Expired,
    RevocationUnavailable,
}

impl fmt::Display for LicenseValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseValidationError::InvalidFormat => write!(f, "invalid_format"),
            LicenseValidationError::InvalidSignature => write!(f, "invalid_signature"),
            LicenseValidationError::MachineMismatch => write!(f, "machine_mismatch"),
            LicenseValidationError::Expired => write!(f, "expired"),
            LicenseValidationError::RevocationUnavailable => write!(f, "revocation_unavailable"),
        }
    }
}

impl LicenseState {
    pub fn initialize(&self, bootstrap: LicenseBootstrap, cache_dir: PathBuf, activated: bool) {
        let mut guard = self.inner.lock().unwrap();
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("InterviewCoder/license-runtime")
            .build()
            .expect("failed to construct license HTTP client");
        *guard = Some(LicenseInner {
            public_key: bootstrap.public_key,
            revocation_url: bootstrap.revocation_url,
            cache_dir,
            activated,
            sync_interval_seconds: bootstrap.sync_interval_seconds,
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

    pub fn is_activated(&self) -> bool {
        self.inner
            .lock()
            .unwrap()
            .as_ref()
            .map(|inner| inner.activated)
            .unwrap_or(false)
    }

    pub fn set_activated(&self, activated: bool) {
        if let Some(inner) = self.inner.lock().unwrap().as_mut() {
            inner.activated = activated;
        }
    }

    fn context(&self) -> Option<LicenseRuntimeContext> {
        self.inner
            .lock()
            .unwrap()
            .as_ref()
            .map(|inner| LicenseRuntimeContext {
                public_key: inner.public_key.clone(),
                revocation_url: inner.revocation_url.clone(),
                cache_dir: inner.cache_dir.clone(),
                client: inner.client.clone(),
            })
    }

    fn sync_interval_seconds(&self) -> Option<u64> {
        self.inner
            .lock()
            .unwrap()
            .as_ref()
            .map(|inner| inner.sync_interval_seconds)
    }
}

#[tauri::command]
pub fn get_activation_status(state: State<LicenseState>) -> Result<bool, String> {
    if !state.is_enabled() {
        return Ok(true);
    }
    if !state.is_ready() {
        return Ok(false);
    }
    Ok(state.is_activated())
}

#[tauri::command]
pub fn get_machine_id() -> Result<String, String> {
    Ok(compute_machine_id())
}

#[tauri::command]
pub async fn submit_activation_code(
    app: AppHandle,
    state: State<'_, LicenseState>,
    encrypted_code: String,
) -> Result<ActivationAttemptPayload, String> {
    let license = encrypted_code.trim();
    if license.is_empty() {
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
    let Some(context) = state.context() else {
        return Ok(ActivationAttemptPayload {
            success: false,
            status: "pending_initialisation".into(),
            activated: false,
        });
    };

    let claims = match validate_signed_license(license, &context.public_key) {
        Ok(claims) => claims,
        Err(err) => {
            return Ok(ActivationAttemptPayload {
                success: false,
                status: err.to_string(),
                activated: false,
            });
        }
    };

    match resolve_revocation_status(&context, &claims.license_id).await {
        Ok(true) => {
            return Ok(ActivationAttemptPayload {
                success: false,
                status: "revoked".into(),
                activated: false,
            });
        }
        Ok(false) => {}
        Err(_) => {
            app_warn!(
                "license",
                "revocation check unavailable during activation; allowing local license to proceed"
            );
        }
    }

    persist_license(&context, license)?;
    state.set_activated(true);
    register_activation_shortcut(&app);
    if let Some(window) = app.get_webview_window("activation_gate") {
        let _ = window.hide();
        let _ = window.close();
    }
    if let Some(main) = app.get_webview_window("main") {
        reveal_main_window(main);
    }
    if preferences_require_onboarding() {
        open_language_selector(&app);
    }
    let _ = app.emit("activation_granted", true);
    Ok(ActivationAttemptPayload {
        success: true,
        status: "success".into(),
        activated: true,
    })
}

pub fn prepare_license_runtime(app_handle: &AppHandle) -> Result<Option<LicenseBootstrap>, String> {
    hydrate_activation_env();
    let Some(public_key) = env::var("LICENSE_PUBLIC_KEY")
        .ok()
        .or_else(|| option_env!("LICENSE_PUBLIC_KEY").map(|value| value.to_string()))
    else {
        app_warn!(
            "license",
            "license system disabled: missing LICENSE_PUBLIC_KEY"
        );
        return Ok(None);
    };

    if public_key.trim().is_empty() {
        return Ok(None);
    }

    let revocation_url = env::var("LICENSE_REVOCATION_URL")
        .ok()
        .or_else(|| option_env!("LICENSE_REVOCATION_URL").map(|value| value.to_string()))
        .unwrap_or_default();
    let sync_interval_seconds = env::var("LICENSE_SYNC_INTERVAL_SECONDS")
        .ok()
        .or_else(|| option_env!("LICENSE_SYNC_INTERVAL_SECONDS").map(|value| value.to_string()))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_REVOCATION_SYNC_INTERVAL_SECS);

    let cache_dir = resolve_license_cache_dir(app_handle)?;
    fs::create_dir_all(&cache_dir).map_err(|err| err.to_string())?;

    Ok(Some(LicenseBootstrap {
        public_key,
        revocation_url,
        sync_interval_seconds,
    }))
}

pub fn host_management_available(_app_handle: &AppHandle) -> bool {
    hydrate_activation_env();
    if !host_machine_matches_current() {
        return false;
    }
    let private_ready = env::var("LICENSE_PRIVATE_KEY")
        .ok()
        .or_else(|| option_env!("LICENSE_PRIVATE_KEY").map(|value| value.to_string()))
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let public_ready = env::var("LICENSE_PUBLIC_KEY")
        .ok()
        .or_else(|| option_env!("LICENSE_PUBLIC_KEY").map(|value| value.to_string()))
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    private_ready && public_ready
}

#[tauri::command]
pub fn host_get_management_context() -> Result<HostManagementContextPayload, String> {
    let private_key = load_host_private_key()?;
    let public_key = env::var("LICENSE_PUBLIC_KEY")
        .ok()
        .or_else(|| option_env!("LICENSE_PUBLIC_KEY").map(|value| value.to_string()))
        .unwrap_or_default();
    let revocation_url = env::var("LICENSE_REVOCATION_URL")
        .ok()
        .or_else(|| option_env!("LICENSE_REVOCATION_URL").map(|value| value.to_string()))
        .unwrap_or_default();

    if public_key.trim().is_empty() {
        return Err("missing LICENSE_PUBLIC_KEY".into());
    }

    let _ = private_key;
    Ok(HostManagementContextPayload {
        public_key,
        revocation_url,
    })
}

#[tauri::command]
pub fn host_issue_license(
    app_handle: AppHandle,
    machine_id: String,
    license_id: String,
    expires_days: Option<u64>,
    customer: Option<String>,
) -> Result<String, String> {
    let private_key = load_host_private_key()?;
    let expires_at =
        expires_days.map(|days| license_manager::now_unix_seconds() + days * 24 * 60 * 60);
    let normalized_customer = customer.filter(|value| !value.trim().is_empty());
    let claims = license_manager::new_license_claims(
        license_id.clone(),
        machine_id.clone(),
        normalized_customer.clone(),
        expires_at,
        vec!["base".to_string()],
    );
    let signed_license =
        license_manager::sign_license(&private_key, claims).map_err(|err| err.to_string())?;
    let signed_license_json =
        serde_json::from_str::<serde_json::Value>(&signed_license).map_err(|err| err.to_string())?;
    persist_issued_license_record(
        &app_handle,
        IssuedLicenseRecord {
            issued_at: now_unix_seconds(),
            license_id,
            machine_id,
            customer: normalized_customer,
            expires_at,
            revoked: false,
            signed_license: signed_license_json,
        },
    )?;
    Ok(signed_license)
}

#[tauri::command]
pub fn host_sign_revocations(
    app_handle: AppHandle,
    version: u64,
    revoked: Vec<String>,
) -> Result<String, String> {
    let private_key = load_host_private_key()?;
    let revoked = revoked
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let payload = license_manager::new_revocation_list(revoked, version);
    let signed_payload =
        license_manager::sign_revocation_list(&private_key, payload).map_err(|err| err.to_string())?;
    sync_issued_license_revocations(&app_handle, &signed_payload)?;
    persist_host_revocations(&app_handle, &signed_payload)?;
    Ok(signed_payload)
}

#[tauri::command]
pub fn host_list_issued_licenses(
    app_handle: AppHandle,
) -> Result<Vec<IssuedLicensePreviewPayload>, String> {
    load_host_private_key()?;
    let records = load_issued_license_records(&app_handle)?;
    Ok(records
        .into_iter()
        .filter(|record| !record.revoked)
        .rev()
        .map(|record| IssuedLicensePreviewPayload {
            issued_at: record.issued_at,
            license_id: record.license_id,
            machine_id: record.machine_id,
            customer: record.customer,
            revoked: record.revoked,
        })
        .collect())
}

#[tauri::command]
pub fn host_diagnose_revocations() -> Result<RevocationDiagnosticPayload, String> {
    let public_key = env::var("LICENSE_PUBLIC_KEY")
        .ok()
        .or_else(|| option_env!("LICENSE_PUBLIC_KEY").map(|value| value.to_string()))
        .unwrap_or_default();
    if public_key.trim().is_empty() {
        return Err("missing LICENSE_PUBLIC_KEY".into());
    }

    let path = resolve_workspace_revocations_path();
    let entries = load_signed_revocation_payloads_from_path(&path)?;
    let mut items = Vec::with_capacity(entries.len());
    let mut valid = 0usize;

    for (index, entry) in entries.iter().enumerate() {
        let version = entry
            .get("payload")
            .and_then(|payload| payload.get("version"))
            .and_then(|value| value.as_u64());
        let generated_at = entry
            .get("payload")
            .and_then(|payload| payload.get("generated_at"))
            .and_then(|value| value.as_u64());
        let revoked_count = entry
            .get("payload")
            .and_then(|payload| payload.get("revoked"))
            .and_then(|value| value.as_array())
            .map(|items| items.len());

        let signed = serde_json::to_string(entry).map_err(|err| err.to_string())?;
        match verify_signed_revocation_list(&public_key, &signed) {
            Ok(_) => {
                valid += 1;
                items.push(RevocationDiagnosticItemPayload {
                    index,
                    ok: true,
                    version,
                    generated_at,
                    revoked_count,
                    error: None,
                });
            }
            Err(err) => {
                items.push(RevocationDiagnosticItemPayload {
                    index,
                    ok: false,
                    version,
                    generated_at,
                    revoked_count,
                    error: Some(err.to_string()),
                });
            }
        }
    }

    let total = items.len();
    Ok(RevocationDiagnosticPayload {
        path: path.display().to_string(),
        total,
        valid,
        invalid: total.saturating_sub(valid),
        items,
    })
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

pub fn open_host_management_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("host_management") {
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = window.show();
        return;
    }

    let url = if is_dev() {
        WebviewUrl::App("host.html".into())
    } else {
        WebviewUrl::App("host/host.html".into())
    };

    let window = WebviewWindowBuilder::new(app_handle, "host_management", url)
        .title("本地宿主管理")
        .inner_size(820.0, 760.0)
        .resizable(true)
        .minimizable(true)
        .maximizable(true)
        .closable(true)
        .decorations(false)
        .visible(true)
        .always_on_top(true)
        .skip_taskbar(false)
        .build()
        .expect("failed to create host management window");

    let _ = window.center();
    let _ = window.set_focus();
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

pub fn load_activation_status(app_handle: &AppHandle, public_key: &str) -> ActivationStatus {
    let cache_dir = match resolve_license_cache_dir(app_handle) {
        Ok(dir) => dir,
        Err(err) => {
            app_error!("license", "{err}");
            return ActivationStatus::default();
        }
    };

    match load_valid_license_claims_from_dir(&cache_dir, public_key) {
        Ok(Some(_)) => ActivationStatus { activated: true },
        Ok(None) => ActivationStatus::default(),
        Err(err) => {
            app_warn!("license", "failed to load local license: {err}");
            ActivationStatus::default()
        }
    }
}

pub fn start_revocation_monitor(app_handle: &AppHandle) {
    let state: State<LicenseState> = app_handle.state();
    if state
        .monitor_started
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let app = app_handle.clone();
    let interval_seconds = state
        .sync_interval_seconds()
        .unwrap_or(DEFAULT_REVOCATION_SYNC_INTERVAL_SECS);
    tauri::async_runtime::spawn(async move {
        loop {
            sleep(Duration::from_secs(interval_seconds)).await;
            if let Err(err) = refresh_runtime_license(&app).await {
                app_warn!("license", "background license refresh failed: {err}");
            }
        }
    });
}

pub async fn refresh_runtime_license(app_handle: &AppHandle) -> Result<(), String> {
    let state: State<LicenseState> = app_handle.state();
    if !state.is_enabled() || !state.is_activated() {
        return Ok(());
    }

    let Some(context) = state.context() else {
        return Ok(());
    };
    let Some(claims) = load_valid_license_claims_from_dir(&context.cache_dir, &context.public_key)?
    else {
        invalidate_runtime_license(app_handle, "missing_or_invalid_license");
        return Ok(());
    };

    match resolve_revocation_status(&context, &claims.license_id).await {
        Ok(true) => invalidate_runtime_license(app_handle, "revoked"),
        Ok(false) => {}
        Err(err) => {
            app_warn!(
                "license",
                "revocation verification unavailable; keeping current license active: {err}"
            );
        }
    }

    Ok(())
}

fn invalidate_runtime_license(app_handle: &AppHandle, reason: &str) {
    let state: State<LicenseState> = app_handle.state();
    state.set_activated(false);
    if let Some(main) = app_handle.get_webview_window("main") {
        let _ = main.hide();
    }
    open_activation_window(app_handle);
    let _ = app_handle.emit("activation_revoked", reason.to_string());
}

fn resolve_license_cache_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_local_data_dir()
        .map(|dir| dir.join("license"))
        .map_err(|err| format!("failed to resolve license cache dir: {err}"))
}

fn resolve_host_management_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_local_data_dir()
        .map(|dir| dir.join("host-management"))
        .map_err(|err| format!("failed to resolve host management dir: {err}"))
}

fn persist_license(context: &LicenseRuntimeContext, signed_license: &str) -> Result<(), String> {
    fs::create_dir_all(&context.cache_dir).map_err(|err| err.to_string())?;
    fs::write(
        context.cache_dir.join(LICENSE_FILE_NAME),
        signed_license.trim(),
    )
    .map_err(|err| err.to_string())
}

fn persist_issued_license_record(
    app_handle: &AppHandle,
    record: IssuedLicenseRecord,
) -> Result<(), String> {
    let host_dir = resolve_host_management_dir(app_handle)?;
    fs::create_dir_all(&host_dir).map_err(|err| err.to_string())?;

    let path = host_dir.join(HOST_ISSUED_LICENSES_FILE_NAME);
    let mut records = load_issued_license_records(app_handle)?;
    records.push(record);
    fs::write(
        path,
        serde_json::to_string_pretty(&records).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn persist_host_revocations(app_handle: &AppHandle, signed_payload: &str) -> Result<(), String> {
    let host_dir = resolve_host_management_dir(app_handle)?;
    fs::create_dir_all(&host_dir).map_err(|err| err.to_string())?;
    let path = host_dir.join(REVOCATION_CACHE_FILE_NAME);
    let next_payload =
        serde_json::from_str::<serde_json::Value>(signed_payload).map_err(|err| err.to_string())?;
    let mut payloads = load_signed_revocation_payloads_from_path(&path)?;
    payloads.push(next_payload);
    let serialized = serde_json::to_string_pretty(&payloads).map_err(|err| err.to_string())?;
    fs::write(&path, &serialized).map_err(|err| err.to_string())?;
    let workspace_path = resolve_workspace_revocations_path();
    fs::write(workspace_path, &serialized).map_err(|err| err.to_string())?;
    Ok(())
}

fn sync_issued_license_revocations(app_handle: &AppHandle, signed_payload: &str) -> Result<(), String> {
    let host_dir = resolve_host_management_dir(app_handle)?;
    fs::create_dir_all(&host_dir).map_err(|err| err.to_string())?;
    let path = host_dir.join(HOST_ISSUED_LICENSES_FILE_NAME);
    let current_revocations_path = host_dir.join(REVOCATION_CACHE_FILE_NAME);
    let mut payloads = load_signed_revocation_payloads_from_path(&current_revocations_path)?;
    payloads.push(serde_json::from_str::<serde_json::Value>(signed_payload).map_err(|err| err.to_string())?);
    let revoked = collect_revoked_ids_from_values(&payloads);
    let mut records = load_issued_license_records(app_handle)?;
    for record in &mut records {
        record.revoked = revoked.contains(&record.license_id);
    }
    fs::write(
        path,
        serde_json::to_string_pretty(&records).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn load_issued_license_records(app_handle: &AppHandle) -> Result<Vec<IssuedLicenseRecord>, String> {
    let path = resolve_host_management_dir(app_handle)?.join(HOST_ISSUED_LICENSES_FILE_NAME);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str::<Vec<IssuedLicenseRecord>>(&raw).map_err(|err| err.to_string())
}

fn load_signed_revocation_payloads_from_path(path: &Path) -> Result<Vec<serde_json::Value>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let parsed = serde_json::from_str::<serde_json::Value>(&raw).map_err(|err| err.to_string())?;
    match parsed {
        serde_json::Value::Array(items) => Ok(items),
        serde_json::Value::Object(_) => Ok(vec![parsed]),
        _ => Err("invalid revocations.json format".into()),
    }
}

fn resolve_workspace_revocations_path() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = crate_dir.parent().unwrap_or(crate_dir.as_path());
    workspace_dir.join(REVOCATION_CACHE_FILE_NAME)
}

fn collect_revoked_ids_from_values(
    payloads: &[serde_json::Value],
) -> std::collections::HashSet<String> {
    payloads
        .iter()
        .filter_map(|entry| entry.get("payload"))
        .filter_map(|payload| payload.get("revoked"))
        .filter_map(|items| items.as_array())
        .flat_map(|items| items.iter())
        .filter_map(|item| item.as_str().map(|value| value.to_string()))
        .collect()
}

fn load_valid_license_claims_from_dir(
    cache_dir: &Path,
    public_key: &str,
) -> Result<Option<license_manager::LicenseClaims>, String> {
    let path = cache_dir.join(LICENSE_FILE_NAME);
    if !path.exists() {
        return Ok(None);
    }
    let signed_license = fs::read_to_string(path).map_err(|err| err.to_string())?;
    validate_signed_license(&signed_license, public_key)
        .map(Some)
        .map_err(|err| err.to_string())
}

fn validate_signed_license(
    signed_license: &str,
    public_key: &str,
) -> Result<license_manager::LicenseClaims, LicenseValidationError> {
    let claims = verify_signed_license(public_key, signed_license).map_err(|err| {
        let message = err.to_string();
        if message.contains("signature") {
            LicenseValidationError::InvalidSignature
        } else {
            LicenseValidationError::InvalidFormat
        }
    })?;

    // 实时计算当前机器的硬件指纹并比对
    let current_machine_id = compute_machine_id();
    if claims.machine_id != current_machine_id {
        app_warn!(
            "license",
            "machine mismatch: license={}, current={}",
            claims.machine_id,
            current_machine_id
        );
        return Err(LicenseValidationError::MachineMismatch);
    }

    if let Some(expires_at) = claims.expires_at
        && expires_at <= now_unix_seconds()
    {
        return Err(LicenseValidationError::Expired);
    }

    Ok(claims)
}

async fn resolve_revocation_status(
    context: &LicenseRuntimeContext,
    license_id: &str,
) -> Result<bool, LicenseValidationError> {
    if context.revocation_url.trim().is_empty() {
        return Ok(false);
    }

    fetch_remote_revocations(context)
        .await
        .map(|revoked| {
            let matched = revoked.iter().any(|item| item == license_id);
            app_debug!(
                "license",
                "revocation resolved for license_id={}, revoked_count={}, matched={}",
                license_id,
                revoked.len(),
                matched
            );
            matched
        })
        .map_err(|err| {
            app_warn!("license", "failed to fetch remote revocations: {err}");
            LicenseValidationError::RevocationUnavailable
        })
}

async fn fetch_remote_revocations(
    context: &LicenseRuntimeContext,
) -> Result<Vec<String>, String> {
    let response = context
        .client
        .get(&context.revocation_url)
        .header("User-Agent", "InterviewCoder/revocations")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!("unexpected status {}", response.status()));
    }
    let body = response.text().await.map_err(|err| err.to_string())?;
    extract_revoked_ids_from_signed_payloads(&context.public_key, &body)
}

fn extract_revoked_ids_from_signed_payloads(
    public_key: &str,
    raw: &str,
) -> Result<Vec<String>, String> {
    let parsed = serde_json::from_str::<serde_json::Value>(raw).map_err(|err| err.to_string())?;
    let entries = match parsed {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(_) => vec![parsed],
        _ => return Err("invalid revocation payload format".into()),
    };

    let mut revoked = Vec::new();
    app_debug!(
        "license",
        "verifying remote revocations payload entries={}",
        entries.len()
    );
    for (index, entry) in entries.into_iter().enumerate() {
        let version = entry
            .get("payload")
            .and_then(|payload| payload.get("version"))
            .and_then(|value| value.as_u64())
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".into());
        let generated_at = entry
            .get("payload")
            .and_then(|payload| payload.get("generated_at"))
            .and_then(|value| value.as_u64())
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".into());
        let revoked_count = entry
            .get("payload")
            .and_then(|payload| payload.get("revoked"))
            .and_then(|value| value.as_array())
            .map(|items| items.len())
            .unwrap_or(0);
        let signature_preview = entry
            .get("signature")
            .and_then(|value| value.as_str())
            .map(|value| value.chars().take(16).collect::<String>())
            .unwrap_or_else(|| "missing".into());
        let signed = serde_json::to_string(&entry).map_err(|err| err.to_string())?;
        app_debug!(
            "license",
            "verifying revocation entry index={}, version={}, generated_at={}, revoked_count={}, signature_prefix={}",
            index,
            version,
            generated_at,
            revoked_count,
            signature_preview
        );
        let payload = match verify_signed_revocation_list(public_key, &signed) {
            Ok(payload) => {
                app_debug!(
                    "license",
                    "revocation entry verified index={}, version={}, revoked_count={}",
                    index,
                    payload.version,
                    payload.revoked.len()
                );
                payload
            }
            Err(err) => {
                app_warn!(
                    "license",
                    "revocation entry verify failed index={}, version={}, generated_at={}, signature_prefix={}, error={}",
                    index,
                    version,
                    generated_at,
                    signature_preview,
                    err
                );
                return Err(err.to_string());
            }
        };
        revoked.extend(payload.revoked);
    }
    Ok(revoked)
}

fn compute_machine_id() -> String {
    create_machine_id(&collect_machine_signature())
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

    app_debug!("license", "machine signature parts collected");
    let mut hasher = Sha256::new();
    hasher.update(parts.join("|").as_bytes());
    hex::encode(hasher.finalize())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_host_private_key() -> Result<String, String> {
    hydrate_activation_env();
    if !host_machine_matches_current() {
        return Err("host private key is not available on this machine".into());
    }
    let private_key = env::var("LICENSE_PRIVATE_KEY")
        .ok()
        .or_else(|| option_env!("LICENSE_PRIVATE_KEY").map(|value| value.to_string()))
        .unwrap_or_default();
    if private_key.trim().is_empty() {
        return Err("LICENSE_PRIVATE_KEY is not configured on this machine".into());
    }
    Ok(private_key)
}

fn host_machine_matches_current() -> bool {
    let expected_machine_id = HOST_MACHINE_ID.trim();
    if expected_machine_id.is_empty() {
        return true;
    }
    compute_machine_id() == expected_machine_id
}
