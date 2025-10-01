use tauri::menu::MenuItem;
use tauri::tray::{SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem, TrayIcon};
use tauri::{Builder, Manager, Wry};
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
    let tray_menu = SystemTrayMenu::new()
        .add_item(SystemTrayMenuItem::Separator)
        .add_item(MenuItem::Quit); // 直接用内置的退出

    tauri::Builder::default()
        .tray(SystemTray::new().with_menu(tray_menu))
        .on_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => {
                if id == "quit" {
                    std::process::exit(0);
                }
            }
            _ => {}
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![show_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
