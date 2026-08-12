use crate::types::*;
use crate::{cache};
use crate::session;
use crate::commands::misc::{clipboard_history_path, notifications_path};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as TokioCommand;

#[tauri::command]
pub fn save_config(config: String) {
    let parsed: cache::UserConfig = serde_json::from_str(&config).unwrap_or_default();
    cache::save_user_config(&parsed);
}

#[tauri::command]
pub fn load_config() -> String {
    cache::load_user_config()
}

#[tauri::command]
pub fn save_window_state(windows: Vec<cache::WindowCache>) {
    cache::save_window_state(&windows);
}

#[tauri::command]
pub fn load_window_state() -> Vec<cache::WindowCache> {
    cache::load_window_state()
}

#[tauri::command]
pub async fn execute_command(command: String) -> Result<CommandOutput, String> {
    let output = TokioCommand::new("sh").arg("-c").arg(&command).output().await.map_err(|e| e.to_string())?;
    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
       stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[tauri::command]
pub fn get_clipboard_history() -> Vec<ClipboardItem> {
    fs::read_to_string(clipboard_history_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn add_to_clipboard_history(content: String) {
    let mut history: Vec<ClipboardItem> = get_clipboard_history();
    history.insert(0, ClipboardItem {
        id: chrono::Utc::now().timestamp_millis().to_string(),
                   content,
                   timestamp: chrono::Utc::now().timestamp_millis() as u64,
    });
    history.truncate(50);
    let _ = fs::write(clipboard_history_path(), serde_json::to_string(&history).unwrap());
}

#[tauri::command]
pub fn clear_clipboard_history() {
    let _ = fs::write(clipboard_history_path(), "[]");
}

#[tauri::command]
pub fn set_night_light_enabled(enabled: bool) -> Result<(), String> {
    match session::detect_session() {
        session::SessionType::WaylandClient => {
            let gamma = if enabled { "1.0:0.8:0.6" } else { "1.0:1.0:1.0" };
            let _ = Command::new("wlr-randr").args(["--output", "eDP-1", "--gamma", gamma]).spawn();
        }
        session::SessionType::X11Client => {
            let temp = if enabled { 4000 } else { 6500 };
            let factor = temp as f32 / 6500.0;
            let _ = Command::new("xrandr").args(["--output", "eDP-1", "--gamma", &format!("{:.2}:{:.2}:{:.2}", 1.0f32, factor * 0.8, factor * 0.6)]).spawn();
        }
        _ => {}
    }
    Ok(())
}

#[tauri::command]
pub fn set_night_light_temperature(temperature: u32) -> Result<(), String> {
    let factor = temperature as f32 / 6500.0;
    let gamma = format!("{:.2}:{:.2}:{:.2}", 1.0f32, factor * 0.8, factor * 0.6);
    match session::detect_session() {
        session::SessionType::WaylandClient => { let _ = Command::new("wlr-randr").args(["--output", "eDP-1", "--gamma", &gamma]).spawn(); }
        session::SessionType::X11Client => { let _ = Command::new("xrandr").args(["--output", "eDP-1", "--gamma", &gamma]).spawn(); }
        _ => {}
    }
    Ok(())
}

#[tauri::command]
pub fn get_notification_history() -> Vec<Notification> {
    fs::read_to_string(notifications_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn save_notification_history(notifications: Vec<Notification>) {
    let _ = fs::write(notifications_path(), serde_json::to_string(&notifications).unwrap());
}

#[tauri::command]
pub fn get_custom_themes() -> Vec<cache::ThemeDefinition> {
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/themes.json");
    fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn save_custom_theme(theme: cache::ThemeDefinition) {
    let mut themes = get_custom_themes();
    themes.retain(|t| t.id != theme.id);
    themes.push(theme);
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/themes.json");
    let _ = fs::write(path, serde_json::to_string_pretty(&themes).unwrap());
}

#[tauri::command]
pub fn delete_custom_theme(theme_id: String) {
    let mut themes = get_custom_themes();
    themes.retain(|t| t.id != theme_id);
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/themes.json");
    let _ = fs::write(path, serde_json::to_string_pretty(&themes).unwrap());
}
