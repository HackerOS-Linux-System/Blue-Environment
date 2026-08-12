use std::fs;
use std::path::PathBuf;
use std::process::Command;
use glob::glob;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[tauri::command]
pub fn get_wallpapers() -> Vec<String> {
    let mut wallpapers: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let default_path = std::path::Path::new("/usr/share/Blue-Environment/wallpapers/default.png");
    if default_path.exists() {
        wallpapers.push(format!("file://{}", default_path.to_string_lossy()));
        seen.insert("default.png".to_string());
    }

    let patterns = [
        "/usr/share/Blue-Environment/wallpapers/*.png",
        "/usr/share/Blue-Environment/wallpapers/*.jpg",
        "/usr/share/wallpapers/*.png",
        "/usr/share/wallpapers/*.jpg",
        "/usr/share/backgrounds/*.png",
        "/usr/share/backgrounds/*.jpg",
    ];

    for pat in &patterns {
        if let Ok(entries) = glob(pat) {
            for entry in entries.filter_map(Result::ok) {
                let fname = entry.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if !seen.contains(&fname) {
                    seen.insert(fname.clone());
                    wallpapers.push(format!("file://{}", entry.to_string_lossy()));
                }
            }
        }
    }
    if wallpapers.is_empty() {
        wallpapers.push("file:///usr/share/Blue-Environment/wallpapers/default.png".to_string());
    }
    wallpapers
}

#[tauri::command]
pub fn get_wallpaper_preview(path: String) -> Result<String, String> {
    let path = PathBuf::from(path.replace("file://", ""));
    if !path.exists() { return Err("File not found".to_string()); }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{};base64,{}", mime, BASE64.encode(data)))
}

#[tauri::command]
pub fn load_distro_info() -> std::collections::HashMap<String, String> {
    let mut info = std::collections::HashMap::new();
    info.insert("Name".to_string(), "LegendaryOS".to_string());
    info.insert("Version".to_string(), "0.6".to_string());
    info.insert("Copyright".to_string(), "© 2026 LegendaryOS Team".to_string());
    for p in &["/etc/xdg/kcm-about-distrorc", "/etc/os-release"] {
        if let Ok(content) = fs::read_to_string(p) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    info.entry(k.trim().to_string()).or_insert(v.trim_matches('"').to_string());
                }
            }
            break;
        }
    }
    info
}

#[tauri::command]
pub fn system_power(action: String) {
    let cmd = match action.as_str() {
        "shutdown"  => "shutdown -h now",
        "reboot"    => "reboot",
        "logout"    => "pkill -u $(whoami)",
        "suspend"   => "systemctl suspend",
        "hibernate" => "systemctl hibernate",
        _ => return,
    };
    let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
}
