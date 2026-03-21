use crate::config::{AppState, DirectionEnum};
use crate::{app_error, app_info};
use fs_extra::dir;
use image::ImageFormat;
use std::env;
use std::io::Cursor;
use tauri::State;
use xcap::Monitor;

fn normalized(filename: String) -> String {
    filename
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

fn should_skip_save() -> bool {
    matches!(
        env::var("CAPTURE_SKIP_SAVE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

#[cfg(target_os = "windows")]
fn capture_assets_dir() -> Result<std::path::PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "failed to resolve data dir".to_string())?;
    Ok(base.join("interview_coder_app").join("assets"))
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
    let monitors = Monitor::all().map_err(|err| format!("failed to enumerate monitors: {err}"))?;

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
    app_info!("capture", "capture_bytes: begin");
    let monitor = get_primary_monitor()?;
    let (monitor_width, monitor_height) = get_monitor_dimensions(&monitor)?;
    app_info!(
        "capture",
        "capture_bytes: monitor {}x{}",
        monitor_width, monitor_height
    );

    let direction = get_capture_direction(&states)?;
    app_info!("capture", "capture_bytes: direction {:?}", direction);

    let (x, y, w, h) = get_region(monitor_width, monitor_height, &direction);
    app_info!("capture", "capture_bytes: region {x},{y} {w}x{h}");
    let image = monitor
        .capture_region(x as u32, y as u32, w, h)
        .map_err(|err| format!("capture_region failed: {err}"))?;
    app_info!("capture", "capture_bytes: region captured");

    #[cfg(target_os = "windows")]
    {
        let assets_dir = capture_assets_dir()?;
        if let Err(err) = dir::create_all(assets_dir.as_path(), false) {
            let message = format!("failed to create assets directory: {err}");
            app_error!("capture", "{message}");
            return Err(message);
        }

        let monitor_name = monitor.name().unwrap_or_else(|_| "unknown".to_string());
        let file_name = format!("monitor-{}-{:?}.png", normalized(monitor_name), &direction);
        let file_path = assets_dir.join(file_name);
        if should_skip_save() {
            app_info!(
                "capture",
                "capture_bytes: skip save (CAPTURE_SKIP_SAVE=1) {}",
                file_path.display()
            );
        } else {
            app_info!("capture", "capture_bytes: save {}", file_path.display());
            if let Err(err) = image.save(&file_path) {
                let message = format!("failed to save capture: {err}");
                app_error!("capture", "{message}");
                return Err(message);
            }
        }
        let mut buf = Cursor::new(Vec::new());
        app_info!("capture", "capture_bytes: encoding png");
        if let Err(err) = image.write_to(&mut buf, ImageFormat::Png) {
            let message = format!("failed to encode capture: {err}");
            app_error!("capture", "{message}");
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
            app_error!("capture", "{message}");
            return Err(message);
        }

        let monitor_name = monitor.name().unwrap_or_else(|| "unknown".to_string());
        let file_path = format!("monitor-{}-{:?}.png", normalized(monitor_name), &direction);
        let file_path = assets.join(file_path);
        app_info!("capture", "capture_bytes: save {}", file_path.display());

        if let Err(err) = image.save(&file_path) {
            let message = format!("failed to save capture: {err}");
            app_error!("capture", "{message}");
            return Err(message);
        }
        let mut buf = Cursor::new(Vec::new());
        app_info!("capture", "capture_bytes: encoding png");
        if let Err(err) = image.write_to(&mut buf, ImageFormat::Png) {
            let message = format!("failed to encode capture: {err}");
            app_error!("capture", "{message}");
            return Err(message);
        }
        Ok(buf.into_inner())
    }
}

#[tauri::command]
pub fn get_screen_capture_to_path(states: State<AppState>) -> Result<String, String> {
    app_info!("capture", "capture_path: begin");
    let monitor = get_primary_monitor()?;
    if should_skip_save() {
        let message = "capture_path: save disabled by CAPTURE_SKIP_SAVE".to_string();
        app_error!("capture", "{message}");
        return Err(message);
    }
    #[cfg(target_os = "windows")]
    let assets_dir = capture_assets_dir()?;

    #[cfg(target_os = "windows")]
    let create_assets = dir::create_all(assets_dir.as_path(), false);
    #[cfg(not(target_os = "windows"))]
    let create_assets = dir::create_all("assets", true);

    if let Err(err) = create_assets {
        let message = format!("failed to create assets directory: {err}");
        app_error!("capture", "{message}");
        return Err(message);
    }

    let (monitor_width, monitor_height) = get_monitor_dimensions(&monitor)?;
    app_info!(
        "capture",
        "capture_path: monitor {}x{}",
        monitor_width, monitor_height
    );

    let direction = get_capture_direction(&states)?;
    app_info!("capture", "capture_path: direction {:?}", direction);

    let (x, y, w, h) = get_region(monitor_width, monitor_height, &direction);
    app_info!("capture", "capture_path: region {x},{y} {w}x{h}");
    let image = monitor
        .capture_region(x as u32, y as u32, w, h)
        .map_err(|err| format!("capture_region failed: {err}"))?;
    app_info!("capture", "capture_path: region captured");

    let monitor_name = monitor.name().unwrap_or_else(|_| "unknown".to_string());
    let file_name = format!("monitor-{}-{:?}.png", normalized(monitor_name), &direction);
    #[cfg(target_os = "windows")]
    let file_path = assets_dir.join(file_name);
    #[cfg(not(target_os = "windows"))]
    let file_path = std::path::PathBuf::from(format!("assets/{}", file_name));

    app_info!("capture", "capture_path: save {}", file_path.display());
    if let Err(err) = image.save(&file_path) {
        let message = format!("failed to save capture: {err}");
        app_error!("capture", "{message}");
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
    app_info!("capture", "assets dir: {}", assets.display());
}
