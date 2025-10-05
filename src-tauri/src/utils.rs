use crate::config::PreferencesConfig;
use confy::load as load_config;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
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
        let result: PreferencesConfig =
            load_config("interview-coder-config", "preferences").unwrap();

        if result.vlm_key.is_empty() {
            eprintln!("环境变量 {} 未设置，请设置后重试", key_name);
            if !is_dev() {
                write_some_log(&format!("环境变量 {} 未设置，请设置后重试", key_name))
            };
            // open_language_selector(app.handle());
            "".to_string()
        } else {
            result.vlm_key.to_string()
        }
    })
}

pub fn is_dev() -> bool {
    cfg!(debug_assertions)
}

pub fn write_some_log(msg: &str) {
    let mut file = OpenOptions::new()
        .create(true) // 文件不存在则创建
        .append(true) // 追加写入
        .open("app.log") // 日志文件名
        .unwrap();

    writeln!(file, "{}", msg).unwrap(); // 写入一行
}

#[test]
fn test_get_env_key() {
    println!("{:?}", is_dev());
}
