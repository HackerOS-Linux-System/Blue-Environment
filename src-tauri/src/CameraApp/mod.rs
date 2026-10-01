use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Serialize, Clone)]
pub struct CameraDevice {
    pub path: String,
    pub name: String,
}

fn pictures_dir() -> PathBuf {
    if let Ok(o) = Command::new("xdg-user-dir").arg("PICTURES").output() {
        let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !p.is_empty() && p != "/" { return PathBuf::from(p); }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Pictures")
}

fn videos_dir() -> PathBuf {
    if let Ok(o) = Command::new("xdg-user-dir").arg("VIDEOS").output() {
        let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !p.is_empty() && p != "/" { return PathBuf::from(p); }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Videos")
}

/// Lists `/dev/video*` nodes. Where `v4l2-ctl` is available we also pull a
/// human-readable device name; otherwise we just fall back to the device
/// path itself so the picker is never empty when a camera does exist.
#[tauri::command(async)]
pub fn camera_list_devices() -> Vec<CameraDevice> {
    let mut devices = Vec::new();
    let Ok(entries) = std::fs::read_dir("/dev") else { return devices };

    let mut paths: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("video") { Some(format!("/dev/{}", name)) } else { None }
        })
        .collect();
    paths.sort();

    for path in paths {
        let name = Command::new("v4l2-ctl")
            .args(["-d", &path, "--info"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .and_then(|out| {
                out.lines()
                    .find(|l| l.trim_start().starts_with("Card type"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| path.clone());
        devices.push(CameraDevice { path, name });
    }
    devices
}

/// Whether ffmpeg is even installed — the frontend uses this to show a
/// clear "install ffmpeg" message instead of a generic failure.
#[tauri::command(async)]
pub fn camera_check_available() -> bool {
    Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Grabs a single JPEG frame from `device` and returns it as a base64 data
/// URL. Used both for "viewfinder" polling and for the actual photo
/// shutter button (the shutter just additionally persists the same frame
/// to disk).
#[tauri::command(async)]
pub fn camera_capture_frame(device: String, width: u32, height: u32) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("blue-camera-frame-{}.jpg", std::process::id()));

    let output = Command::new("ffmpeg")
        .args([
            "-y", "-loglevel", "error",
            "-f", "v4l2",
            "-video_size", &format!("{}x{}", width, height),
            "-i", &device,
            "-frames:v", "1",
        ])
        .arg(&tmp)
        .output()
        .map_err(|e| format!("ffmpeg not available: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)))
}

/// Captures a photo and saves it permanently under ~/Pictures, returning
/// the absolute path so the frontend can show/open it.
#[tauri::command(async)]
pub fn camera_capture_photo(device: String, width: u32, height: u32) -> Result<String, String> {
    let dir = pictures_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let filename = format!("photo-{}.jpg", chrono::Local::now().format("%Y-%m-%d-%H%M%S"));
    let path = dir.join(filename);

    let output = Command::new("ffmpeg")
        .args([
            "-y", "-loglevel", "error",
            "-f", "v4l2",
            "-video_size", &format!("{}x{}", width, height),
            "-i", &device,
            "-frames:v", "1",
        ])
        .arg(&path)
        .output()
        .map_err(|e| format!("ffmpeg not available: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(path.to_string_lossy().to_string())
}

/// Records `duration_secs` of video from `device` to ~/Videos (used for
/// the timer-based video mode). Recording is intentionally bounded by a
/// duration rather than an open-ended start/stop, since stopping an
/// in-flight ffmpeg child process cleanly from the frontend would need a
/// process-handle registry; a fixed countdown matches how the existing UI
/// (3s/5s/10s timer buttons) already behaves for photos and keeps this
/// simple and reliable.
#[tauri::command]
pub async fn camera_record_video(device: String, width: u32, height: u32, duration_secs: u32) -> Result<String, String> {
    let dir = videos_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let filename = format!("video-{}.mp4", chrono::Local::now().format("%Y-%m-%d-%H%M%S"));
    let path = dir.join(filename);
    let path_str = path.to_string_lossy().to_string();

    let output = tokio::process::Command::new("ffmpeg")
        .args([
            "-y", "-loglevel", "error",
            "-f", "v4l2",
            "-video_size", &format!("{}x{}", width, height),
            "-i", &device,
            "-t", &duration_secs.to_string(),
            "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
        ])
        .arg(&path)
        .output()
        .await
        .map_err(|e| format!("ffmpeg not available: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(path_str)
}
