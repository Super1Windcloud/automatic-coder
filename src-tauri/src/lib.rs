mod capture;
mod config;
mod lan;
mod license;
mod system;
mod utils;
mod vlm;

use crate::{
    config::AppState,
    license::{
        LicenseBootstrap, LicenseState, get_activation_status, get_machine_id,
        host_get_management_context, host_issue_license, host_sign_revocations,
        open_activation_window, prepare_license_runtime, show_main_window_now,
        start_revocation_monitor, submit_activation_code,
    },
};
use capture::*;
use config::*;
use dotenv::{dotenv, from_filename};
use lan::{LanAnswerState, start_lan_answer_server};
use license::load_activation_status;
use system::*;
use tauri::{App, Manager, Wry};
#[allow(unused_imports)]
use utils::*;
use vlm::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    from_filename("src-tauri/.env").ok();
    dotenv().ok();
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
            *message
        } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
            message.as_str()
        } else {
            "unknown panic payload"
        };

        let location = if let Some(location) = panic_info.location() {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        } else {
            "unknown location".to_string()
        };

        app_error!("app", "panic: {payload} @ {location}");
    }));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::default())
        .manage(LanAnswerState::default())
        .manage(LicenseState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            show_window,
            set_selected_language,
            get_screen_capture_to_path,
            get_screen_capture_to_bytes,
            set_capture_position,
            get_store_config,
            set_selected_language_prompt,
            create_screenshot_solution_stream,
            set_vlm_key,
            set_vlm_model,
            set_custom_openai_enabled,
            save_custom_openai_config,
            set_page_opacity,
            set_background_broadcast,
            get_activation_status,
            get_machine_id,
            host_get_management_context,
            host_issue_license,
            host_sign_revocations,
            submit_activation_code
        ])
        .setup(|app| {
            if let Err(err) = clear_app_log(app.handle()) {
                eprintln!("failed to clear startup log file: {err}");
            } else {
                app_info!("app", "startup log file cleared");
            }
            load_preferences(app);
            create_tray_icon(app);
            create_shortcut(app);
            if let Err(err) = start_lan_answer_server(app) {
                app_error!("app", "{err}");
            }
            check_activation_status(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[allow(dead_code)]
fn check_activation_status_cheat(app: &mut App<Wry>) {
    let state: tauri::State<LicenseState> = app.state();

    // 1. 依然获取路径，确保 state.inner 不是 None
    if let Ok(cache_dir) = app.path().app_local_data_dir() {
        let bootstrap = LicenseBootstrap {
            public_key: "DEV_KEY".into(),
            revocation_url: String::new(),
            offline_grace_seconds: 72 * 60 * 60,
            sync_interval_seconds: 900,
        };

        state.initialize(bootstrap, cache_dir.join("license"), true);

        // 3. 必须注册快捷键
        register_activation_shortcut(app.handle());

        // 4. 显式显示主窗口
        show_main_window_now(app.handle());
    }
}

fn check_activation_status(app: &mut App<Wry>) {
    let state: tauri::State<LicenseState> = app.state();
    match prepare_license_runtime(app.handle()) {
        Ok(Some(bootstrap)) => {
            let status = load_activation_status(app.handle(), &bootstrap.public_key);
            let cache_dir = app
                .path()
                .app_local_data_dir()
                .map(|dir| dir.join("license"))
                .unwrap_or_else(|_| std::path::PathBuf::from(".license"));
            let needs_activation = !status.activated;
            state.initialize(bootstrap, cache_dir, status.activated);
            start_revocation_monitor(app.handle());
            if needs_activation {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.hide();
                }
                open_activation_window(app.handle());
            } else {
                register_activation_shortcut(app.handle());
                if preferences_require_onboarding() {
                    open_language_selector(app.handle());
                }
            }
        }
        Ok(None) => {
            state.disable();
            app_warn!(
                "app",
                "license runtime unavailable; continuing without activation gate"
            );
            register_activation_shortcut(app.handle());
        }
        Err(err) => {
            state.disable();
            app_error!("app", "license runtime initialisation failed: {err}");
            register_activation_shortcut(app.handle());
        }
    }
}
