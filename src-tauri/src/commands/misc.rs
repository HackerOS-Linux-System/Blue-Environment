use crate::types::*;
use crate::{apps, cache, ai, packages, window_tracker};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as TokioCommand;
use serde::{Serialize, Deserialize};
use tauri::Emitter;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[tauri::command]
pub fn list_icon_themes() -> Vec<String> {
    crate::icon_resolver::list_installed_icon_themes()
}

#[tauri::command]
pub fn set_icon_theme(theme: Option<String>) {
    crate::icon_resolver::set_icon_theme(theme);
}

#[tauri::command]
pub fn set_panel_enabled(enabled: bool) -> Result<(), String> {
    println!("Panel enabled: {}", enabled);
    Ok(())
}

// ── Clipboard + data-URL file helpers ───────────────────────────────────────
// These back the frontend's SystemBridge fallbacks (used when the
// @tauri-apps/plugin-clipboard-manager JS bindings aren't available), so they
// shell out to the usual Wayland/X11 CLI tools rather than pulling in a new
// clipboard crate dependency.

fn decode_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let comma = data_url.find(',').ok_or("Invalid data URL")?;
    let (_, b64) = data_url.split_at(comma + 1);
    BASE64.decode(b64).map_err(|e| e.to_string())
}

fn data_url_mime(data_url: &str) -> String {
    data_url
        .strip_prefix("data:")
        .and_then(|rest| rest.split(';').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("image/png")
        .to_string()
}

#[tauri::command]
pub fn clipboard_copy(text: String) -> Result<(), String> {
    use std::io::Write;
    use crate::session::{self, SessionType};

    let mut cmd = match session::detect_session() {
        SessionType::WaylandClient => Command::new("wl-copy"),
        _ => {
            let mut c = Command::new("xclip");
            c.args(["-selection", "clipboard"]);
            c
        }
    };
    let mut child = cmd.stdin(std::process::Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clipboard_paste() -> Result<String, String> {
    use crate::session::{self, SessionType};

    let output = match session::detect_session() {
        SessionType::WaylandClient => Command::new("wl-paste").arg("-n").output(),
        _ => Command::new("xclip").args(["-selection", "clipboard", "-o"]).output(),
    }
    .map_err(|e| e.to_string())?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
pub fn write_clipboard_image(data_url: String) -> Result<(), String> {
    use std::io::Write;
    use crate::session::{self, SessionType};

    let bytes = decode_data_url(&data_url)?;
    let mime = data_url_mime(&data_url);

    let mut cmd = match session::detect_session() {
        SessionType::WaylandClient => {
            let mut c = Command::new("wl-copy");
            c.args(["--type", mime.as_str()]);
            c
        }
        _ => {
            let mut c = Command::new("xclip");
            c.args(["-selection", "clipboard", "-t", mime.as_str(), "-i"]);
            c
        }
    };
    let mut child = cmd.stdin(std::process::Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn save_file_from_data_url(path: String, data_url: String) -> Result<(), String> {
    let bytes = decode_data_url(&data_url)?;
    if let Some(parent) = PathBuf::from(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&path, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_config_file(filename: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".config/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_config_file(filename: String, content: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".config/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_cache_file(filename: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".cache/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_cache_file(filename: String, content: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".cache/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn start_panel() {
    let panel_path = std::env::current_exe()
    .unwrap_or_default()
    .parent()
    .unwrap_or(std::path::Path::new("/"))
    .join("blue-panel");
    if panel_path.exists() {
        let _ = Command::new(panel_path).spawn();
    }
}

/// Connects to the compositor's Unix socket and relays messages as Tauri events.
/// Runs in a background thread — reconnects every 2 seconds if the compositor
/// is not yet running or the connection drops.
pub fn compositor_ipc_relay(app: tauri::AppHandle) {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let socket_path = {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        std::path::PathBuf::from(runtime_dir).join("blue-compositor.sock")
    };

    loop {
        if !socket_path.exists() {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }

        let stream = match UnixStream::connect(&socket_path) {
            Ok(s) => s,
            Err(_) => {
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        let mut reader = BufReader::new(stream);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // disconnected
                Ok(_) => {
                    let t = line.trim();
                    if t.is_empty() { continue; }

                    // Parse the type field to pick the event name
                    let event_name = if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
                        match v.get("type").and_then(|t| t.as_str()) {
                            Some("window_list")         => Some("compositor:window-list"),
                            Some("window_focused")      => Some("compositor:window-focused"),
                            Some("window_opened")       => Some("compositor:window-opened"),
                            Some("window_closed")       => Some("compositor:window-closed"),
                            Some("workspace_switched")  => Some("compositor:workspace-switched"),
                            Some("toggle_start_menu")   => Some("compositor:toggle-start-menu"),
                            Some("idle_changed")        => Some("compositor:idle-changed"),
                            Some("screenshot_ready")    => Some("compositor:screenshot-ready"),
                            Some("screen_locked")       => Some("compositor:screen-locked"),
                            Some("output_changed")      => Some("compositor:output-changed"),
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if let Some(event) = event_name {
                        // Parse payload and emit as Tauri event to all windows
                        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(t) {
                            let _ = app.emit(event, payload);
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // Disconnected — wait before reconnecting
        std::thread::sleep(Duration::from_secs(2));
    }
}

pub fn clipboard_history_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("/tmp"));
    let dir = home.join(".config/Blue-Environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("clipboard_history.json")
}

pub fn notifications_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("/tmp"));
    let dir = home.join(".config/Blue-Environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("notifications.json")
}

// ── Security: pattern lock + fingerprint status (Settings → Security) ──────
// Mirrors BEDM's own pattern hashing exactly (see
// BEDM/bedm-daemon/src/pam_auth.rs::hash_pattern) so a pattern set from
// inside a running desktop session is immediately usable at the next BEDM
// login — same file, same algorithm, no daemon restart needed.

fn pattern_hash_path(home: &str) -> PathBuf {
    PathBuf::from(home).join(".config/Blue-Environment/pattern.hash")
}

fn hash_pattern(username: &str, pattern: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    hasher.update(b":");
    hasher.update(pattern);
    format!("{:x}", hasher.finalize())
}

#[tauri::command]
pub fn save_pattern_lock(username: String, pattern: Vec<u8>) -> Result<(), String> {
    if pattern.len() < 4 {
        return Err("Pattern too short".to_string());
    }
    let home = dirs::home_dir().ok_or("No home directory")?;
    let path = pattern_hash_path(home.to_str().unwrap_or("/tmp"));
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&path, hash_pattern(&username, &pattern)).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_pattern_lock(username: String) -> Result<(), String> {
    let _ = username;
    let home = dirs::home_dir().ok_or("No home directory")?;
    let path = pattern_hash_path(home.to_str().unwrap_or("/tmp"));
    let _ = fs::remove_file(path);
    Ok(())
}

#[tauri::command]
pub fn pattern_is_configured(username: String, home: String) -> bool {
    let _ = username;
    pattern_hash_path(&home).exists()
}

/// Same fprintd check BEDM's greeter uses (see
/// BEDM/bedm-greeter/src/main.rs::has_fingerprint) — duplicated here rather
/// than shared since BEDM and the main desktop session are separate Tauri
/// apps/crates with no code-sharing mechanism between them currently (see
/// STATUS.md "shell" gaps for the follow-up: extract a shared `blue-auth`
/// crate instead of this kind of copy).
#[tauri::command]
pub fn has_fingerprint(username: String) -> bool {
    Command::new("fprintd-list")
        .arg(&username)
        .output()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("no fingers"))
        .unwrap_or(false)
}
