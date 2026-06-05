// BEDM Greeter — Tauri backend
// Connects to bedm daemon via Unix socket and exposes commands to React UI

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc_client;

use ipc_client::{BedmClient, SessionInfo, UserInfo};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

type State = Arc<Mutex<Option<BedmClient>>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthResult {
    pub success: bool,
    pub username: Option<String>,
    pub error: Option<String>,
    pub attempts_left: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DaemonInfo {
    pub version: String,
    pub hostname: String,
    pub uptime: u64,
    pub os_name: String,
    pub os_version: String,
    pub connected: bool,
}

// ── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
async fn connect_daemon(state: tauri::State<'_, State>) -> Result<DaemonInfo, String> {
    let socket_path = std::env::var("BEDM_SOCKET")
        .unwrap_or_else(|_| "/run/bedm/bedm.sock".to_string());

    match BedmClient::connect(&socket_path).await {
        Ok((client, info)) => {
            *state.lock().await = Some(client);
            Ok(DaemonInfo {
                version: info.version,
                hostname: info.hostname,
                uptime: info.uptime,
                os_name: info.os_name,
                os_version: info.os_version,
                connected: true,
            })
        }
        Err(e) => Err(format!("Cannot connect to BEDM daemon: {}", e)),
    }
}

#[tauri::command]
async fn get_sessions(state: tauri::State<'_, State>) -> Result<Vec<SessionInfo>, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.get_sessions().await
}

#[tauri::command]
async fn get_users(state: tauri::State<'_, State>) -> Result<Vec<UserInfo>, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.get_users().await
}

#[tauri::command]
async fn authenticate(
    state: tauri::State<'_, State>,
    username: String,
    password: String,
) -> Result<AuthResult, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.authenticate(&username, &password).await
}

#[tauri::command]
async fn start_session(
    state: tauri::State<'_, State>,
    username: String,
    session: String,
) -> Result<String, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.start_session(&username, &session).await
}

#[tauri::command]
async fn power_action(
    state: tauri::State<'_, State>,
    action: String,
) -> Result<bool, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.power_action(&action).await
}

#[tauri::command]
fn get_wallpaper() -> Option<String> {
    // Check config for wallpaper
    let paths = [
        "/etc/bedm/wallpaper.png",
        "/etc/bedm/wallpaper.jpg",
        "/usr/share/Blue-Environment/wallpapers/default.png",
        "/usr/share/wallpapers/default.png",
    ];
    for p in &paths {
        if std::path::Path::new(p).exists() {
            return Some(format!("file://{}", p));
        }
    }
    None
}

#[tauri::command]
fn get_hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "localhost".to_string())
        .trim()
        .to_string()
}

#[tauri::command]
fn get_current_time() -> String {
    let now = chrono::Local::now();
    now.format("%H:%M").to_string()
}

#[tauri::command]
fn get_current_date() -> String {
    let now = chrono::Local::now();
    now.format("%A, %B %-d, %Y").to_string()
}

#[tauri::command]
fn read_user_avatar(path: String) -> Option<String> {
    let data = std::fs::read(&path).ok()?;
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let mime = match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    Some(format!("data:{};base64,{}", mime, b64))
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("BEDM Greeter starting");

    let state: State = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            connect_daemon,
            get_sessions,
            get_users,
            authenticate,
            start_session,
            power_action,
            get_wallpaper,
            get_hostname,
            get_current_time,
            get_current_date,
            read_user_avatar,
        ])
        .run(tauri::generate_context!())
        .expect("BEDM greeter error");
}

// Add chrono to deps
