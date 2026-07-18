#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc_client;

use ipc_client::{BedmClient, SessionInfo, UserInfo};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Manager;

type ClientState = Arc<Mutex<Option<BedmClient>>>;

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GreeterConfig {
    pub theme: String,
    pub clock_format: String,
    pub show_user_list: bool,
    pub background: Option<String>,
}

// ── Tauri v2 commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn connect_daemon(state: tauri::State<'_, ClientState>) -> Result<DaemonInfo, String> {
    let socket_path = std::env::var("BEDM_SOCKET")
        .unwrap_or_else(|_| "/run/bedm/bedm.sock".to_string());

    match BedmClient::connect(&socket_path).await {
        Ok((client, info)) => {
            *state.lock().await = Some(client);
            Ok(DaemonInfo {
                version:    info.version,
                hostname:   info.hostname,
                uptime:     info.uptime,
                os_name:    info.os_name,
                os_version: info.os_version,
                connected:  true,
            })
        }
        Err(e) => Err(format!("Cannot connect to BEDM daemon: {}", e)),
    }
}

#[tauri::command]
async fn get_sessions(state: tauri::State<'_, ClientState>) -> Result<Vec<SessionInfo>, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.get_sessions().await
}

#[tauri::command]
async fn get_users(state: tauri::State<'_, ClientState>) -> Result<Vec<UserInfo>, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.get_users().await
}

#[tauri::command]
async fn authenticate(
    state: tauri::State<'_, ClientState>,
    username: String,
    password: String,
) -> Result<AuthResult, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.authenticate(&username, &password).await
}

#[tauri::command]
async fn authenticate_pattern(
    state: tauri::State<'_, ClientState>,
    username: String,
    pattern: Vec<u8>,
) -> Result<AuthResult, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.authenticate_pattern(&username, &pattern).await
}

#[tauri::command]
async fn authenticate_fingerprint(
    state: tauri::State<'_, ClientState>,
    username: String,
) -> Result<AuthResult, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.authenticate_fingerprint(&username).await
}

/// Real hardware check — NOT a placeholder. Queries fprintd (via the
/// `fprintd-list` CLI, which wraps the same D-Bus calls a GUI would use)
/// for whether this specific user has at least one finger enrolled AND a
/// fingerprint reader is present at all. Returns false on any error
/// (missing fprintd package, no reader, no enrollment, permission denied)
/// so the UI simply hides the fingerprint option rather than offering a
/// button that can never succeed.
#[tauri::command]
async fn has_fingerprint(username: String) -> bool {
    tokio::task::spawn_blocking(move || {
        std::process::Command::new("fprintd-list")
            .arg(&username)
            .output()
            .map(|o| {
                o.status.success()
                    && !String::from_utf8_lossy(&o.stdout).contains("no fingers")
            })
            .unwrap_or(false)
    }).await.unwrap_or(false)
}

#[tauri::command]
async fn pattern_is_configured(username: String, home: String) -> bool {
    let _ = username;
    tokio::fs::metadata(format!("{}/.config/Blue-Environment/pattern.hash", home))
        .await.is_ok()
}

#[tauri::command]
async fn start_session(
    state: tauri::State<'_, ClientState>,
    username: String,
    session: String,
) -> Result<String, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.start_session(&username, &session).await
}

#[tauri::command]
async fn power_action(
    state: tauri::State<'_, ClientState>,
    action: String,
) -> Result<bool, String> {
    let mut guard = state.lock().await;
    let client = guard.as_mut().ok_or("Not connected to daemon")?;
    client.power_action(&action).await
}

#[derive(serde::Deserialize, Default)]
struct GreeterRawGeneral {
    background: Option<String>,
    theme: Option<String>,
    clock_format: Option<String>,
    show_user_list: Option<bool>,
}

#[derive(serde::Deserialize, Default)]
struct GreeterRawConfig {
    #[serde(default)]
    general: GreeterRawGeneral,
}

fn read_general_section(config_path: &str) -> GreeterRawGeneral {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|content| toml::from_str::<GreeterRawConfig>(&content).ok())
        .map(|cfg| cfg.general)
        .unwrap_or_default()
}

#[tauri::command]
fn get_wallpaper() -> Option<String> {
    // Read from BEDM config
    let config_path = std::env::var("BEDM_CONFIG")
        .unwrap_or_else(|_| "/etc/bedm/bedm.toml".to_string());

    // Try to read background from the TOML config
    let general = read_general_section(&config_path);
    if let Some(s) = general.background {
        if !s.is_empty() && std::path::Path::new(&s).exists() {
            return Some(format!("file://{}", s));
        }
    }

    // Fallback wallpaper paths
    let paths = [
        "/etc/bedm/wallpaper.png",
        "/etc/bedm/wallpaper.jpg",
        "/usr/share/Blue-Environment/wallpapers/default.png",
        "/usr/share/wallpapers/default.png",
    ];
    paths.iter()
        .find(|p| std::path::Path::new(*p).exists())
        .map(|p| format!("file://{}", p))
}

#[tauri::command]
fn get_greeter_config() -> GreeterConfig {
    let config_path = std::env::var("BEDM_CONFIG")
        .unwrap_or_else(|_| "/etc/bedm/bedm.toml".to_string());

    let general = read_general_section(&config_path);

    GreeterConfig {
        theme: general.theme.unwrap_or_else(|| "blue".to_string()),
        clock_format: general.clock_format.unwrap_or_else(|| "%H:%M".to_string()),
        show_user_list: general.show_user_list.unwrap_or(true),
        background: general.background.filter(|s| !s.is_empty()),
    }
}

#[tauri::command]
fn get_hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "localhost".to_string())
        .trim().to_string()
}

#[tauri::command]
fn get_current_time() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

#[tauri::command]
fn get_current_date() -> String {
    chrono::Local::now().format("%A, %B %-d, %Y").to_string()
}

#[tauri::command]
fn read_user_avatar(path: String) -> Option<String> {
    let data = std::fs::read(&path).ok()?;
    let ext = std::path::Path::new(&path)
        .extension().and_then(|e| e.to_str()).unwrap_or("png");
    let mime = match ext { "jpg"|"jpeg" => "image/jpeg", "svg" => "image/svg+xml", _ => "image/png" };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    Some(format!("data:{};base64,{}", mime, b64))
}

// ── Fix: "BEDM wychodzi poza ekran" (greeter renders off-screen) ───────────
//
// Root cause: tauri.conf.json previously hard-coded a 1920x1080,
// non-resizable, "fullscreen" window. On any monitor that isn't exactly
// 1920x1080 (very common: 1366x768 laptops, ultrawide/4K panels, or a
// greeter compositor that doesn't honor the `fullscreen` hint), the window
// is created at its literal configured size and can be positioned partly or
// fully outside the visible display, with no way to resize it back since
// `resizable` was false.
//
// Fix: don't trust a static size at all. On startup, read the size of the
// monitor the window actually landed on (falling back to the primary
// monitor), resize+reposition the window to exactly match it, THEN show the
// window. This makes the greeter correct on any resolution, on any
// monitor, and even across multi-monitor setups where the greeter may spawn
// on a non-primary display.
fn fit_window_to_monitor(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use tauri::{PhysicalPosition, PhysicalSize};

    let monitor = window
        .current_monitor()?
        .or(window.primary_monitor()?)
        .ok_or_else(|| tauri::Error::WindowNotFound)?;

    let pos = monitor.position();
    let size = monitor.size();

    window.set_position(PhysicalPosition::new(pos.x, pos.y))?;
    window.set_size(PhysicalSize::new(size.width, size.height))?;
    window.set_fullscreen(true)?;
    window.show()?;
    window.set_focus()?;

    Ok(())
}

fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let state: ClientState = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = fit_window_to_monitor(&window) {
                    tracing::warn!("Could not fit greeter window to monitor, falling back to fullscreen(): {e}");
                    let _ = window.set_fullscreen(true);
                    let _ = window.show();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Re-fit on any resolution/monitor change (e.g. hot-plugged
            // display, resolution switch) so the greeter never gets stuck
            // off-screen again after it's already visible.
            if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                if let Some(webview_window) = window.app_handle().get_webview_window(window.label()) {
                    let _ = fit_window_to_monitor(&webview_window);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            connect_daemon,
            get_sessions,
            get_users,
            authenticate,
            authenticate_pattern,
            authenticate_fingerprint,
            has_fingerprint,
            pattern_is_configured,
            start_session,
            power_action,
            get_wallpaper,
            get_greeter_config,
            get_hostname,
            get_current_time,
            get_current_date,
            read_user_avatar,
        ])
        .run(tauri::generate_context!())
        .expect("BEDM greeter error");
}
