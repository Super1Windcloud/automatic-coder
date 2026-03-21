mod capture;
mod config;
mod license;
mod system;
mod utils;
mod vlm;

use crate::{
    config::AppState,
    license::{
        ActivationBootstrap, LicenseState, RemoteActivationConfig, get_activation_status,
        open_activation_window, prepare_activation_repository, show_main_window_now,
        submit_activation_code,
    },
};
use capture::*;
use config::*;
use dotenv::{dotenv, from_filename};
use license::load_activation_status;
use std::{fs, path::PathBuf};
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

        write_some_log(&format!("panic: {payload} @ {location}"));
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
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
            append_app_log,
            get_activation_status,
            submit_activation_code
        ])
        .setup(|app| {
            create_tray_icon(app);
            create_shortcut(app);
            load_preferences(app);
            check_activation_status_cheat(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[allow(dead_code)]
fn check_activation_status_cheat(app: &mut App<Wry>) {
    let state: tauri::State<LicenseState> = app.state();

    // 1. 依然获取路径，确保 state.inner 不是 None
    if let Ok(status_roots) = collect_status_roots(app) {
        // 构造一个假的引导数据
        let bootstrap = ActivationBootstrap {
            activation_key: "DEV_KEY".into(),
            remote: RemoteActivationConfig {
                owner: "".into(),
                repo: "".into(),
                tag: "".into(),
                token: "".into(),
                raw_url: "".into(),
            },
        };

        // 2. 强制初始化为“已激活”状态
        state.initialize(bootstrap, status_roots, true);

        // 3. 必须注册快捷键
        register_activation_shortcut(app.handle());

        // 4. 显式显示主窗口
        show_main_window_now(app.handle());
    }
}

fn check_activation_status(app: &mut App<Wry>) {
    match collect_status_roots(app) {
        Ok(status_roots) => {
            let status = load_activation_status(&status_roots);
            let state: tauri::State<LicenseState> = app.state();
            match prepare_activation_repository(app.handle()) {
                Ok(Some(bootstrap)) => {
                    let needs_activation = !status.activated;
                    state.initialize(bootstrap, status_roots.clone(), status.activated);
                    if needs_activation {
                        open_activation_window(app.handle());
                    } else {
                        register_activation_shortcut(app.handle());
                        // show_main_window_now(app.handle());
                    }
                }
                Ok(None) => {
                    state.disable();
                    if is_dev() {
                        println!(
                            "activation repository unavailable; continuing without activation gate"
                        );
                    } else {
                        write_some_log(
                            "activation repository unavailable; continuing without activation gate",
                        );
                    }
                    register_activation_shortcut(app.handle());
                    // show_main_window_now(app.handle());
                }
                Err(err) => {
                    state.disable();
                    if is_dev() {
                        println!("activation repository initialisation failed: {err}");
                    } else {
                        write_some_log(
                            format!("activation repository initialisation failed: {err}").as_str(),
                        );
                    }
                    register_activation_shortcut(app.handle());
                    // show_main_window_now(app.handle());
                }
            }
        }
        Err(err) => {
            let state: tauri::State<LicenseState> = app.state();
            state.disable();
            if is_dev() {
                println!("{err}");
            } else {
                write_some_log(format!("{err}").as_str());
            }
            show_main_window_now(app.handle());
        }
    }
}

fn collect_status_roots(app: &App<Wry>) -> Result<Vec<PathBuf>, String> {
    let resolver = app.path();
    let mut roots = Vec::with_capacity(3);
    let targets = [
        ("documents", resolver.document_dir()),
        ("local", resolver.app_local_data_dir()),
        ("roaming", resolver.app_data_dir()),
    ];

    for (label, dir_result) in targets {
        let dir =
            dir_result.map_err(|err| format!("failed to resolve {label} directory: {err}"))?;
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .map_err(|err| format!("failed to prepare {label} directory: {err}"))?;
        }
        roots.push(dir);
    }

    Ok(roots)
}
