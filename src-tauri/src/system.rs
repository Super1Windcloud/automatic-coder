use crate::config::open_language_selector;
use crate::utils::toggle_webview_devtools;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Position, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

static ACTIVATION_SHORTCUT_REGISTERED: AtomicBool = AtomicBool::new(false);

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

    Ok(())
}

pub fn create_shortcut(app: &mut App<Wry>) {
    let hide_or_show_shortcut = activation_shortcut_definition();

    #[cfg(target_os = "windows")]
    let toggle_dev_tools_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::F12);
    #[cfg(target_os = "macos")]
    let toggle_dev_tools_shortcut = Shortcut::new(Some(Modifiers::META), Code::F12);

    let open_language_window = Shortcut::new(Some(Modifiers::ALT), Code::Digit3);

    let quit_shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Digit4);

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
                                let config_window =
                                    _app.get_webview_window("code_language_selector");
                                #[allow(clippy::unnecessary_unwrap)]
                                if config_window.is_some() {
                                    let config_window = config_window.unwrap();
                                    if config_window.is_visible().unwrap() {
                                        config_window.hide().unwrap();
                                    } else {
                                        config_window.show().unwrap();
                                    }
                                }
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
                    } else if shortcut == &quit_shortcut {
                        graceful_exit(_app)
                    }
                })
                .build(),
        )
        .unwrap();

    app.global_shortcut()
        .register(toggle_dev_tools_shortcut)
        .unwrap();

    app.global_shortcut()
        .register(open_language_window)
        .unwrap();
    app.global_shortcut().register(quit_shortcut).unwrap();
}

pub fn register_activation_shortcut(app: &AppHandle) {
    if ACTIVATION_SHORTCUT_REGISTERED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let shortcut = activation_shortcut_definition();
    if let Err(err) = app.global_shortcut().register(shortcut) {
        println!("failed to register activation shortcut: {err}");
        ACTIVATION_SHORTCUT_REGISTERED.store(false, Ordering::SeqCst);
    }
}

fn activation_shortcut_definition() -> Shortcut {
    #[cfg(target_os = "windows")]
    {
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Backquote)
    }
    #[cfg(target_os = "macos")]
    {
        return Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT), Code::Backquote);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        return Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Backquote);
    }
}

fn graceful_exit(app: &AppHandle) {
    for window in app.webview_windows().values() {
        let _ = window.hide();
        let _ = window.close();
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    app.exit(0);
}

pub fn create_tray_icon(app: &mut App<Wry>) {
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, Some("Alt+4")).unwrap();
    let code_language =
        MenuItem::with_id(app, "code_language", "偏好设置", true, Some("Alt+3")).unwrap();
    let menu = Menu::with_items(app, &[&code_language, &quit_i]).unwrap();

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                graceful_exit(app);
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
