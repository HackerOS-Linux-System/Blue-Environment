mod ai;
mod apps;
mod cache;
mod packages;
mod session;
mod window_tracker;
mod icon_resolver;
#[path = "MailApp/mod.rs"]
mod mail_app;
#[path = "BlueArchiveApp/mod.rs"]
mod blue_archive_app;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemStats {
    pub cpu: f32,
    pub ram: f32,
    pub battery: Option<f32>,
    pub is_charging: bool,
    pub volume: f32,
    pub brightness: f32,
    pub wifi_ssid: String,
    pub kernel: String,
    pub session_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessInfo {
    pub pid: String,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioSink {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub volume: f32,
    pub muted: bool,
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: i32,
    pub secure: bool,
    pub in_use: bool,
    pub bssid: String,
    pub frequency: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BluetoothDevice {
    pub name: String,
    pub mac: String,
    pub device_type: String,
    pub connected: bool,
    pub paired: bool,
    pub trusted: bool,
    pub battery: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalWindow {
    pub id: String,
    pub pid: u32,
    pub title: String,
    pub class: String,
    pub icon_path: String,
    pub is_minimized: bool,
    pub desktop: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub mime_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PowerProfile {
    pub name: String,
    pub active: bool,
    pub icon: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    pub message: Option<String>,
    pub timestamp: u64,
    pub read: bool,
    pub app: Option<String>,
    pub app_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeDefinition {
    pub id: String,
    pub name: String,
    pub colors: HashMap<String, String>,
    pub r#type: Option<String>,
    pub css: Option<String>,
}

// ── Package manager commands ──────────────────────────────────────────────────

#[tauri::command]
async fn get_dnf_packages() -> Vec<ai::PackageInfo> {
    tokio::task::spawn_blocking(packages::get_dnf_packages).await.unwrap_or_default()
}
#[tauri::command]
async fn get_flatpak_packages() -> Vec<ai::PackageInfo> {
    tokio::task::spawn_blocking(packages::get_flatpak_packages).await.unwrap_or_default()
}
#[tauri::command]
async fn get_appimage_packages() -> Vec<ai::PackageInfo> {
    tokio::task::spawn_blocking(packages::get_appimage_packages).await.unwrap_or_default()
}
#[tauri::command]
async fn install_dnf_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::install_dnf(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn remove_dnf_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::remove_dnf(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn update_dnf_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::update_dnf(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn install_flatpak_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::install_flatpak(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn remove_flatpak_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::remove_flatpak(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn update_flatpak_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::update_flatpak(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn install_appimage(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::install_appimage(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn remove_appimage(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::remove_appimage(&pkg_id)).await.unwrap_or(false))
}
#[tauri::command]
async fn update_appimage(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::update_appimage(&pkg_id)).await.unwrap_or(false))
}

// ── App commands ──────────────────────────────────────────────────────────────
// Frontend calls: get_system_apps, launch_process, record_app_launch,
//                 get_recent_apps, invalidate_app_cache

/// Alias for get_installed_apps — frontend calls this as "get_system_apps"
#[tauri::command]
async fn get_system_apps(force_refresh: Option<bool>) -> Result<Vec<apps::AppInfo>, String> {
    let fr = force_refresh.unwrap_or(false);
    if fr {
        tokio::task::spawn_blocking(|| apps::scan_desktop_apps(true))
        .await
        .map(|v| v.into_iter().map(apps::AppInfo::from).collect())
        .map_err(|e| e.to_string())
    } else {
        apps::get_installed_apps().await
    }
}

/// Frontend calls launch_process({ command, appId }) — maps to launching by exec string
#[tauri::command]
async fn launch_process(command: String, app_id: Option<String>) -> Result<bool, String> {
    let _ = app_id; // optionally record; ignored for now
    tokio::task::spawn_blocking(move || {
        std::process::Command::new("sh")
        .arg("-c")
        .arg(&command)
        .spawn()
        .map(|_| true)
        .unwrap_or(false)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Track recently launched apps (stored in a simple JSON file)
#[tauri::command]
async fn record_app_launch(app_id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let path = recent_apps_path();
        let mut ids = load_recent_ids(&path);
        ids.retain(|id| id != &app_id);
        ids.insert(0, app_id);
        ids.truncate(20);
        let _ = std::fs::write(&path, serde_json::to_string(&ids).unwrap_or_default());
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recent_apps() -> Vec<String> {
    tokio::task::spawn_blocking(|| load_recent_ids(&recent_apps_path()))
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn invalidate_app_cache() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        cache::invalidate_app_cache();
    })
    .await
    .map_err(|e| e.to_string())
}

fn recent_apps_path() -> std::path::PathBuf {
    dirs::data_local_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    .join("blue-environment/recent_apps.json")
}

fn load_recent_ids(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default()
}

// ── Session / VT ──────────────────────────────────────────────────────────────

#[tauri::command]
async fn lock_session() -> Result<(), String> {
    session::lock().await
}
#[tauri::command]
fn get_session_type() -> String {
    std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "x11".to_string())
}

// ── System stats ─────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_system_stats() -> Result<SystemStats, String> {
    tokio::task::spawn_blocking(|| {
        let cpu = read_cpu_usage();
        let ram = read_ram_usage();
        let (battery, is_charging) = read_battery();
        let volume = read_pulse_volume();
        let brightness = read_brightness();
        let wifi_ssid = read_wifi_ssid();
        let kernel = read_kernel();
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "x11".to_string());
        SystemStats { cpu, ram, battery, is_charging, volume, brightness, wifi_ssid, kernel, session_type }
    })
    .await
    .map_err(|e| e.to_string())
}

fn read_cpu_usage() -> f32 {
    // Read /proc/stat twice with a short sleep for delta
    fn read_stat() -> Option<(u64, u64)> {
        let s = std::fs::read_to_string("/proc/stat").ok()?;
        let line = s.lines().next()?;
        let nums: Vec<u64> = line.split_whitespace().skip(1)
        .filter_map(|v| v.parse().ok()).collect();
        let idle = nums.get(3).copied().unwrap_or(0);
        let total: u64 = nums.iter().sum();
        Some((idle, total))
    }
    let a = read_stat().unwrap_or((0, 1));
    std::thread::sleep(std::time::Duration::from_millis(100));
    let b = read_stat().unwrap_or((0, 1));
    let total_delta = b.1.saturating_sub(a.1);
    let idle_delta  = b.0.saturating_sub(a.0);
    if total_delta == 0 { return 0.0; }
    ((total_delta - idle_delta) as f32 / total_delta as f32) * 100.0
}

fn read_ram_usage() -> f32 {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let get = |key: &str| -> u64 {
        s.lines().find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
    };
    let total = get("MemTotal:");
    let available = get("MemAvailable:");
    if total == 0 { return 0.0; }
    ((total - available) as f32 / total as f32) * 100.0
}

fn read_battery() -> (Option<f32>, bool) {
    let base = std::path::Path::new("/sys/class/power_supply");
    if !base.exists() { return (None, false); }
    let dirs = std::fs::read_dir(base).ok();
    for entry in dirs.into_iter().flatten().filter_map(|e| e.ok()) {
        let p = entry.path();
        let r#type = std::fs::read_to_string(p.join("type")).unwrap_or_default();
        if r#type.trim() != "Battery" { continue; }
        let cap: f32 = std::fs::read_to_string(p.join("capacity"))
        .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
        let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
        let charging = status.trim() == "Charging" || status.trim() == "Full";
        return (Some(cap), charging);
    }
    (None, false)
}

fn read_pulse_volume() -> f32 {
    let out = std::process::Command::new("sh")
    .arg("-c")
    .arg("pactl get-sink-volume @DEFAULT_SINK@ 2>/dev/null | grep -oP '\\d+(?=%)' | head -1")
    .output()
    .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(0.0)
}

fn read_brightness() -> f32 {
    let base = std::path::Path::new("/sys/class/backlight");
    if let Ok(mut entries) = std::fs::read_dir(base) {
        if let Some(Ok(entry)) = entries.next() {
            let p = entry.path();
            let cur: f32 = std::fs::read_to_string(p.join("brightness"))
            .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
            let max: f32 = std::fs::read_to_string(p.join("max_brightness"))
            .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
            return if max > 0.0 { (cur / max) * 100.0 } else { 100.0 };
        }
    }
    100.0
}

fn read_wifi_ssid() -> String {
    std::process::Command::new("sh")
    .arg("-c")
    .arg("iwgetid -r 2>/dev/null || nmcli -t -f active,ssid dev wifi 2>/dev/null | grep '^yes' | cut -d: -f2")
    .output()
    .ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim().to_string())
    .unwrap_or_default()
}

fn read_kernel() -> String {
    std::process::Command::new("uname")
    .arg("-r")
    .output()
    .ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|| "unknown".to_string())
}

// ── Processes ────────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_processes() -> Vec<ProcessInfo> {
    tokio::task::spawn_blocking(|| {
        let stdout = std::process::Command::new("sh")
        .arg("-c")
        .arg("ps axo pid=,comm=,pcpu=,rss= --no-headers 2>/dev/null")
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
        String::from_utf8_lossy(&stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 { return None; }
            Some(ProcessInfo {
                pid: parts[0].to_string(),
                 name: parts[1].to_string(),
                 cpu: parts[2].parse().unwrap_or(0.0),
                 memory: parts[3].parse::<u64>().unwrap_or(0) * 1024,
            })
        })
        .collect()
    })
    .await
    .unwrap_or_default()
}

// ── Audio ────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_audio_sinks() -> Vec<AudioSink> {
    tokio::task::spawn_blocking(|| {
        let stdout = std::process::Command::new("pactl")
        .args(["list", "sinks"])
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
        let text = String::from_utf8_lossy(&stdout);
        parse_pactl_sinks(&text)
    })
    .await
    .unwrap_or_default()
}

fn parse_pactl_sinks(text: &str) -> Vec<AudioSink> {
    let mut sinks = Vec::new();
    let mut id = 0u32;
    let mut name = String::new();
    let mut description = String::new();
    let mut volume = 0f32;
    let mut muted = false;
    let mut is_default = false;
    let mut in_sink = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Sink #") {
            if in_sink && !name.is_empty() {
                sinks.push(AudioSink { id, name: name.clone(), description: description.clone(), volume, muted, is_default });
            }
            id = trimmed.trim_start_matches("Sink #").parse().unwrap_or(0);
            name.clear(); description.clear(); volume = 0.0; muted = false; is_default = false;
            in_sink = true;
        } else if in_sink {
            if trimmed.starts_with("Name:") {
                name = trimmed.trim_start_matches("Name:").trim().to_string();
            } else if trimmed.starts_with("Description:") {
                description = trimmed.trim_start_matches("Description:").trim().to_string();
            } else if trimmed.starts_with("Mute:") {
                muted = trimmed.contains("yes");
            } else if trimmed.starts_with("Volume:") {
                volume = trimmed.split('%').next()
                .and_then(|s| s.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<String>().chars().rev().collect::<String>().parse().ok())
                .unwrap_or(0.0);
            }
        }
    }
    if in_sink && !name.is_empty() {
        sinks.push(AudioSink { id, name, description, volume, muted, is_default });
    }
    sinks
}

#[tauri::command]
async fn set_volume(level: f32) -> Result<(), String> {
    let pct = format!("{}%", level as u32);
    tokio::process::Command::new("pactl")
    .args(["set-sink-volume", "@DEFAULT_SINK@", &pct])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_sink_volume(sink_name: String, volume: f32) -> Result<(), String> {
    let pct = format!("{}%", volume as u32);
    tokio::process::Command::new("pactl")
    .args(["set-sink-volume", &sink_name, &pct])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_default_sink(sink_name: String) -> Result<(), String> {
    tokio::process::Command::new("pactl")
    .args(["set-default-sink", &sink_name])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_sink_mute(sink_name: String) -> Result<(), String> {
    tokio::process::Command::new("pactl")
    .args(["set-sink-mute", &sink_name, "toggle"])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

// ── Brightness ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn set_brightness(level: f32) -> Result<(), String> {
    let level_str = format!("{}", (level as u32).clamp(0, 100));
    // Try brightnessctl first, fall back to xrandr
    let res = tokio::process::Command::new("brightnessctl")
    .args(["set", &format!("{}%", level_str)])
    .status().await;
    if res.map(|s| s.success()).unwrap_or(false) { return Ok(()); }
    tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("xrandr --output $(xrandr | grep ' connected' | head -1 | awk '{{print $1}}') --brightness {}", level as f32 / 100.0))
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

// ── Wi-Fi ─────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_wifi_networks_real() -> Result<Vec<WifiNetwork>, String> {
    let out = tokio::process::Command::new("nmcli")
    .args(["-t", "-f", "SSID,SIGNAL,SECURITY,IN-USE,BSSID,FREQ", "dev", "wifi"])
    .output().await.map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut nets: Vec<WifiNetwork> = text.lines().filter_map(|line| {
        let parts: Vec<&str> = line.splitn(6, ':').collect();
        if parts.len() < 5 { return None; }
        let ssid = parts[0].to_string();
        if ssid.is_empty() { return None; }
        Some(WifiNetwork {
            ssid,
            signal: parts[1].parse().unwrap_or(0),
             secure: !parts[2].is_empty() && parts[2] != "--",
             in_use: parts[3] == "*",
             bssid: parts[4].to_string(),
             frequency: parts.get(5).unwrap_or(&"").to_string(),
        })
    }).collect();
    nets.sort_by(|a, b| b.signal.cmp(&a.signal));
    Ok(nets)
}

#[tauri::command]
async fn connect_wifi_real(ssid: String, password: String) -> Result<(), String> {
    tokio::process::Command::new("nmcli")
    .args(["dev", "wifi", "connect", &ssid, "password", &password])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn disconnect_wifi() -> Result<(), String> {
    tokio::process::Command::new("sh")
    .arg("-c")
    .arg("nmcli dev disconnect $(nmcli -t -f DEVICE,TYPE dev | grep wifi | cut -d: -f1 | head -1)")
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_wifi(enabled: bool) -> Result<(), String> {
    let state = if enabled { "on" } else { "off" };
    tokio::process::Command::new("nmcli")
    .args(["radio", "wifi", state])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

// ── Bluetooth ─────────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_bluetooth_devices_real() -> Result<Vec<BluetoothDevice>, String> {
    let out = tokio::process::Command::new("bluetoothctl")
    .arg("devices")
    .output().await.map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut devices = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 3 { continue; }
        let mac = parts[1].to_string();
        let name = parts[2].to_string();
        let info_stdout = tokio::process::Command::new("bluetoothctl")
        .args(["info", &mac])
        .output().await
        .map(|o| o.stdout)
        .unwrap_or_default();
        let info_text = String::from_utf8_lossy(&info_stdout);
        let connected = info_text.lines().any(|l| l.trim().starts_with("Connected: yes"));
        let paired    = info_text.lines().any(|l| l.trim().starts_with("Paired: yes"));
        let trusted   = info_text.lines().any(|l| l.trim().starts_with("Trusted: yes"));
        devices.push(BluetoothDevice {
            name, mac,
            device_type: "unknown".to_string(),
                     connected, paired, trusted,
                     battery: None,
        });
    }
    Ok(devices)
}

#[tauri::command]
async fn bluetooth_connect(mac: String) -> Result<(), String> {
    tokio::process::Command::new("bluetoothctl")
    .args(["connect", &mac])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn bluetooth_disconnect(mac: String) -> Result<(), String> {
    tokio::process::Command::new("bluetoothctl")
    .args(["disconnect", &mac])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn bluetooth_pair(mac: String) -> Result<(), String> {
    tokio::process::Command::new("bluetoothctl")
    .args(["pair", &mac])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

// ── Screenshot ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn take_screenshot() -> Result<(), String> {
    let ts = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
    let path = format!("{}/screenshot_{}.png",
                       dirs::picture_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp")).display(), ts);
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let cmd = if session.contains("wayland") {
        format!("grim {}", path)
    } else {
        format!("scrot {}", path)
    };
    tokio::process::Command::new("sh")
    .arg("-c").arg(&cmd)
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

// ── Wallpapers ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_wallpapers() -> Vec<String> {
    tokio::task::spawn_blocking(|| {
        let dirs = [
            "/usr/share/Blue-Environment/wallpapers",
            "/usr/share/wallpapers",
            "/usr/share/backgrounds",
        ];
        let exts = ["jpg", "jpeg", "png", "webp"];
        let mut wps = Vec::new();
        for dir in &dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.extension().map_or(false, |e| exts.contains(&e.to_str().unwrap_or(""))) {
                        wps.push(format!("file://{}", p.display()));
                    }
                }
            }
        }
        wps
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn get_wallpaper_preview(path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let b64 = base64_encode(&bytes);
        let mime = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
        Ok(format!("data:{};base64,{}", mime, b64))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(CHARS[b0 >> 2] as char);
        out.push(CHARS[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(if chunk.len() > 1 { CHARS[((b1 & 0xf) << 2) | (b2 >> 6)] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[b2 & 0x3f] as char } else { '=' });
    }
    out
}

// ── Config ───────────────────────────────────────────────────────────────────

fn config_path() -> std::path::PathBuf {
    dirs::config_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    .join("blue-environment/config.json")
}

#[tauri::command]
async fn save_config(config: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let p = config_path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, &config).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn load_config() -> String {
    tokio::task::spawn_blocking(|| std::fs::read_to_string(config_path()).unwrap_or_else(|_| "{}".to_string()))
    .await
    .unwrap_or_else(|_| "{}".to_string())
}

// ── Window state ─────────────────────────────────────────────────────────────

fn window_state_path() -> std::path::PathBuf {
    dirs::data_local_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    .join("blue-environment/window_state.json")
}

#[tauri::command]
async fn save_window_state(windows: serde_json::Value) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let p = window_state_path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, serde_json::to_string(&windows).unwrap_or_default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn load_window_state() -> Vec<serde_json::Value> {
    tokio::task::spawn_blocking(|| {
        std::fs::read_to_string(window_state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

// ── Files ─────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn list_files(path: String) -> Result<Vec<FileEntry>, String> {
    tokio::task::spawn_blocking(move || {
        let entries = std::fs::read_dir(&path).map_err(|e| e.to_string())?;
        let mut files = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            let modified = meta.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
            let name = entry.file_name().to_string_lossy().to_string();
            let full_path = entry.path().to_string_lossy().to_string();
            let is_dir = meta.is_dir();
            let mime_type = if is_dir { "inode/directory".to_string() }
            else { mime_guess(&name) };
            files.push(FileEntry {
                name, path: full_path, is_dir,
                size: if is_dir { 0 } else { meta.len() },
                       modified, mime_type,
            });
        }
        files.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(files)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn mime_guess(name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png"  => "image/png", "jpg"|"jpeg" => "image/jpeg", "gif" => "image/gif",
        "svg"  => "image/svg+xml", "webp" => "image/webp",
        "mp4"  => "video/mp4", "mkv" => "video/x-matroska", "avi" => "video/x-msvideo",
        "mp3"  => "audio/mpeg", "ogg" => "audio/ogg", "flac" => "audio/flac",
        "pdf"  => "application/pdf", "zip" => "application/zip",
        "txt"  => "text/plain", "md" => "text/markdown",
        "html" => "text/html", "css" => "text/css", "js" => "text/javascript",
        "rs"   => "text/x-rust", "ts" => "text/typescript", "tsx" => "text/typescript",
        _      => "application/octet-stream",
    }.to_string()
}

#[tauri::command]
async fn read_text_file(path: String) -> Result<String, String> {
    tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn write_text_file(path: String, content: String) -> Result<(), String> {
    tokio::fs::write(&path, content).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_folder(path: String, name: String) -> Result<(), String> {
    let full = std::path::Path::new(&path).join(&name);
    tokio::fs::create_dir_all(&full).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_file(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if p.is_dir() {
        tokio::fs::remove_dir_all(&path).await.map_err(|e| e.to_string())
    } else {
        tokio::fs::remove_file(&path).await.map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn copy_file(src: String, dest: String) -> Result<(), String> {
    tokio::fs::copy(&src, &dest).await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn move_file(src: String, dest: String) -> Result<(), String> {
    tokio::fs::rename(&src, &dest).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_text_file(path: String, name: String, content: Option<String>) -> Result<(), String> {
    let full = std::path::Path::new(&path).join(&name);
    tokio::fs::write(&full, content.unwrap_or_default()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn read_file_as_data_url(path: String) -> Result<String, String> {
    let bytes = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
    let b64 = base64_encode(&bytes);
    let mime = mime_guess(&path);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
async fn save_file_from_data_url(path: String, data_url: String) -> Result<(), String> {
    let base64_part = data_url.splitn(2, ',').nth(1).unwrap_or("");
    let bytes = base64_decode(base64_part).map_err(|e| e.to_string())?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())
}

fn base64_decode(input: &str) -> Result<Vec<u8>, &'static str> {
    let input: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = Vec::new();
    let decode_char = |c: char| -> Result<u8, &'static str> {
        match c {
            'A'..='Z' => Ok(c as u8 - b'A'),
            'a'..='z' => Ok(c as u8 - b'a' + 26),
            '0'..='9' => Ok(c as u8 - b'0' + 52),
            '+' => Ok(62), '/' => Ok(63), '=' => Ok(0),
            _ => Err("Invalid base64 char"),
        }
    };
    for chunk in input.as_bytes().chunks(4) {
        let s: String = chunk.iter().map(|&b| b as char).collect();
        let chars: Vec<char> = s.chars().collect();
        if chars.len() < 2 { break; }
        let b0 = decode_char(chars[0])?;
        let b1 = decode_char(chars[1])?;
        out.push((b0 << 2) | (b1 >> 4));
        if chars.get(2).copied() != Some('=') {
            let b2 = decode_char(chars.get(2).copied().unwrap_or('='))?;
            out.push((b1 << 4) | (b2 >> 2));
            if chars.get(3).copied() != Some('=') {
                let b3 = decode_char(chars.get(3).copied().unwrap_or('='))?;
                out.push((b2 << 6) | b3);
            }
        }
    }
    Ok(out)
}

#[tauri::command]
async fn get_home_path() -> String {
    dirs::home_dir()
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_else(|| "/home/user".to_string())
}

#[tauri::command]
async fn get_default_desktop_path() -> String {
    dirs::home_dir()
    .map(|p| p.join("Desktop").to_string_lossy().to_string())
    .unwrap_or_else(|| "/home/user/Desktop".to_string())
}

// ── Terminal / commands ───────────────────────────────────────────────────────

#[tauri::command]
async fn execute_command(command: String) -> Result<serde_json::Value, String> {
    let out = tokio::process::Command::new("sh")
    .arg("-c")
    .arg(&command)
    .output()
    .await
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "stdout": String::from_utf8_lossy(&out.stdout),
                         "stderr": String::from_utf8_lossy(&out.stderr),
                         "code": out.status.code().unwrap_or(-1),
    }))
}

#[tauri::command]
async fn spawn_terminal(window_id: String) -> Result<serde_json::Value, String> {
    let _ = window_id;
    // Try common terminal emulators
    let terms = ["foot", "alacritty", "kitty", "xterm", "gnome-terminal"];
    for term in &terms {
        if tokio::process::Command::new(term).spawn().is_ok() {
            return Ok(serde_json::json!({ "success": true }));
        }
    }
    Ok(serde_json::json!({ "success": false, "error": "No terminal emulator found" }))
}

#[tauri::command]
async fn write_to_terminal(command: String) -> Result<serde_json::Value, String> {
    // PTY management would require a more complex implementation.
    // This stub returns an error so the frontend knows it's not fully implemented.
    let _ = command;
    Ok(serde_json::json!({ "error": "PTY write requires terminal spawned by spawn_terminal" }))
}

// ── Git ───────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn git_status(path: String) -> Result<Vec<String>, String> {
    let out = tokio::process::Command::new("git")
    .args(["-C", &path, "status", "--short"])
    .output().await.map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.lines().map(|l| l.to_string()).collect())
}

// ── Power ─────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn system_power(action: String) -> Result<(), String> {
    let cmd = match action.as_str() {
        "shutdown"  => "systemctl poweroff",
        "reboot"    => "systemctl reboot",
        "suspend"   => "systemctl suspend",
        "hibernate" => "systemctl hibernate",
        "logout"    => "loginctl terminate-session $XDG_SESSION_ID",
        _ => return Err(format!("Unknown power action: {}", action)),
    };
    tokio::process::Command::new("sh")
    .arg("-c").arg(cmd)
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_power_profiles() -> Vec<PowerProfile> {
    let out = std::process::Command::new("powerprofilesctl")
    .arg("list")
    .output()
    .map(|o| o.stdout)
    .unwrap_or_default();
    let text = String::from_utf8_lossy(&out);
    let active = text.lines()
    .find(|l| l.contains("*"))
    .and_then(|l| l.split_whitespace().next())
    .unwrap_or("balanced")
    .trim_start_matches('*')
    .to_string();
    vec![
        PowerProfile { name: "power-saver".to_string(),  active: active == "power-saver",  icon: "Battery".to_string(), description: "Oszczędzanie energii".to_string() },
        PowerProfile { name: "balanced".to_string(),     active: active == "balanced",      icon: "Wind".to_string(),    description: "Zrównoważony".to_string() },
        PowerProfile { name: "performance".to_string(),  active: active == "performance",   icon: "Zap".to_string(),     description: "Wydajność".to_string() },
    ]
}

#[tauri::command]
async fn set_power_profile(profile: String) -> Result<(), String> {
    tokio::process::Command::new("powerprofilesctl")
    .args(["set", &profile])
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

// ── Distro info ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn load_distro_info() -> serde_json::Value {
    tokio::task::spawn_blocking(|| {
        let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let get = |key: &str| -> String {
            os_release.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.splitn(2, '=').nth(1))
            .map(|v| v.trim_matches('"').to_string())
            .unwrap_or_default()
        };
        serde_json::json!({
            "Name": get("NAME"),
                          "Version": get("VERSION"),
                          "Copyright": format!("© 2026 {} Team", get("NAME")),
        })
    })
    .await
    .unwrap_or_else(|_| serde_json::json!({}))
}

// ── AI ────────────────────────────────────────────────────────────────────────

fn ai_config_path() -> std::path::PathBuf {
    dirs::config_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    .join("blue-environment/ai_config.json")
}

#[tauri::command]
async fn get_ai_config() -> Option<serde_json::Value> {
    tokio::task::spawn_blocking(|| {
        std::fs::read_to_string(ai_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    })
    .await
    .unwrap_or(None)
}

#[tauri::command]
async fn save_ai_config(config: serde_json::Value) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let p = ai_config_path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, serde_json::to_string(&config).unwrap_or_default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn ai_call(request: ai::AICallRequest) -> Result<String, String> {
    ai::ai_call(request).await
}

// ── Clipboard ─────────────────────────────────────────────────────────────────

fn clipboard_history_path() -> std::path::PathBuf {
    dirs::data_local_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    .join("blue-environment/clipboard_history.json")
}

#[tauri::command]
async fn get_clipboard_history() -> Vec<ClipboardEntry> {
    tokio::task::spawn_blocking(|| {
        std::fs::read_to_string(clipboard_history_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn add_to_clipboard_history(content: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut hist: Vec<ClipboardEntry> = std::fs::read_to_string(clipboard_history_path())
        .ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        hist.retain(|e| e.content != content);
        hist.insert(0, ClipboardEntry {
            id: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string(),
                    content,
                    timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
        });
        hist.truncate(50);
        let p = clipboard_history_path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, serde_json::to_string(&hist).unwrap_or_default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn clear_clipboard_history() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        let p = clipboard_history_path();
        if p.exists() { std::fs::write(&p, "[]").map_err(|e| e.to_string()) } else { Ok(()) }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn clipboard_copy(text: String) -> Result<(), String> {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let cmd = if session.contains("wayland") {
        format!("echo -n '{}' | wl-copy", text.replace('\'', "'\\''"))
    } else {
        format!("echo -n '{}' | xclip -selection clipboard", text.replace('\'', "'\\''"))
    };
    tokio::process::Command::new("sh")
    .arg("-c").arg(&cmd)
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clipboard_paste() -> Result<String, String> {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let cmd = if session.contains("wayland") { "wl-paste" } else { "xclip -o -selection clipboard" };
    let out = tokio::process::Command::new("sh")
    .arg("-c").arg(cmd)
    .output().await.map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[tauri::command]
async fn write_clipboard_image(data_url: String) -> Result<(), String> {
    let _ = data_url;
    // Requires wl-copy --type image/png or xclip with image support
    Ok(())
}

// ── Notifications ─────────────────────────────────────────────────────────────

fn notification_history_path() -> std::path::PathBuf {
    dirs::data_local_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    .join("blue-environment/notification_history.json")
}

#[tauri::command]
async fn get_notification_history() -> Vec<Notification> {
    tokio::task::spawn_blocking(|| {
        std::fs::read_to_string(notification_history_path())
        .ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn save_notification_history(notifications: Vec<Notification>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let p = notification_history_path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, serde_json::to_string(&notifications).unwrap_or_default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Custom themes ─────────────────────────────────────────────────────────────

fn themes_path() -> std::path::PathBuf {
    dirs::config_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
    .join("blue-environment/custom_themes.json")
}

#[tauri::command]
async fn get_custom_themes() -> Vec<ThemeDefinition> {
    tokio::task::spawn_blocking(|| {
        std::fs::read_to_string(themes_path())
        .ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn save_custom_theme(theme: ThemeDefinition) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut themes: Vec<ThemeDefinition> = std::fs::read_to_string(themes_path())
        .ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        themes.retain(|t| t.id != theme.id);
        themes.push(theme);
        let p = themes_path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, serde_json::to_string(&themes).unwrap_or_default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn delete_custom_theme(theme_id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut themes: Vec<ThemeDefinition> = std::fs::read_to_string(themes_path())
        .ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        themes.retain(|t| t.id != theme_id);
        let p = themes_path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, serde_json::to_string(&themes).unwrap_or_default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Night light ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn set_night_light_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        tokio::process::Command::new("sh")
        .arg("-c").arg("gammastep -O 4000 &")
        .spawn().map(|_| ()).map_err(|e| e.to_string())
    } else {
        tokio::process::Command::new("sh")
        .arg("-c").arg("pkill gammastep; xrandr --gamma 1:1:1")
        .status().await.map(|_| ()).map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn set_night_light_temperature(temperature: u32) -> Result<(), String> {
    tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("pkill gammastep; gammastep -O {} &", temperature))
    .spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ── External windows (stub — requires compositor integration) ─────────────────

#[tauri::command]
async fn get_external_windows() -> Vec<ExternalWindow> {
    // Full implementation requires X11/Wayland compositor bindings.
    // window_tracker module can be extended here.
    vec![]
}

#[tauri::command]
async fn focus_external_window(win_id: String) -> Result<(), String> {
    tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("wmctrl -ia {}", win_id))
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn minimize_external_window(win_id: String) -> Result<(), String> {
    tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("xdotool windowminimize {}", win_id))
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn close_external_window(win_id: String) -> Result<(), String> {
    tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("wmctrl -ic {}", win_id))
    .status().await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn embed_external_window(_win_id: String, _parent_id: String) -> bool {
    false // Embedding requires XEmbed protocol support
}

// ── Panel ─────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn set_panel_enabled(_enabled: bool) -> Result<(), String> {
    // Panel visibility is controlled by the frontend; no backend action needed
    Ok(())
}

// ── LSP ───────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn start_language_server(language: String, root_path: String) -> Result<serde_json::Value, String> {
    let bin = match language.as_str() {
        "rust"       => "rust-analyzer",
        "typescript" | "javascript" => "typescript-language-server",
        "python"     => "pylsp",
        "c" | "cpp"  => "clangd",
        "go"         => "gopls",
        _ => return Ok(serde_json::json!({ "success": false, "error": format!("Unknown language: {}", language) })),
    };
    let _ = root_path;
    match tokio::process::Command::new(bin).spawn() {
        Ok(_) => Ok(serde_json::json!({ "success": true })),
        Err(e) => Ok(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

// ── Tauri v2 builder ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
    .plugin(tauri_plugin_fs::init())
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_http::init())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_process::init())
    .invoke_handler(tauri::generate_handler![
        mail_app::mail_get_accounts, mail_app::mail_save_account, mail_app::mail_delete_account,
        mail_app::mail_fetch_inbox, mail_app::mail_send, mail_app::mail_mark_read, mail_app::mail_move_message,
        blue_archive_app::archive_list, blue_archive_app::archive_extract, blue_archive_app::archive_create,
        // Packages
        get_dnf_packages, get_flatpak_packages, get_appimage_packages,
        install_dnf_package, remove_dnf_package, update_dnf_package,
        install_flatpak_package, remove_flatpak_package, update_flatpak_package,
        install_appimage, remove_appimage, update_appimage,
        // Apps — named to match systemBridge.ts exactly
        get_system_apps, launch_process, record_app_launch, get_recent_apps, invalidate_app_cache,
        // Session
        lock_session, get_session_type,
        // System
        get_system_stats, get_processes,
        // Audio
        get_audio_sinks, set_volume, set_sink_volume, set_default_sink, toggle_sink_mute,
        // Wi-Fi
        get_wifi_networks_real, connect_wifi_real, disconnect_wifi, toggle_wifi,
        // Bluetooth
        get_bluetooth_devices_real, bluetooth_connect, bluetooth_disconnect, bluetooth_pair,
        // Brightness & screenshot
        set_brightness, take_screenshot,
        // Wallpapers
        get_wallpapers, get_wallpaper_preview,
        // Config
        save_config, load_config,
        // Window state
        save_window_state, load_window_state,
        // Files
        list_files, read_text_file, write_text_file,
        create_folder, delete_file, copy_file, move_file, create_text_file,
        read_file_as_data_url, save_file_from_data_url,
        get_home_path, get_default_desktop_path,
        // Terminal
        execute_command, spawn_terminal, write_to_terminal,
        // Git
        git_status,
        // Power
        system_power, get_power_profiles, set_power_profile,
        // Distro info
        load_distro_info,
        // AI
        get_ai_config, save_ai_config, ai_call,
        // Clipboard
        get_clipboard_history, add_to_clipboard_history, clear_clipboard_history,
        clipboard_copy, clipboard_paste, write_clipboard_image,
        // Notifications
        get_notification_history, save_notification_history,
        // Themes
        get_custom_themes, save_custom_theme, delete_custom_theme,
        // Night light
        set_night_light_enabled, set_night_light_temperature,
        // External windows
        get_external_windows, focus_external_window, minimize_external_window,
        close_external_window, embed_external_window,
        // Panel & misc
        set_panel_enabled, start_language_server,
    ])
    .run(tauri::generate_context!())
    .expect("Blue Environment error")
}
