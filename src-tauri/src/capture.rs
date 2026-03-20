use crate::config::{AppState, DirectionEnum};
use crate::utils::write_some_log;
use fs_extra::dir;
use image::ImageFormat;
use std::io::Cursor;
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

fn get_primary_monitor() -> Result<Monitor, String> {
    let monitors =
        Monitor::all().map_err(|err| format!("failed to enumerate monitors: {err}"))?;

    monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or_else(|| "No primary monitor found".to_string())
}

fn get_monitor_dimensions(monitor: &Monitor) -> Result<(u32, u32), String> {
    let monitor_width = monitor
        .width()
        .map_err(|err| format!("failed to read monitor width: {err}"))?;
    let monitor_height = monitor
        .height()
        .map_err(|err| format!("failed to read monitor height: {err}"))?;
    Ok((monitor_width, monitor_height))
}

fn get_capture_direction(states: &State<AppState>) -> Result<DirectionEnum, String> {
    states
        .capture_position
        .lock()
        .map(|guard| *guard)
        .map_err(|_| "capture position lock poisoned".to_string())
}

#[tauri::command]
pub fn get_screen_capture_to_bytes(
    states: State<AppState>,
    _app: tauri::AppHandle,
) -> Result<Vec<u8>, String> {
    let monitor = get_primary_monitor()?;
    let (monitor_width, monitor_height) = get_monitor_dimensions(&monitor)?;

    let direction = get_capture_direction(&states)?;

    let (x, y, w, h) = get_region(monitor_width, monitor_height, &direction);
    let image = monitor
        .capture_region(x as u32, y as u32, w, h)
        .map_err(|err| format!("capture_region failed: {err}"))?;

    #[cfg(target_os = "windows")]
    {
        if let Err(err) = dir::create_all("assets", true) {
            let message = format!("failed to create assets directory: {err}");
            write_some_log(&message);
            return Err(message);
        }

        let monitor_name = monitor
            .name()
            .unwrap_or_else(|_| "unknown".to_string());
        let file_path = format!(
            "assets/monitor-{}-{:?}.png",
            normalized(monitor_name),
            &direction
        );
        if let Err(err) = image.save(&file_path) {
            let message = format!("failed to save capture: {err}");
            write_some_log(&message);
            return Err(message);
        }
        let mut buf = Cursor::new(Vec::new());
        if let Err(err) = image.write_to(&mut buf, ImageFormat::Png) {
            let message = format!("failed to encode capture: {err}");
            write_some_log(&message);
            return Err(message);
        }
        Ok(buf.into_inner())
    }

    #[cfg(target_os = "macos")]
    {
        let log_dir = dirs::data_dir().unwrap().join("interview_coder_app");
        let assets = log_dir.join("assets");
        if let Err(err) = dir::create_all(assets.as_path(), false) {
            let message = format!("failed to create assets directory: {err}");
            write_some_log(&message);
            return Err(message);
        }

        let monitor_name = monitor
            .name()
            .unwrap_or_else(|| "unknown".to_string());
        let file_path = format!(
            "monitor-{}-{:?}.png",
            normalized(monitor_name),
            &direction
        );
        let file_path = assets.join(file_path);
        write_some_log(file_path.to_str().unwrap());

        if let Err(err) = image.save(&file_path) {
            let message = format!("failed to save capture: {err}");
            write_some_log(&message);
            return Err(message);
        }
        let mut buf = Cursor::new(Vec::new());
        if let Err(err) = image.write_to(&mut buf, ImageFormat::Png) {
            let message = format!("failed to encode capture: {err}");
            write_some_log(&message);
            return Err(message);
        }
        Ok(buf.into_inner())
    }
}

#[tauri::command]
pub fn get_screen_capture_to_path(states: State<AppState>) -> Result<String, String> {
    let monitor = get_primary_monitor()?;
    if let Err(err) = dir::create_all("assets", true) {
        let message = format!("failed to create assets directory: {err}");
        write_some_log(&message);
        return Err(message);
    }

    let (monitor_width, monitor_height) = get_monitor_dimensions(&monitor)?;

    let direction = get_capture_direction(&states)?;

    let (x, y, w, h) = get_region(monitor_width, monitor_height, &direction);
    let image = monitor
        .capture_region(x as u32, y as u32, w, h)
        .map_err(|err| format!("capture_region failed: {err}"))?;

    let monitor_name = monitor
        .name()
        .unwrap_or_else(|_| "unknown".to_string());
    let file_path = format!(
        "assets/monitor-{}-{:?}.png",
        normalized(monitor_name),
        &direction
    );
    if let Err(err) = image.save(&file_path) {
        let message = format!("failed to save capture: {err}");
        write_some_log(&message);
        return Err(message);
    }
    std::fs::canonicalize(&file_path)
        .map_err(|err| format!("failed to get absolute path: {err}"))?
        .to_str()
        .ok_or_else(|| "failed to convert path to string".to_string())
        .map(|value| value.to_string())
}

#[test]
fn test_prod_asset_file() {
    let log_dir = dirs::data_dir().unwrap().join("interview_coder_app");
    let assets = log_dir.join("assets");
    println!("assets dir: {}", assets.display());
}
