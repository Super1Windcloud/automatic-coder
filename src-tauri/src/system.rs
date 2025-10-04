use crate::config::open_language_selector;
use crate::utils::toggle_webview_devtools;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager, Position, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
pub fn show_window(window: tauri::Window) -> Result<(), String> {
    if window.is_visible().unwrap() {
        return Ok(());
    }

    window
        .set_position(Position::Logical((100, 50).into()))
        .unwrap();
    window.set_ignore_cursor_events(true).unwrap();
    window
        .show()
        .map_err(|e| format!("Failed to show window: {}", e))?;

    use window_vibrancy::*;

    #[cfg(target_os = "macos")]
    apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None)
        .expect("Unsupported platform! 'apply_vibrancy' is only supported on macOS");

    #[cfg(target_os = "windows")]
    apply_acrylic(&window, None)
        .expect("Unsupported platform! 'apply_blur' is only supported on Windows");

    Ok(())
}

pub fn create_shortcut(app: &mut App<Wry>) {
    let hide_or_show_shortcut =
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyQ);
    let toggle_dev_tools_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::F12);

    let open_language_window =
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyP);
    app.handle()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, shortcut, event| {
                    if shortcut == &hide_or_show_shortcut {
                        match event.state() {
                            ShortcutState::Pressed => {
                                println!("{:?}", shortcut);
                            }
                            ShortcutState::Released => {
                                let window = _app.get_webview_window("main").unwrap();
                                if window.is_visible().unwrap() {
                                    window.hide().unwrap();
                                } else {
                                    window.show().unwrap();
                                }
                            }
                        }
                    } else if shortcut == &toggle_dev_tools_shortcut {
                        toggle_webview_devtools(_app)
                    } else if shortcut == &open_language_window {
                        open_language_selector(_app)
                    }
                })
                .build(),
        )
        .unwrap();

    app.global_shortcut()
        .register(hide_or_show_shortcut)
        .unwrap();
    app.global_shortcut()
        .register(toggle_dev_tools_shortcut)
        .unwrap();
    app.global_shortcut()
        .register(open_language_window)
        .unwrap();
}

pub fn create_tray_icon(app: &mut App<Wry>) {
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>).unwrap();
    let code_language =
        MenuItem::with_id(app, "code_language", "偏好设置", true, Some("CTRL+SHIFT+P")).unwrap();
    let menu = Menu::with_items(app, &[&code_language, &quit_i]).unwrap();

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "code_language" => {
                open_language_selector(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)
        .unwrap();
}
