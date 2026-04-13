use crate::config::{
    open_language_selector, persist_page_opacity, toggle_background_broadcast, toggle_vlm_model,
};
use crate::utils::{is_dev, toggle_webview_devtools};
use crate::{app_debug, app_error, app_info};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Position, Wry};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

static ACTIVATION_SHORTCUT_REGISTERED: AtomicBool = AtomicBool::new(false);
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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

    let quit_shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Digit5);
    let toggle_model_shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Digit4);

    app.handle()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, shortcut, event| {
                    if shortcut == &hide_or_show_shortcut {
                        match event.state() {
                            ShortcutState::Pressed => {
                                app_debug!("system", "shortcut pressed: {:?}", shortcut);
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
                        match event.state() {
                            ShortcutState::Pressed => {
                                app_debug!("system", "shortcut pressed: {:?}", shortcut);
                            }
                            ShortcutState::Released => toggle_webview_devtools(_app),
                        }
                    } else if shortcut == &open_language_window {
                        match event.state() {
                            ShortcutState::Pressed => {
                                app_debug!("system", "shortcut pressed: {:?}", shortcut);
                            }
                            ShortcutState::Released => open_language_selector(_app),
                        }
                    } else if shortcut == &quit_shortcut {
                        match event.state() {
                            ShortcutState::Pressed => {
                                app_debug!("system", "shortcut pressed: {:?}", shortcut);
                            }
                            ShortcutState::Released => graceful_exit(_app),
                        }
                    } else if shortcut == &toggle_model_shortcut {
                        match event.state() {
                            ShortcutState::Pressed => {
                                app_debug!("system", "shortcut pressed: {:?}", shortcut);
                            }
                            ShortcutState::Released => match toggle_vlm_model(_app) {
                                Ok(model) => {
                                    app_info!("system", "VLM model switched to {model:?}");
                                    if is_dev() {
                                        _app.notification()
                                            .builder()
                                            .title("Tauri")
                                            .body(format!("Tauri VLM Model is {model:?}"))
                                            .show()
                                            .unwrap()
                                    }
                                }
                                Err(err) => {
                                    app_error!("system", "{err}");
                                    if is_dev() {
                                        _app.notification()
                                            .builder()
                                            .title("Tauri")
                                            .body(err)
                                            .show()
                                            .unwrap()
                                    }
                                }
                            },
                        }
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
    app.global_shortcut()
        .register(toggle_model_shortcut)
        .unwrap();
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
        app_error!("system", "failed to register activation shortcut: {err}");
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

pub fn show_about_dialog(app: &AppHandle) {
    let message = format!(
        "🌈 **Interview Coder v{}**\n\
        👤 作者: SuperWindcloud\n\
        💬 这是一个面向笔试答题场景的桌面应用。\n\n\
        🌐 联系: ss1178933440@gmail.com \n\n\
        ✨ 完全透视。真正隐形。\n\
        即使你的鼠标悬停或点击 InterviewCoder，系统和应用也不会检测到。\n\
        没有焦点转移，没有标记，没有痕迹。",
        APP_VERSION
    );

    let dialog = app.dialog();

    let _ = dialog
        .message(message)
        .title("关于 Interview Coder")
        .blocking_show();
}
pub fn create_tray_icon(app: &mut App<Wry>) {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let initial_background_broadcast = {
        let state: tauri::State<crate::config::AppState> = app.state();
        *state.background_broadcast.lock().unwrap()
    };

    let quit_i = MenuItem::with_id(app, "quit", "退出", true, Some("Alt+5")).unwrap();
    let code_language =
        MenuItem::with_id(app, "code_language", "偏好设置", true, Some("Alt+3")).unwrap();
    let toggle_model =
        MenuItem::with_id(app, "toggle_model", "切换模型", true, Some("Alt+4")).unwrap();
    let shortcut_capture =
        MenuItem::with_id(app, "shortcut_capture", "截图", false, Some("Alt+1")).unwrap();
    let shortcut_answer =
        MenuItem::with_id(app, "shortcut_answer", "答案", false, Some("Alt+2")).unwrap();
    let shortcut_preferences =
        MenuItem::with_id(app, "shortcut_preferences", "偏好设置", false, Some("Alt+3")).unwrap();
    let shortcut_model =
        MenuItem::with_id(app, "shortcut_model", "切换模型", false, Some("Alt+4")).unwrap();
    let shortcut_exit =
        MenuItem::with_id(app, "shortcut_exit", "退出", false, Some("Alt+5")).unwrap();
    let shortcut_stop_audio =
        MenuItem::with_id(
            app,
            "shortcut_stop_audio",
            "暂停/恢复播音",
            false,
            Some("Alt+Space"),
        )
            .unwrap();
    let shortcut_move =
        MenuItem::with_id(app, "shortcut_move", "移动窗口", false, Some("Alt+↑↓←→")).unwrap();
    let shortcut_reset =
        MenuItem::with_id(app, "shortcut_reset", "重置窗口", false, Some("Alt+`")).unwrap();
    let shortcut_hide = MenuItem::with_id(
        app,
        "shortcut_hide",
        "显示/隐藏窗口",
        false,
        Some("Ctrl+Shift+`"),
    )
    .unwrap();
    let background_broadcast = CheckMenuItem::with_id(
        app,
        "background_broadcast",
        "后台播音",
        true,
        initial_background_broadcast,
        None::<&str>,
    )
    .unwrap();
    let background_broadcast_item = background_broadcast.clone();
    let about_item = MenuItem::with_id(app, "about", "关于", true, Some("")).unwrap();
    let opacity_100 = MenuItem::with_id(app, "page_opacity_100", "100%", true, None::<&str>)
        .unwrap();
    let opacity_90 =
        MenuItem::with_id(app, "page_opacity_90", "90%", true, None::<&str>).unwrap();
    let opacity_80 =
        MenuItem::with_id(app, "page_opacity_80", "80%", true, None::<&str>).unwrap();
    let opacity_70 =
        MenuItem::with_id(app, "page_opacity_70", "70%", true, None::<&str>).unwrap();
    let opacity_60 =
        MenuItem::with_id(app, "page_opacity_60", "60%", true, None::<&str>).unwrap();
    let opacity_50 =
        MenuItem::with_id(app, "page_opacity_50", "50%", true, None::<&str>).unwrap();
    let opacity_40 =
        MenuItem::with_id(app, "page_opacity_40", "40%", true, None::<&str>).unwrap();
    let opacity_30 =
        MenuItem::with_id(app, "page_opacity_30", "30%", true, None::<&str>).unwrap();
    let opacity_20 =
        MenuItem::with_id(app, "page_opacity_20", "20%", true, None::<&str>).unwrap();
    let page_opacity_submenu = Submenu::with_items(
        app,
        "页面透明度",
        true,
        &[
            &opacity_100,
            &opacity_90,
            &opacity_80,
            &opacity_70,
            &opacity_60,
            &opacity_50,
            &opacity_40,
            &opacity_30,
            &opacity_20,
        ],
    )
    .unwrap();
    let shortcut_help_submenu = Submenu::with_items(
        app,
        "快捷键帮助",
        true,
        &[
            &shortcut_capture,
            &shortcut_answer,
            &shortcut_stop_audio,
            &shortcut_preferences,
            &shortcut_model,
            &shortcut_exit,
            &shortcut_move,
            &shortcut_reset,
            &shortcut_hide,
        ],
    )
    .unwrap();
    let menu = Menu::with_items(
        app,
        &[
            &about_item,
            &shortcut_help_submenu,
            &code_language,
            &toggle_model,
            &background_broadcast,
            &page_opacity_submenu,
            &quit_i,
        ],
    )
    .unwrap();

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "quit" => {
                graceful_exit(app);
            }
            "code_language" => {
                open_language_selector(app);
            }
            "toggle_model" => match toggle_vlm_model(app) {
                Ok(model) => {
                    app_info!("system", "VLM model switched to {model:?}");
                    if is_dev() {
                        app.notification()
                            .builder()
                            .title("Tauri")
                            .body(format!("Tauri VLM Model is {model:?}"))
                            .show()
                            .unwrap()
                    }
                }
                Err(err) => {
                    app_error!("system", "{err}");
                    if is_dev() {
                        app.notification()
                            .builder()
                            .title("Tauri")
                            .body(err)
                            .show()
                            .unwrap()
                    }
                }
            },
            "background_broadcast" => match toggle_background_broadcast(app) {
                Ok(enabled) => {
                    let _ = background_broadcast_item.set_checked(enabled);
                    app_info!("system", "background broadcast changed to {}", enabled);
                }
                Err(err) => {
                    app_error!("system", "{err}");
                }
            },
            "page_opacity_100" => apply_page_opacity_from_tray(app, 1.0),
            "page_opacity_90" => apply_page_opacity_from_tray(app, 0.9),
            "page_opacity_80" => apply_page_opacity_from_tray(app, 0.8),
            "page_opacity_70" => apply_page_opacity_from_tray(app, 0.7),
            "page_opacity_60" => apply_page_opacity_from_tray(app, 0.6),
            "page_opacity_50" => apply_page_opacity_from_tray(app, 0.5),
            "page_opacity_40" => apply_page_opacity_from_tray(app, 0.4),
            "page_opacity_30" => apply_page_opacity_from_tray(app, 0.3),
            "page_opacity_20" => apply_page_opacity_from_tray(app, 0.2),
            "about" => show_about_dialog(app),
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

fn apply_page_opacity_from_tray(app: &AppHandle, opacity: f64) {
    match persist_page_opacity(app, opacity) {
        Ok(value) => {
            app_info!("system", "page opacity changed to {}", value);
            if is_dev() {
                let _ = app
                    .notification()
                    .builder()
                    .title("Tauri")
                    .body(format!("页面透明度已调整为 {}%", (value * 100.0) as i32))
                    .show();
            }
        }
        Err(err) => {
            app_error!("system", "{err}");
            if is_dev() {
                let _ = app.notification().builder().title("Tauri").body(err).show();
            }
        }
    }
}
