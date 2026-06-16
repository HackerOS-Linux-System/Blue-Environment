#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod session;
mod cache;
mod apps;
mod window_tracker;
mod ai;
mod packages;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex as StdMutex;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use glob::glob;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cache::CachedApp;
use tokio::process::{Command as TokioCommand};
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use tokio::sync::Mutex;
use tauri::{Window, Emitter};
use lazy_static::lazy_static;
use serde::{Serialize, Deserialize};

// ── Structs ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: String,
    mime_type: String,
    modified: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SystemStats {
    cpu: f32,
    ram: f32,
    battery: f32,
    is_charging: bool,
    volume: i32,
    brightness: i32,
    wifi_ssid: String,
    kernel: String,
    session_type: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProcessEntry {
    pid: String,
    name: String,
    cpu: f32,
    memory: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WifiNetwork {
    ssid: String,
    signal: u8,
    secure: bool,
    in_use: bool,
    bssid: String,
    frequency: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BluetoothDevice {
    name: String,
    mac: String,
    device_type: String,
    connected: bool,
    paired: bool,
    trusted: bool,
    battery: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioSink {
    id: u32,
    name: String,
    description: String,
    volume: f32,
    muted: bool,
    is_default: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PowerProfile {
    name: String,
    active: bool,
    icon: Option<String>,
    description: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommandOutput {
    stdout: String,
    stderr: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ClipboardItem {
    id: String,
    content: String,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Notification {
    id: String,
    title: String,
    message: String,
    app_id: Option<String>,
    timestamp: u64,
    read: bool,
    icon: Option<String>,
    actions: Option<Vec<Action>>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Action {
    label: String,
    action: String,
}

struct TerminalProcess {
    stdin: tokio::process::ChildStdin,
}

lazy_static! {
    static ref TERMINALS: std::sync::Arc<Mutex<HashMap<String, TerminalProcess>>> =
    std::sync::Arc::new(Mutex::new(HashMap::new()));
}

fn clipboard_history_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("/tmp"));
    let dir = home.join(".config/Blue-Environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("clipboard_history.json")
}

fn notifications_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("/tmp"));
    let dir = home.join(".config/Blue-Environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("notifications.json")
}

// ── Session ────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_session_type() -> String {
    session::session_info()
}

// ── Apps ───────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_system_apps(force_refresh: bool) -> Vec<CachedApp> {
    apps::scan_desktop_apps(force_refresh)
}

#[tauri::command]
fn get_recent_apps() -> Vec<String> {
    cache::get_recent_apps()
}

#[tauri::command]
fn record_app_launch(app_id: String) {
    cache::record_app_launch(&app_id);
}

#[tauri::command]
fn invalidate_app_cache() {
    cache::invalidate_app_cache();
}

#[tauri::command]
fn launch_process(command: String, app_id: Option<String>) {
    if let Some(id) = app_id {
        cache::record_app_launch(&id);
    }
    let session = session::detect_session();
    std::thread::spawn(move || {
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        match session {
            session::SessionType::Tty => {
                cmd.env("WAYLAND_DISPLAY", "wayland-blue-1")
                .arg(format!("{} &", command));
            }
            _ => {
                cmd.arg(format!("{} &", command));
            }
        }
        let _ = cmd.spawn();
    });
}

// ── External window management ─────────────────────────────────────────────

#[tauri::command]
fn get_external_windows() -> Vec<window_tracker::ExternalWindow> {
    window_tracker::get_external_windows()
}

#[tauri::command]
fn focus_external_window(win_id: String) {
    window_tracker::focus_window(&win_id);
}

#[tauri::command]
fn minimize_external_window(win_id: String) {
    window_tracker::minimize_window(&win_id);
}

#[tauri::command]
fn close_external_window(win_id: String) {
    window_tracker::close_window(&win_id);
}

#[tauri::command]
fn embed_external_window(win_id: String, _parent_id: String) -> bool {
    let session = session::detect_session();
    match session {
        session::SessionType::X11Client => {
            let _ = Command::new("xdotool")
            .args(["windowfocus", "--sync", &win_id])
            .spawn();
            true
        }
        session::SessionType::WaylandClient => {
            let _ = Command::new("swaymsg")
            .args(["[pid=", &win_id, "]", "focus"])
            .spawn();
            false
        }
        session::SessionType::Tty => true,
    }
}

// ── Files ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn list_files(path: String) -> Vec<FileEntry> {
    let target = if path == "HOME" {
        dirs::home_dir().unwrap_or(PathBuf::from("/"))
    } else {
        PathBuf::from(&path)
    };

    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(&target) {
        for entry in rd.flatten() {
            if let Ok(meta) = entry.metadata() {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = meta.is_dir();
                let size = if is_dir {
                    "DIR".to_string()
                } else {
                    format!("{:.1} KB", meta.len() as f64 / 1024.0)
                };
                let mime_type = if is_dir {
                    "inode/directory".to_string()
                } else {
                    let ext = entry.path().extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                    match ext.as_str() {
                        "png"|"jpg"|"jpeg"|"gif"|"webp"|"svg" => "image",
                        "mp4"|"mkv"|"webm"|"avi"|"mov"        => "video",
                        "mp3"|"wav"|"ogg"|"flac"|"aac"         => "audio",
                        "pdf"                                    => "application/pdf",
                        "txt"|"md"|"rs"|"ts"|"js"|"py"|"toml"  => "text",
                        _                                        => "application/octet-stream",
                    }.to_string()
                };
                let modified = meta.modified().ok().map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
                });
                entries.push(FileEntry { name, path: entry.path().to_string_lossy().to_string(), is_dir, size, mime_type, modified });
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
fn read_text_file(path: String) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| format!("Error: {}", e))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn git_status(path: String) -> Vec<String> {
    Command::new("git")
    .args(["-C", &path, "status", "--short"])
    .output()
    .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.to_string()).collect())
    .unwrap_or_default()
}

// ── System stats ───────────────────────────────────────────────────────────

#[tauri::command]
fn get_system_stats() -> SystemStats {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let volume = get_pipewire_volume().unwrap_or_else(get_alsa_volume);
    let wifi_ssid = Command::new("nmcli")
    .args(["-t", "-f", "active,ssid", "dev", "wifi"])
    .output()
    .map(|o| {
        String::from_utf8_lossy(&o.stdout)
        .lines()
        .find(|l| l.starts_with("yes:"))
        .map(|l| l.replacen("yes:", "", 1))
        .unwrap_or_else(|| "Disconnected".to_string())
    })
    .unwrap_or("Unknown".to_string());

    let kernel = Command::new("uname").arg("-r")
    .output()
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    .unwrap_or("Unknown".to_string());

    let (battery, is_charging) = get_battery_info();

    SystemStats {
        cpu: sys.global_cpu_info().cpu_usage(),  // FIX: was global_cpu_usage()
        ram: (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0,
        battery,
        is_charging,
        volume,
        brightness: get_brightness(),
        wifi_ssid,
        kernel,
        session_type: session::session_info(),
    }
}

fn get_battery_info() -> (f32, bool) {
    let bat_paths = ["/sys/class/power_supply/BAT0", "/sys/class/power_supply/BAT1", "/sys/class/power_supply/battery"];
    for bat in &bat_paths {
        let cap_path = PathBuf::from(bat).join("capacity");
        let status_path = PathBuf::from(bat).join("status");
        if let Ok(cap) = fs::read_to_string(&cap_path) {
            let level: f32 = cap.trim().parse().unwrap_or(0.0);
            let charging = fs::read_to_string(&status_path)
            .map(|s| s.trim() == "Charging" || s.trim() == "Full")
            .unwrap_or(false);
            return (level, charging);
        }
    }
    (100.0, true)
}

fn get_brightness() -> i32 {
    let paths = ["/sys/class/backlight/intel_backlight", "/sys/class/backlight/amdgpu_bl0", "/sys/class/backlight/acpi_video0"];
    for p in &paths {
        let cur = fs::read_to_string(format!("{}/brightness", p));
        let max = fs::read_to_string(format!("{}/max_brightness", p));
        if let (Ok(c), Ok(m)) = (cur, max) {
            let c: f32 = c.trim().parse().unwrap_or(0.0);
            let m: f32 = m.trim().parse().unwrap_or(1.0);
            return ((c / m) * 100.0) as i32;
        }
    }
    -1
}

fn get_pipewire_volume() -> Option<i32> {
    let out = Command::new("pactl").args(["get-sink-volume", "@DEFAULT_SINK@"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split('%').next()?.rsplit('/').next()?.trim().parse().ok()
}

fn get_alsa_volume() -> i32 {
    Command::new("sh")
    .arg("-c")
    .arg("amixer get Master 2>/dev/null | grep -o '[0-9]*%' | head -1")
    .output()
    .map(|o| String::from_utf8_lossy(&o.stdout).replace('%', "").trim().parse().unwrap_or(50))
    .unwrap_or(50)
}

#[tauri::command]
fn get_audio_sinks() -> Vec<AudioSink> {
    let mut sinks = Vec::new();
    let default_out = Command::new("pactl").args(["get-default-sink"]).output()
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    .unwrap_or_default();

    if let Ok(o) = Command::new("pactl").args(["--format=json", "list", "sinks"]).output() {
        let text = String::from_utf8_lossy(&o.stdout);
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(arr) = arr.as_array() {
                for s in arr {
                    let id = s["index"].as_u64().unwrap_or(0) as u32;
                    let name = s["name"].as_str().unwrap_or("").to_string();
                    let desc = s["description"].as_str().unwrap_or("").to_string();
                    let muted = s["mute"].as_bool().unwrap_or(false);
                    let vol_left = s["volume"]["front-left"]["value_percent"]
                    .as_str()
                    .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
                    .unwrap_or(0.0);
                    sinks.push(AudioSink { id, is_default: name == default_out, name, description: desc, volume: vol_left, muted });
                }
            }
        }
    }
    sinks
}

#[tauri::command]
fn set_sink_volume(sink_name: String, volume: f32) -> Result<(), String> {
    Command::new("pactl").args(["set-sink-volume", &sink_name, &format!("{}%", volume.clamp(0.0, 150.0) as u32)]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn set_default_sink(sink_name: String) -> Result<(), String> {
    Command::new("pactl").args(["set-default-sink", &sink_name]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn toggle_sink_mute(sink_name: String) -> Result<(), String> {
    Command::new("pactl").args(["set-sink-mute", &sink_name, "toggle"]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn set_volume(level: i32) {
    let _ = Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", level.clamp(0, 150))]).spawn();
}

// ── Wi-Fi ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_wifi_networks_real() -> Vec<WifiNetwork> {
    let mut networks = Vec::new();
    let _ = Command::new("nmcli").args(["dev", "wifi", "rescan"]).output();
    if let Ok(o) = Command::new("nmcli").args(["-t", "-f", "IN-USE,BSSID,SSID,MODE,CHAN,FREQ,RATE,SIGNAL,BARS,SECURITY", "dev", "wifi", "list"]).output() {
        let text = String::from_utf8_lossy(&o.stdout);
        let mut seen = std::collections::HashSet::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.splitn(10, ':').collect();
            if parts.len() < 9 { continue; }
            let ssid = parts[2].to_string();
            if ssid.is_empty() || seen.contains(&ssid) { continue; }
            seen.insert(ssid.clone());
            networks.push(WifiNetwork {
                in_use: parts[0] == "*",
                bssid: parts[1].to_string(),
                          ssid,
                          frequency: parts[5].to_string(),
                          signal: parts[7].parse().unwrap_or(0),
                          secure: parts.get(9).map(|s| !s.is_empty() && *s != "--").unwrap_or(false),
            });
        }
    }
    networks.sort_by(|a, b| b.signal.cmp(&a.signal));
    networks
}

#[tauri::command]
fn connect_wifi_real(ssid: String, password: String) -> Result<String, String> {
    let args = if password.is_empty() {
        vec!["dev".to_string(), "wifi".to_string(), "connect".to_string(), ssid]
    } else {
        vec!["dev".to_string(), "wifi".to_string(), "connect".to_string(), ssid, "password".to_string(), password]
    };
    let o = Command::new("nmcli").args(&args).output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(String::from_utf8_lossy(&o.stdout).to_string()) }
    else { Err(String::from_utf8_lossy(&o.stderr).to_string()) }
}

#[tauri::command]
fn disconnect_wifi() -> Result<(), String> {
    Command::new("nmcli").args(["dev", "disconnect", "wlan0"]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn toggle_wifi(enabled: bool) {
    let _ = Command::new("nmcli").args(["radio", "wifi", if enabled { "on" } else { "off" }]).spawn();
}

// ── Bluetooth ──────────────────────────────────────────────────────────────

#[tauri::command]
fn get_bluetooth_devices_real() -> Vec<BluetoothDevice> {
    let mut devices = Vec::new();
    if let Ok(o) = Command::new("bluetoothctl").arg("devices").output() {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if !line.starts_with("Device ") { continue; }
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() < 3 { continue; }
            let mac = parts[1].to_string();
            let name = parts[2].to_string();
            let info = Command::new("bluetoothctl").args(["info", &mac]).output()
            .map(|i| String::from_utf8_lossy(&i.stdout).to_string())
            .unwrap_or_default();
            devices.push(BluetoothDevice {
                name, mac,
                connected: info.contains("Connected: yes"),
                         paired: info.contains("Paired: yes"),
                         trusted: info.contains("Trusted: yes"),
                         device_type: info.lines().find(|l| l.trim_start().starts_with("Icon:"))
                         .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
                         .unwrap_or("unknown".to_string()),
                         battery: info.lines().find(|l| l.trim_start().starts_with("Battery Percentage:"))
                         .and_then(|l| l.split('(').nth(1).and_then(|s| s.trim_end_matches(')').parse::<u8>().ok())),
            });
        }
    }
    devices
}

#[tauri::command]
fn bluetooth_connect(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl").args(["connect", &mac]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn bluetooth_disconnect(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl").args(["disconnect", &mac]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn bluetooth_pair(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl").args(["pair", &mac]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Power Profiles ─────────────────────────────────────────────────────────

#[tauri::command]
fn get_power_profiles() -> Vec<PowerProfile> {
    let mut profiles = Vec::new();
    let out = Command::new("powerprofilesctl").arg("list").output();
    let (has_saver, has_balanced, has_perf, active) = if let Ok(o) = out {
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        let active = text.lines().find(|l| l.contains('*'))
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or("").trim_end_matches(':').to_string();
        (text.contains("power-saver"), text.contains("balanced"), text.contains("performance"), active)
    } else { (false, false, false, "balanced".to_string()) };

    if has_saver || !has_balanced {
        profiles.push(PowerProfile { name: "power-saver".to_string(), active: active == "power-saver", icon: Some("Battery".to_string()), description: "Oszczędzanie energii".to_string() });
    }
    profiles.push(PowerProfile { name: "balanced".to_string(), active: active == "balanced" || active.is_empty(), icon: Some("Wind".to_string()), description: "Zrównoważony".to_string() });
    if has_perf {
        profiles.push(PowerProfile { name: "performance".to_string(), active: active == "performance", icon: Some("Zap".to_string()), description: "Wydajność".to_string() });
    }
    profiles
}

#[tauri::command]
fn set_power_profile(profile: String) -> Result<(), String> {
    Command::new("powerprofilesctl").args(["set", &profile]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Brightness ─────────────────────────────────────────────────────────────

#[tauri::command]
fn set_brightness(level: i32) {
    if Command::new("brightnessctl").args(["set", &format!("{}%", level)]).spawn().is_err() {
        let _ = Command::new("sh").arg("-c")
        .arg(format!("xrandr --output $(xrandr | grep ' connected' | head -1 | cut -d' ' -f1) --brightness {:.2}", level as f32 / 100.0))
        .spawn();
    }
}

// ── System ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_processes() -> Vec<ProcessEntry> {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_processes();  // FIX: was refresh_processes(ProcessesToUpdate::All, true)
    let mut procs: Vec<ProcessEntry> = sys.processes().iter().map(|(pid, p)| ProcessEntry {
        pid: pid.to_string(),
                                                                  name: p.name().to_string(),  // FIX: was to_string_lossy().to_string()
    cpu: p.cpu_usage(),
                                                                  memory: p.memory(),
    }).collect();
    procs.sort_by(|a, b| b.memory.cmp(&a.memory));
    procs.truncate(50);
    procs
}

#[tauri::command]
fn take_screenshot() {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("/"));
    let pics = home.join("Pictures");
    let _ = fs::create_dir_all(&pics);
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = pics.join(format!("screenshot-{}.png", ts)).to_string_lossy().to_string();
    let cmd = format!(
        "flameshot gui -p '{p}' 2>/dev/null || scrot '{p}' 2>/dev/null || gnome-screenshot -f '{p}' 2>/dev/null || spectacle -b -o '{p}' 2>/dev/null",
        p = path
    );
    let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
}

// ── Wallpapers ─────────────────────────────────────────────────────────────

#[tauri::command]
fn get_wallpapers() -> Vec<String> {
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
fn get_wallpaper_preview(path: String) -> Result<String, String> {
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
fn load_distro_info() -> std::collections::HashMap<String, String> {
    let mut info = std::collections::HashMap::new();
    info.insert("Name".to_string(), "HackerOS".to_string());
    info.insert("Version".to_string(), "0.2.0-alpha".to_string());
    info.insert("Copyright".to_string(), "© 2026 HackerOS Team".to_string());
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
fn system_power(action: String) {
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

// ── Config ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn save_config(config: String) {
    let parsed: cache::UserConfig = serde_json::from_str(&config).unwrap_or_default();
    cache::save_user_config(&parsed);
}

#[tauri::command]
fn load_config() -> String {
    cache::load_user_config()
}

#[tauri::command]
fn save_window_state(windows: Vec<cache::WindowCache>) {
    cache::save_window_state(&windows);
}

#[tauri::command]
fn load_window_state() -> Vec<cache::WindowCache> {
    cache::load_window_state()
}

// ── File operations ────────────────────────────────────────────────────────

#[tauri::command]
fn read_file_as_data_url(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.exists() { return Err("File not found".to_string()); }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{};base64,{}", mime, BASE64.encode(data)))
}

#[tauri::command]
fn create_folder(path: String, name: String) -> Result<(), String> {
    fs::create_dir_all(PathBuf::from(path).join(name)).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if path.is_dir() { fs::remove_dir_all(path).map_err(|e| e.to_string()) }
    else { fs::remove_file(path).map_err(|e| e.to_string()) }
}

#[tauri::command]
fn copy_file(src: String, dest: String) -> Result<(), String> {
    fs::copy(src, dest).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn move_file(src: String, dest: String) -> Result<(), String> {
    fs::rename(src, dest).map_err(|e| e.to_string())
}

// ── Terminal ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn execute_command(command: String) -> Result<CommandOutput, String> {
    let output = TokioCommand::new("sh").arg("-c").arg(&command).output().await.map_err(|e| e.to_string())?;
    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
       stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[tauri::command]
async fn spawn_terminal(window: Window, window_id: String) -> Result<bool, String> {
    let mut child = TokioCommand::new("bash")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .map_err(|e| e.to_string())?;

    let stdin = child.stdin.take().ok_or("Failed to open stdin")?;
    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to open stderr")?;

    let window_clone = window.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = window_clone.emit("terminal-output", serde_json::json!({"type":"stdout","data":line+"\r\n"}));
        }
    });

    let window_clone = window.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = window_clone.emit("terminal-output", serde_json::json!({"type":"stderr","data":line+"\r\n"}));
        }
    });

    TERMINALS.lock().await.insert(window_id, TerminalProcess { stdin });
    Ok(true)
}

#[tauri::command]
async fn write_to_terminal(window_id: String, command: String) -> Result<(), String> {
    let mut terminals = TERMINALS.lock().await;
    if let Some(term) = terminals.get_mut(&window_id) {
        let cmd = command + "\n";
        term.stdin.write_all(cmd.as_bytes()).await.map_err(|e| e.to_string())?;
        term.stdin.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Terminal not found".to_string())
    }
}

// ── Desktop path ───────────────────────────────────────────────────────────

#[tauri::command]
fn get_default_desktop_path() -> String {
    if let Ok(output) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() { return path; }
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    for dir_name in &["Desktop", "Pulpit"] {
        let dir = home.join(dir_name);
        if dir.exists() { return dir.to_string_lossy().to_string(); }
    }
    "HOME/Desktop".to_string()
}

#[tauri::command]
fn create_text_file(path: String, name: String, content: String) -> Result<(), String> {
    fs::write(PathBuf::from(path).join(name), content).map_err(|e| e.to_string())
}

// ── Clipboard history ─────────────────────────────────────────────────────

#[tauri::command]
fn get_clipboard_history() -> Vec<ClipboardItem> {
    fs::read_to_string(clipboard_history_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
fn add_to_clipboard_history(content: String) {
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
fn clear_clipboard_history() {
    let _ = fs::write(clipboard_history_path(), "[]");
}

// ── Night Light ────────────────────────────────────────────────────────────

#[tauri::command]
fn set_night_light_enabled(enabled: bool) -> Result<(), String> {
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
fn set_night_light_temperature(temperature: u32) -> Result<(), String> {
    let factor = temperature as f32 / 6500.0;
    let gamma = format!("{:.2}:{:.2}:{:.2}", 1.0f32, factor * 0.8, factor * 0.6);
    match session::detect_session() {
        session::SessionType::WaylandClient => { let _ = Command::new("wlr-randr").args(["--output", "eDP-1", "--gamma", &gamma]).spawn(); }
        session::SessionType::X11Client => { let _ = Command::new("xrandr").args(["--output", "eDP-1", "--gamma", &gamma]).spawn(); }
        _ => {}
    }
    Ok(())
}

// ── Notifications ──────────────────────────────────────────────────────────

#[tauri::command]
fn get_notification_history() -> Vec<Notification> {
    fs::read_to_string(notifications_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
fn save_notification_history(notifications: Vec<Notification>) {
    let _ = fs::write(notifications_path(), serde_json::to_string(&notifications).unwrap());
}

// ── Custom themes ─────────────────────────────────────────────────────────

#[tauri::command]
fn get_custom_themes() -> Vec<cache::ThemeDefinition> {
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/themes.json");
    fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
fn save_custom_theme(theme: cache::ThemeDefinition) {
    let mut themes = get_custom_themes();
    themes.retain(|t| t.id != theme.id);
    themes.push(theme);
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/themes.json");
    let _ = fs::write(path, serde_json::to_string_pretty(&themes).unwrap());
}

#[tauri::command]
fn delete_custom_theme(theme_id: String) {
    let mut themes = get_custom_themes();
    themes.retain(|t| t.id != theme_id);
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/themes.json");
    let _ = fs::write(path, serde_json::to_string_pretty(&themes).unwrap());
}

// ── AI ─────────────────────────────────────────────────────────────────────

#[tauri::command]
async fn ai_call(request: ai::AICallRequest) -> Result<String, String> {
    ai::ai_call(request).await
}

#[tauri::command]
async fn get_ai_config() -> Result<Option<ai::AIConfig>, String> {
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/ai_config.json");
    Ok(fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<ai::AIConfig>(&s).ok()))
}

#[tauri::command]
async fn save_ai_config(config: ai::AIConfig) -> Result<(), String> {
    let path = dirs::home_dir().unwrap_or(PathBuf::from("/tmp")).join(".config/Blue-Environment/ai_config.json");
    fs::write(path, serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

// ── Package managers ───────────────────────────────────────────────────────

#[tauri::command]
async fn get_apt_packages() -> Vec<ai::PackageInfo> { vec![] }
#[tauri::command]
async fn get_flatpak_packages() -> Vec<ai::PackageInfo> { vec![] }
#[tauri::command]
async fn get_snap_packages() -> Vec<ai::PackageInfo> { vec![] }
#[tauri::command]
async fn get_appimage_packages() -> Vec<ai::PackageInfo> { vec![] }
#[tauri::command]
async fn install_apt_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn remove_apt_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn update_apt_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn install_flatpak_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn remove_flatpak_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn update_flatpak_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn install_snap_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn remove_snap_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn update_snap_package(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn install_appimage(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn remove_appimage(_pkg_id: String) -> Result<bool, String> { Ok(false) }
#[tauri::command]
async fn update_appimage(_pkg_id: String) -> Result<bool, String> { Ok(false) }

// ── Panel ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn set_panel_enabled(enabled: bool) -> Result<(), String> {
    println!("Panel enabled: {}", enabled);
    Ok(())
}

// ── PTY ───────────────────────────────────────────────────────────────────

struct PtySession {
    writer: Box<dyn std::io::Write + Send>,
    pid: u32,
}
type PtySessions = std::sync::Arc<StdMutex<HashMap<String, PtySession>>>;

#[tauri::command]
fn pty_create(
    id: String,
    shell: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
    window: tauri::Window,
    sessions: tauri::State<PtySessions>,
) -> Result<(), String> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: rows.unwrap_or(24),
                                  cols: cols.unwrap_or(80),
                                  pixel_width: 0,
                                  pixel_height: 0,
    }).map_err(|e| e.to_string())?;

    let shell_path = shell.unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()));
    let mut cmd = CommandBuilder::new(&shell_path);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if let Ok(home) = std::env::var("HOME") { cmd.env("HOME", &home); }
    if let Ok(user) = std::env::var("USER") { cmd.env("USER", &user); }
    cmd.cwd(std::env::var("HOME").unwrap_or_else(|_| "/".to_string()));

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    let id_clone = id.clone();
    let win = window.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = win.emit(&format!("pty-exit-{}", id_clone), ());
                    break;
                }
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = win.emit(&format!("pty-data-{}", id_clone), data);
                }
            }
        }
    });

    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    let pid = child.process_id().unwrap_or(0);
    map.insert(id, PtySession { writer: Box::new(writer), pid });
    Ok(())
}

#[tauri::command]
fn pty_write(id: String, data: String, sessions: tauri::State<PtySessions>) -> Result<(), String> {
    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    if let Some(session) = map.get_mut(&id) {
        session.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        session.writer.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn pty_resize(_id: String, _cols: u16, _rows: u16, _sessions: tauri::State<PtySessions>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn pty_close(id: String, sessions: tauri::State<PtySessions>) -> Result<(), String> {
    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    if let Some(session) = map.remove(&id) {
        if session.pid > 0 {
            unsafe { libc::kill(session.pid as i32, libc::SIGTERM); }
        }
    }
    Ok(())
}

// ── Config files ───────────────────────────────────────────────────────────

#[tauri::command]
fn read_config_file(filename: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".config/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_config_file(filename: String, content: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".config/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_cache_file(filename: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".cache/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_cache_file(filename: String, content: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("No home dir")?;
    let path = home.join(".cache/Blue-Environment").join(&filename);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    cache::ensure_dirs();

    let config = cache::load_user_config();
    let config_parsed: cache::UserConfig = serde_json::from_str(&config).unwrap_or_default();

    if config_parsed.panel_enabled {
        start_panel();
    }

    tauri::Builder::default()
    .manage(std::sync::Arc::new(StdMutex::new(HashMap::<String, PtySession>::new())))
    .invoke_handler(tauri::generate_handler![
        get_session_type,
        get_system_apps, get_recent_apps, record_app_launch, invalidate_app_cache, launch_process,
        get_external_windows, focus_external_window, minimize_external_window, close_external_window, embed_external_window,
        list_files, read_text_file, write_text_file, git_status,
        get_system_stats, get_processes,
        read_config_file, write_config_file, read_cache_file, write_cache_file,
        take_screenshot, get_wallpapers, get_wallpaper_preview, load_distro_info, system_power,
        get_audio_sinks, set_sink_volume, set_default_sink, toggle_sink_mute, set_volume,
        get_wifi_networks_real, connect_wifi_real, disconnect_wifi, toggle_wifi,
        get_bluetooth_devices_real, bluetooth_connect, bluetooth_disconnect, bluetooth_pair,
        get_power_profiles, set_power_profile,
        set_brightness,
        save_config, load_config, save_window_state, load_window_state,
        read_file_as_data_url, create_folder, delete_file, copy_file, move_file,
        execute_command, pty_create, pty_write, pty_resize, pty_close, spawn_terminal, write_to_terminal,
        get_default_desktop_path, create_text_file,
        get_clipboard_history, add_to_clipboard_history, clear_clipboard_history,
        set_night_light_enabled, set_night_light_temperature,
        get_notification_history, save_notification_history,
        get_custom_themes, save_custom_theme, delete_custom_theme,
        ai_call, get_ai_config, save_ai_config,
        get_apt_packages, get_flatpak_packages, get_snap_packages, get_appimage_packages,
        install_apt_package, remove_apt_package, update_apt_package,
        install_flatpak_package, remove_flatpak_package, update_flatpak_package,
        install_snap_package, remove_snap_package, update_snap_package,
        install_appimage, remove_appimage, update_appimage,
        set_panel_enabled,
    ])
    .run(tauri::generate_context!())
    .expect("error while running Blue Environment");
}

fn start_panel() {
    let panel_path = std::env::current_exe()
    .unwrap_or_default()
    .parent()
    .unwrap_or(std::path::Path::new("/"))
    .join("blue-panel");
    if panel_path.exists() {
        let _ = Command::new(panel_path).spawn();
    }
}
