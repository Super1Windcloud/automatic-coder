use crate::config::{AppState, DirectionEnum};
use fs_extra::dir;
use image::ImageFormat;
use std::io::Cursor;
use tauri::path::BaseDirectory;
use tauri::State;
use xcap::Monitor;

fn normalized(filename: String) -> String {
    filename.replace(['|', '\\', ':', '/'], "")
}

fn get_region(
    monitor_width: u32,
    monitor_height: u32,
    direction: &DirectionEnum,
) -> (i32, i32, u32, u32) {
    match direction {
        DirectionEnum::LeftHalf => (0, 0, monitor_width / 2, monitor_height),
        DirectionEnum::RightHalf => (
            (monitor_width / 2) as i32,
            0,
            monitor_width / 2,
            monitor_height,
        ),
        DirectionEnum::UpHalf => (0, 0, monitor_width, monitor_height / 2),
        DirectionEnum::DownHalf => (
            0,
            (monitor_height / 2) as i32,
            monitor_width,
            monitor_height / 2,
        ),
        DirectionEnum::FullScreen => (0, 0, monitor_width, monitor_height),
    }
}

#[tauri::command]
pub fn get_screen_capture_to_bytes(states: State<AppState>) -> Vec<u8> {
    let monitors = Monitor::all().unwrap();
    dir::create_all("assets", true).unwrap();

    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .expect("No primary monitor found");

    let monitor_width = monitor.width().unwrap();
    let monitor_height = monitor.height().unwrap();

    let direction = *states.capture_position.lock().unwrap();

    let (x, y, w, h) = get_region(monitor_width, monitor_height, &direction);
    let image = monitor.capture_region(x as u32, y as u32, w, h).unwrap();

    #[cfg(target_os = "windows")]
    let file_path = format!(
        "assets/monitor-{}-{:?}.png",
        normalized(monitor.name().unwrap()),
        &direction
    );
    #[cfg(target_os = "macos")]
    {
        let data_dir = app
            .path()
            .resolve("assets", BaseDirectory::AppData)
            .unwrap();
        std::fs::create_dir_all(&data_dir).unwrap();
        let file_path = data_dir.join("monitor.png");
    }

    image.save(&file_path).unwrap();
    let mut buf = Cursor::new(Vec::new());
    image.write_to(&mut buf, ImageFormat::Png).unwrap();
    buf.into_inner()
}

#[tauri::command]
pub fn get_screen_capture_to_path(states: State<AppState>) -> String {
    let monitors = Monitor::all().unwrap();
    dir::create_all("assets", true).unwrap();

    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .expect("No primary monitor found");

    let monitor_width = monitor.width().unwrap();
    let monitor_height = monitor.height().unwrap();

    let direction = *states.capture_position.lock().unwrap();

    let (x, y, w, h) = get_region(monitor_width, monitor_height, &direction);
    let image = monitor.capture_region(x as u32, y as u32, w, h).unwrap();

    let file_path = format!(
        "assets/monitor-{}-{:?}.png",
        normalized(monitor.name().unwrap()),
        &direction
    );
    image.save(&file_path).unwrap();
    std::fs::canonicalize(&file_path)
        .expect("Failed to get absolute path")
        .to_str()
        .unwrap()
        .to_string()
}
