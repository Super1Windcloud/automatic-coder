use crate::config::PreferencesConfig;
use confy::load as load_config;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

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
            "sk-pakzoefbduyiqonsznhvnyczilfcjwefjolbvjslcliafefk".to_string()
        } else {
            #[cfg(target_os = "macos")]
            if !is_dev() {
                write_some_log(&format!(
                    "环境变量 {} 已设置，值为 {}",
                    key_name, result.vlm_key
                ))
            }
            result.vlm_key.to_string()
        }
    })
}

pub fn toggle_webview_devtools(app: &AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    if !window.is_devtools_open() {
        window.open_devtools();
    } else {
        window.close_devtools();
    }
}

pub fn is_dev() -> bool {
    cfg!(debug_assertions)
}

pub fn write_some_log(msg: &str) {
    #[cfg(target_os = "macos")]
    {
        if let Some(log_dir) = dirs::data_dir() {
            let mut path = PathBuf::from(log_dir.join("interview_coder_app"));
            path.push("interview_coder_app.log");
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(file, "{}", msg);
                let _ = file.flush();
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(mut file) = OpenOptions::new()
            .create(true) // 文件不存在则创建
            .append(true) // 追加写入
            .open("app.log")
        {
            let _ = writeln!(file, "{}", msg);
            let _ = file.flush();
        }
    }
}

#[tauri::command]
pub fn append_app_log(message: String) {
    write_some_log(&message);
}

#[test]
fn test_get_env_key() {
    println!("{:?}", is_dev());
}
