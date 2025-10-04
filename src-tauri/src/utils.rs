use std::env;
use tauri::{AppHandle, Manager};

pub fn toggle_webview_devtools(app: &AppHandle) {
    #[cfg(debug_assertions)] // only include this code on debug builds
    {
        let window = app.get_webview_window("main").unwrap();

        if !window.is_devtools_open() {
            window.open_devtools();
        } else {
            window.close_devtools();
        }
    }
}

pub fn get_env_key(key_name: &str) -> String {
    env::var(key_name).unwrap_or_else(|_| {
        eprintln!("环境变量 {} 未设置，请设置后重试", key_name);
        std::process::exit(1);
    })
}
