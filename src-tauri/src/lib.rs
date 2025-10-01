use tauri::tray::TrayIconBuilder;
use tauri::Manager;

#[tauri::command]
fn show_window(window: tauri::Window) -> Result<(), String> {
    if window.is_visible().unwrap() {
        return Ok(());
    }
    window.center().unwrap();
    window.show_menu().unwrap();

    window
        .show()
        .map_err(|e| format!("Failed to show window: {}", e))?;
    window
        .set_focus()
        .map_err(|e| format!("Failed to set focus: {}", e))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {




    tauri::Builder::default()
        .setup(|app| {
            let tray = TrayIconBuilder::new().build(app)?;
             Ok(())
         })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![show_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
