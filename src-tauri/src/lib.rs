mod capture;
mod config;
mod license;
mod system;
mod utils;
mod vlm;

use crate::{
    config::AppState,
    license::{
        get_activation_status, open_activation_window, prepare_activation_repository,
        show_main_window_now, submit_activation_code, LicenseState,
    },
};
use capture::*;
use config::*;
use dotenv::dotenv;
use license::load_activation_status;
use system::*;
#[allow(unused_imports)]
use utils::*;
use vlm::*;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenv().ok();

    tauri::Builder::default()
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
            append_app_log,
            get_activation_status,
            submit_activation_code
        ])
        .setup(|app| {
            create_tray_icon(app);
            create_shortcut(app);
            load_preferences(app);
            match app.path().app_data_dir() {
                Ok(status_dir) => {
                    let status_path = status_dir.join("activation_status.json");
                    let status = load_activation_status(&status_path);
                    let state: tauri::State<LicenseState> = app.state();
                    match prepare_activation_repository(app.handle()) {
                        Ok(Some(repository)) => {
                            let needs_activation = !status.activated;
                            state.initialize(repository, status_path.clone(), status.activated);
                            if needs_activation {
                                open_activation_window(app.handle());
                            } else {
                                show_main_window_now(app.handle());
                            }
                        }
                        Ok(None) => {
                            state.disable();
                            println!("activation repository unavailable; continuing without activation gate");
                            show_main_window_now(app.handle());
                        }
                        Err(err) => {
                            state.disable();
                            println!("activation repository initialisation failed: {err}");
                            show_main_window_now(app.handle());
                        }
                    }
                }
                Err(err) => {
                    let state: tauri::State<LicenseState> = app.state();
                    state.disable();
                    println!("failed to determine data directory; activation disabled ({err})");
                    show_main_window_now(app.handle());
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
