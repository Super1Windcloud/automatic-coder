mod capture;
mod config;
mod license;
mod system;
mod utils;
mod vlm;

use crate::{
    config::AppState,
    license::{
        LicenseState, get_activation_status, open_activation_window, prepare_activation_repository,
        show_main_window_now, submit_activation_code,
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
            check_activation_status(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
                    println!(
                        "activation repository unavailable; continuing without activation gate"
                    );
                    register_activation_shortcut(app.handle());
                    // show_main_window_now(app.handle());
                }
                Err(err) => {
                    state.disable();
                    println!("activation repository initialisation failed: {err}");
                    register_activation_shortcut(app.handle());
                    // show_main_window_now(app.handle());
                }
            }
        }
        Err(err) => {
            let state: tauri::State<LicenseState> = app.state();
            state.disable();
            println!("{err}");
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
