use crate::config::{
    open_custom_openai_selector, open_language_selector, persist_background_broadcast,
    persist_custom_openai_enabled, persist_page_opacity, toggle_vlm_model,
};
use crate::license::{host_management_available, open_host_management_window};
use crate::lan::current_lan_urls;
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

pub fn show_help_dialog(app: &AppHandle) {
    let lan_section = {
        let urls = current_lan_urls(app);
        if urls.is_empty() {
            "局域网答案页\n- 服务启动后会监听 37999-38009 之间的可用端口\n- 完整访问地址可在应用启动日志中查看".to_string()
        } else {
            format!(
                "局域网答案页\n- 可直接在同一局域网设备访问以下地址：\n- {}\n- 默认端口范围为 37999-38009",
                urls.join("\n- ")
            )
        }
    };

    let message = format!(
        "Interview Coder v{}\n\n\
功能说明\n\
- 截图识别题目并生成答案\n\
- 答案窗口悬浮显示\n\
- 支持后台播音\n\
- 支持切换视觉模型\n\
- 支持自定义 OpenAI 兼容接口\n\
- 启动时自动开启局域网答案页\n\n\
快捷键\n\
- Alt+1: 截图\n\
- Alt+2: 生成答案\n\
- Alt+3: 打开偏好设置\n\
- Alt+4: 切换模型\n\
- Alt+5: 退出应用\n\
- Alt+Space: 暂停/恢复播音\n\
- Alt+↑↓←→: 移动窗口\n\
- Alt+`: 重置窗口\n\
- Ctrl+Shift+`: 显示/隐藏窗口\n\n\
补充说明\n\
- 后台播音开启后，前端窗口会隐藏，Alt+2 改为扬声器播报答案\n\
- 已启用自定义 OpenAI 兼容 API 时，切换模型菜单会被禁用\n\
- {}",
        APP_VERSION,
        lan_section
    );

    let _ = app
        .dialog()
        .message(message)
        .title("帮助信息")
        .blocking_show();
}

pub fn create_tray_icon(app: &mut App<Wry>) {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let initial_background_broadcast = {
        let state: tauri::State<crate::config::AppState> = app.state();
        *state.background_broadcast.lock().unwrap()
    };
    let initial_custom_openai_enabled = {
        let state: tauri::State<crate::config::AppState> = app.state();
        *state.custom_openai_enabled.lock().unwrap()
    };
    let initial_page_opacity = {
        let state: tauri::State<crate::config::AppState> = app.state();
        *state.page_opacity.lock().unwrap()
    };

    let quit_i = MenuItem::with_id(app, "quit", "退出", true, Some("Alt+5")).unwrap();
    let code_language =
        MenuItem::with_id(app, "code_language", "偏好设置", true, Some("Alt+3")).unwrap();
    let toggle_model = MenuItem::with_id(
        app,
        "toggle_model",
        "切换模型",
        !initial_custom_openai_enabled,
        Some("Alt+4"),
    )
    .unwrap();
    let toggle_model_item = toggle_model.clone();
    let custom_openai_settings = MenuItem::with_id(
        app,
        "custom_openai_settings",
        "自定义 OpenAI 接口配置",
        true,
        None::<&str>,
    )
    .unwrap();
    let host_management =
        MenuItem::with_id(app, "host_management", "本地宿主管理", true, None::<&str>).unwrap();
    let help_item = MenuItem::with_id(app, "help", "帮助信息", true, None::<&str>).unwrap();
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
    let custom_openai_enabled = CheckMenuItem::with_id(
        app,
        "custom_openai_enabled",
        "启用自定义 OpenAI 兼容 API",
        true,
        initial_custom_openai_enabled,
        None::<&str>,
    )
    .unwrap();
    let custom_openai_enabled_item = custom_openai_enabled.clone();
    let about_item = MenuItem::with_id(app, "about", "关于", true, Some("")).unwrap();
    let opacity_100 = CheckMenuItem::with_id(
        app,
        "page_opacity_100",
        "100%",
        true,
        is_same_opacity(initial_page_opacity, 1.0),
        None::<&str>,
    )
    .unwrap();
    let opacity_90 = CheckMenuItem::with_id(
        app,
        "page_opacity_90",
        "90%",
        true,
        is_same_opacity(initial_page_opacity, 0.9),
        None::<&str>,
    )
    .unwrap();
    let opacity_80 = CheckMenuItem::with_id(
        app,
        "page_opacity_80",
        "80%",
        true,
        is_same_opacity(initial_page_opacity, 0.8),
        None::<&str>,
    )
    .unwrap();
    let opacity_70 = CheckMenuItem::with_id(
        app,
        "page_opacity_70",
        "70%",
        true,
        is_same_opacity(initial_page_opacity, 0.7),
        None::<&str>,
    )
    .unwrap();
    let opacity_60 = CheckMenuItem::with_id(
        app,
        "page_opacity_60",
        "60%",
        true,
        is_same_opacity(initial_page_opacity, 0.6),
        None::<&str>,
    )
    .unwrap();
    let opacity_50 = CheckMenuItem::with_id(
        app,
        "page_opacity_50",
        "50%",
        true,
        is_same_opacity(initial_page_opacity, 0.5),
        None::<&str>,
    )
    .unwrap();
    let opacity_40 = CheckMenuItem::with_id(
        app,
        "page_opacity_40",
        "40%",
        true,
        is_same_opacity(initial_page_opacity, 0.4),
        None::<&str>,
    )
    .unwrap();
    let opacity_30 = CheckMenuItem::with_id(
        app,
        "page_opacity_30",
        "30%",
        true,
        is_same_opacity(initial_page_opacity, 0.3),
        None::<&str>,
    )
    .unwrap();
    let opacity_20 = CheckMenuItem::with_id(
        app,
        "page_opacity_20",
        "20%",
        true,
        is_same_opacity(initial_page_opacity, 0.2),
        None::<&str>,
    )
    .unwrap();
    let opacity_items = vec![
        (opacity_100.clone(), 1.0),
        (opacity_90.clone(), 0.9),
        (opacity_80.clone(), 0.8),
        (opacity_70.clone(), 0.7),
        (opacity_60.clone(), 0.6),
        (opacity_50.clone(), 0.5),
        (opacity_40.clone(), 0.4),
        (opacity_30.clone(), 0.3),
        (opacity_20.clone(), 0.2),
    ];
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
    let menu = if host_management_available(app.handle()) {
        Menu::with_items(
            app,
            &[
                &about_item,
                &help_item,
                &host_management,
                &code_language,
                &toggle_model,
                &custom_openai_settings,
                &custom_openai_enabled,
                &background_broadcast,
                &page_opacity_submenu,
                &quit_i,
            ],
        )
        .unwrap()
    } else {
        Menu::with_items(
            app,
            &[
                &about_item,
                &help_item,
                &code_language,
                &toggle_model,
                &custom_openai_settings,
                &custom_openai_enabled,
                &background_broadcast,
                &page_opacity_submenu,
                &quit_i,
            ],
        )
        .unwrap()
    };

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
            "toggle_model" => {
                let custom_openai_enabled = {
                    let state: tauri::State<crate::config::AppState> = app.state();
                    *state.custom_openai_enabled.lock().unwrap()
                };
                if custom_openai_enabled {
                    app_info!(
                        "system",
                        "toggle model ignored because custom openai is enabled"
                    );
                    return;
                }
                match toggle_vlm_model(app) {
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
                }
            }
            "custom_openai_settings" => {
                open_custom_openai_selector(app);
            }
            "host_management" => {
                open_host_management_window(app);
            }
            "custom_openai_enabled" => match custom_openai_enabled_item.is_checked() {
                Ok(enabled) => match persist_custom_openai_enabled(app, enabled) {
                    Ok(saved_enabled) => {
                        let _ = custom_openai_enabled_item.set_checked(saved_enabled);
                        let _ = toggle_model_item.set_enabled(!saved_enabled);
                        app_info!("system", "custom openai changed to {}", saved_enabled);
                    }
                    Err(err) => {
                        app_error!("system", "{err}");
                    }
                },
                Err(err) => {
                    app_error!("system", "failed to read custom openai menu state: {err}");
                }
            },
            "background_broadcast" => match background_broadcast_item.is_checked() {
                Ok(enabled) => match persist_background_broadcast(app, enabled) {
                    Ok(saved_enabled) => {
                        let _ = background_broadcast_item.set_checked(saved_enabled);
                        app_info!(
                            "system",
                            "background broadcast changed to {}",
                            saved_enabled
                        );
                    }
                    Err(err) => {
                        app_error!("system", "{err}");
                    }
                },
                Err(err) => {
                    app_error!(
                        "system",
                        "failed to read background broadcast menu state: {err}"
                    );
                }
            },
            "page_opacity_100" => apply_page_opacity_from_tray(app, 1.0, &opacity_items),
            "page_opacity_90" => apply_page_opacity_from_tray(app, 0.9, &opacity_items),
            "page_opacity_80" => apply_page_opacity_from_tray(app, 0.8, &opacity_items),
            "page_opacity_70" => apply_page_opacity_from_tray(app, 0.7, &opacity_items),
            "page_opacity_60" => apply_page_opacity_from_tray(app, 0.6, &opacity_items),
            "page_opacity_50" => apply_page_opacity_from_tray(app, 0.5, &opacity_items),
            "page_opacity_40" => apply_page_opacity_from_tray(app, 0.4, &opacity_items),
            "page_opacity_30" => apply_page_opacity_from_tray(app, 0.3, &opacity_items),
            "page_opacity_20" => apply_page_opacity_from_tray(app, 0.2, &opacity_items),
            "help" => show_help_dialog(app),
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

fn is_same_opacity(left: f64, right: f64) -> bool {
    (left - right).abs() < 0.01
}

fn sync_page_opacity_menu_state(opacity_items: &[(CheckMenuItem<Wry>, f64)], opacity: f64) {
    for (item, value) in opacity_items {
        let _ = item.set_checked(is_same_opacity(*value, opacity));
    }
}

fn apply_page_opacity_from_tray(
    app: &AppHandle,
    opacity: f64,
    opacity_items: &[(CheckMenuItem<Wry>, f64)],
) {
    match persist_page_opacity(app, opacity) {
        Ok(value) => {
            sync_page_opacity_menu_state(opacity_items, value);
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
