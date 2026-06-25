use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// ── FileEntry ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: String,
    pub mime_type: String,
    pub modified: Option<String>,
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png"|"jpg"|"jpeg"|"gif"|"webp"|"svg"|"avif"|"bmp" => "image",
        "mp4"|"mkv"|"webm"|"avi"|"mov"|"flv"|"wmv"         => "video",
        "mp3"|"wav"|"ogg"|"flac"|"aac"|"opus"              => "audio",
        "pdf"                                               => "application/pdf",
        "txt"|"md"|"rs"|"ts"|"js"|"tsx"|"jsx"|"py"|"rb"|
        "go"|"toml"|"yaml"|"yml"|"json"|"xml"|"sh"|"css"|
        "html"|"ini"|"conf"|"log"                           => "text",
        _                                                   => "application/octet-stream",
    }
}

// ── Path resolution ─────────────────────────────────────────────────────────

/// Resolves paths that start with the `HOME` sentinel used throughout
/// the frontend's default config (e.g. `"HOME/Desktop"`) or `~`.
pub fn resolve_path(path: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if path == "HOME" {
        home
    } else if let Some(rest) = path.strip_prefix("HOME/") {
        home.join(rest)
    } else if path == "~" {
        home
    } else if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(path)
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_files(path: String) -> Vec<FileEntry> {
    let target = resolve_path(&path);
    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(&target) {
        for entry in rd.flatten() {
            if let Ok(meta) = entry.metadata() {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = meta.is_dir();
                let size = if is_dir { "DIR".to_string() } else {
                    format!("{:.1} KB", meta.len() as f64 / 1024.0)
                };
                let ext = entry.path().extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let mime_type = if is_dir { "inode/directory".to_string() } else { mime_for_ext(&ext).to_string() };
                let modified = meta.modified().ok().map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M").to_string()
                });
                entries.push(FileEntry {
                    name,
                    path: entry.path().to_string_lossy().to_string(),
                    is_dir, size, mime_type, modified,
                });
            }
        }
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    entries
}

#[tauri::command]
pub fn read_text_file(path: String) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| format!("Error: {}", e))
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&path).parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_folder(path: String, name: String) -> Result<(), String> {
    fs::create_dir_all(resolve_path(&path).join(name)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_file(path: String) -> Result<(), String> {
    let p = resolve_path(&path);
    if p.is_dir() { fs::remove_dir_all(p) } else { fs::remove_file(p) }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn copy_file(src: String, dest: String) -> Result<(), String> {
    fs::copy(resolve_path(&src), resolve_path(&dest)).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_file(src: String, dest: String) -> Result<(), String> {
    fs::rename(resolve_path(&src), resolve_path(&dest)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_text_file(path: String, name: String, content: String) -> Result<(), String> {
    let p = resolve_path(&path).join(name);
    if let Some(parent) = p.parent() { fs::create_dir_all(parent).ok(); }
    fs::write(p, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_file_as_data_url(path: String) -> Result<String, String> {
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let ext = PathBuf::from(&path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mime = match ext.as_str() {
        "jpg"|"jpeg" => "image/jpeg",
        "png"        => "image/png",
        "gif"        => "image/gif",
        "svg"        => "image/svg+xml",
        "webp"       => "image/webp",
        _            => "application/octet-stream",
    };
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    Ok(format!("data:{};base64,{}", mime, STANDARD.encode(bytes)))
}

#[tauri::command]
pub fn get_default_desktop_path() -> String {
    if let Ok(o) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if o.status.success() {
            let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !p.is_empty() && p != "/" { return p; }
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    for dir in &["Desktop", "Pulpit"] {
        let d = home.join(dir);
        if d.exists() { return d.to_string_lossy().to_string(); }
    }
    let d = home.join("Desktop");
    let _ = fs::create_dir_all(&d);
    d.to_string_lossy().to_string()
}

#[tauri::command]
pub fn get_home_path() -> String {
    dirs::home_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "/root".to_string())
}

#[tauri::command]
pub fn get_username() -> String {
    if let Ok(u) = std::env::var("USER") { if !u.is_empty() { return u; } }
    if let Ok(u) = std::env::var("LOGNAME") { if !u.is_empty() { return u; } }
    Command::new("whoami").output()
        .ok()
        .and_then(|o| {
            let u = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if u.is_empty() { None } else { Some(u) }
        })
        .unwrap_or_else(|| "user".to_string())
}

#[tauri::command]
pub fn get_hostname() -> String {
    if let Ok(o) = Command::new("hostname").output() {
        let h = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !h.is_empty() { return h; }
    }
    if let Ok(h) = fs::read_to_string("/etc/hostname") {
        let h = h.trim().to_string();
        if !h.is_empty() { return h; }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string())
}

#[tauri::command]
pub fn pick_file(filters: Option<String>) -> Option<String> {
    // Fallback backend picker (Tauri's plugin-dialog is preferred on the
    // frontend; this is only reached when the JS plugin isn't available).
    let _ = filters;
    None
}

#[tauri::command]
pub fn pick_directory() -> Option<String> {
    None
}

#[tauri::command]
pub fn git_status(path: String) -> Vec<String> {
    Command::new("git")
        .args(["-C", &path, "status", "--short"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.to_string()).collect())
        .unwrap_or_default()
}
