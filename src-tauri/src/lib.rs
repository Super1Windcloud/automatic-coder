mod capture;
mod config;
mod system;
mod utils;
mod vlm;

use crate::config::AppState;
use capture::*;
use config::*;
use dotenv::dotenv;
use system::*;
#[allow(unused_imports)]
use utils::*;
use vlm::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenv().ok();

    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            show_window,
            set_selected_language,
            get_screen_capture_to_path,
            get_screen_capture_to_bytes,
            set_capture_position,
            get_store_config,
            set_selected_language_prompt,
            create_screenshot_solution_stream,
            set_vlm_key
        ])
        .setup(|app| {
            create_tray_icon(app);
            create_shortcut(app);
            load_preferences(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
