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
    app_debug, app_error, app_warn, config::open_language_selector,
    system::register_activation_shortcut, utils::is_dev,
};

const LICENSE_FILE_NAME: &str = "license.json";
const REVOCATION_CACHE_FILE_NAME: &str = "revocations.json";
const DEFAULT_REVOCATION_SYNC_INTERVAL_SECS: u64 = 900;

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
    offline_grace_seconds: u64,
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

#[derive(Clone)]
pub struct LicenseBootstrap {
    pub public_key: String,
    pub revocation_url: String,
    pub offline_grace_seconds: u64,
    pub sync_interval_seconds: u64,
}

#[derive(Clone)]
struct LicenseRuntimeContext {
    public_key: String,
    revocation_url: String,
    cache_dir: PathBuf,
    offline_grace_seconds: u64,
    client: Client,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RevocationCache {
    fetched_at: u64,
    signed_payload: String,
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
            offline_grace_seconds: bootstrap.offline_grace_seconds,
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
                offline_grace_seconds: inner.offline_grace_seconds,
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
            return Ok(ActivationAttemptPayload {
                success: false,
                status: "revocation_unavailable".into(),
                activated: false,
            });
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
    open_language_selector(&app);
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
    let offline_grace_seconds = env::var("LICENSE_OFFLINE_GRACE_HOURS")
        .ok()
        .or_else(|| option_env!("LICENSE_OFFLINE_GRACE_HOURS").map(|value| value.to_string()))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(72)
        * 60
        * 60;
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
        offline_grace_seconds,
        sync_interval_seconds,
    }))
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
        Err(_) => invalidate_runtime_license(app_handle, "revocation_unavailable"),
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

fn persist_license(context: &LicenseRuntimeContext, signed_license: &str) -> Result<(), String> {
    fs::create_dir_all(&context.cache_dir).map_err(|err| err.to_string())?;
    fs::write(
        context.cache_dir.join(LICENSE_FILE_NAME),
        signed_license.trim(),
    )
    .map_err(|err| err.to_string())
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

    if claims.machine_id != compute_machine_id() {
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

    match fetch_and_cache_revocations(context).await {
        Ok(revoked) => Ok(revoked.iter().any(|item| item == license_id)),
        Err(err) => {
            app_warn!("license", "failed to fetch remote revocations: {err}");
            if let Some(cached) = load_cached_revocations(context)? {
                if now_unix_seconds().saturating_sub(cached.fetched_at)
                    <= context.offline_grace_seconds
                {
                    let payload =
                        verify_signed_revocation_list(&context.public_key, &cached.signed_payload)
                            .map_err(|_| LicenseValidationError::InvalidSignature)?;
                    return Ok(payload.revoked.iter().any(|item| item == license_id));
                }
            }
            Err(LicenseValidationError::RevocationUnavailable)
        }
    }
}

async fn fetch_and_cache_revocations(
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
    let payload =
        verify_signed_revocation_list(&context.public_key, &body).map_err(|err| err.to_string())?;
    let cache = RevocationCache {
        fetched_at: now_unix_seconds(),
        signed_payload: body,
    };
    fs::create_dir_all(&context.cache_dir).map_err(|err| err.to_string())?;
    fs::write(
        context.cache_dir.join(REVOCATION_CACHE_FILE_NAME),
        serde_json::to_string_pretty(&cache).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    Ok(payload.revoked)
}

fn load_cached_revocations(
    context: &LicenseRuntimeContext,
) -> Result<Option<RevocationCache>, LicenseValidationError> {
    let path = context.cache_dir.join(REVOCATION_CACHE_FILE_NAME);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(path).map_err(|_| LicenseValidationError::RevocationUnavailable)?;
    let parsed: RevocationCache =
        serde_json::from_str(&raw).map_err(|_| LicenseValidationError::RevocationUnavailable)?;
    Ok(Some(parsed))
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
