#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod session;
mod cache;
mod apps;
mod window_tracker;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::collections::HashMap;
use std::sync::Arc;
use glob::glob;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cache::CachedApp;
use tokio::process::Command as TokioCommand;
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use tokio::sync::Mutex;
use tauri::Window;
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
    static ref TERMINALS: Arc<Mutex<HashMap<String, TerminalProcess>>> = Arc::new(Mutex::new(HashMap::new()));
}

fn clipboard_history_path() -> PathBuf {
    let home = dirs::home_dir().unwrap();
    home.join(".local/share/blue-env/clipboard_history.json")
}

fn notifications_path() -> PathBuf {
    let home = dirs::home_dir().unwrap();
    home.join(".local/share/blue-env/notifications.json")
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
            session::SessionType::WaylandClient => {
                cmd.arg(format!("{} &", command));
            }
            session::SessionType::X11Client => {
                cmd.arg(format!("{} &", command));
            }
            session::SessionType::Tty => {
                cmd.env("WAYLAND_DISPLAY", "wayland-blue-1")
                .arg(format!("{} &", command));
            }
        }

        let _ = cmd.spawn();
    });
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
                        "mp3"|"wav"|"ogg"|"flac"|"aac"        => "audio",
                        "pdf"                                   => "application/pdf",
                        "txt"|"md"|"rs"|"ts"|"js"|"py"|"toml" => "text",
                        _                                       => "application/octet-stream",
                    }.to_string()
                };

                entries.push(FileEntry {
                    name,
                    path: entry.path().to_string_lossy().to_string(),
                             is_dir,
                             size,
                             mime_type,
                });
            }
        }
    }

    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    entries
}

#[tauri::command]
fn read_text_file(path: String) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| format!("Error: {}", e))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}

// ── System stats ───────────────────────────────────────────────────────────

#[tauri::command]
fn get_system_stats() -> SystemStats {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let volume = get_pipewire_volume().unwrap_or_else(|| get_alsa_volume());

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
        cpu: sys.global_cpu_info().cpu_usage(),
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
    let bat_paths = [
        "/sys/class/power_supply/BAT0",
        "/sys/class/power_supply/BAT1",
        "/sys/class/power_supply/battery",
    ];
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
    let paths = [
        "/sys/class/backlight/intel_backlight",
        "/sys/class/backlight/amdgpu_bl0",
        "/sys/class/backlight/acpi_video0",
    ];
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

// ── PipeWire/PulseAudio audio ──────────────────────────────────────────────

fn get_pipewire_volume() -> Option<i32> {
    let out = Command::new("pactl")
    .args(["get-sink-volume", "@DEFAULT_SINK@"])
    .output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let vol: i32 = text.split('%')
    .next()?
    .rsplit('/')
    .next()?
    .trim()
    .parse().ok()?;
    Some(vol)
}

fn get_alsa_volume() -> i32 {
    Command::new("sh")
    .arg("-c")
    .arg("amixer get Master 2>/dev/null | grep -o '[0-9]*%' | head -1")
    .output()
    .map(|o| {
        String::from_utf8_lossy(&o.stdout)
        .replace('%', "")
        .trim()
        .parse()
        .unwrap_or(50)
    })
    .unwrap_or(50)
}

#[tauri::command]
fn get_audio_sinks() -> Vec<AudioSink> {
    let mut sinks = Vec::new();

    let default_out = Command::new("pactl")
    .args(["get-default-sink"])
    .output()
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    .unwrap_or_default();

    let out = Command::new("pactl")
    .args(["--format=json", "list", "sinks"])
    .output();

    if let Ok(o) = out {
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

                    sinks.push(AudioSink {
                        id,
                        is_default: name == default_out,
                        name,
                        description: desc,
                        volume: vol_left,
                        muted,
                    });
                }
            }
        }
    }
    sinks
}

#[tauri::command]
fn set_sink_volume(sink_name: String, volume: f32) -> Result<(), String> {
    let vol_pct = format!("{}%", volume.clamp(0.0, 150.0) as u32);
    Command::new("pactl")
    .args(["set-sink-volume", &sink_name, &vol_pct])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn set_default_sink(sink_name: String) -> Result<(), String> {
    Command::new("pactl")
    .args(["set-default-sink", &sink_name])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn toggle_sink_mute(sink_name: String) -> Result<(), String> {
    Command::new("pactl")
    .args(["set-sink-mute", &sink_name, "toggle"])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn set_volume(level: i32) {
    let pct = format!("{}%", level.clamp(0, 150));
    let _ = Command::new("pactl")
    .args(["set-sink-volume", "@DEFAULT_SINK@", &pct])
    .spawn();
}

// ── Wi-Fi via nmcli ────────────────────────────────────────────────────────

#[tauri::command]
fn get_wifi_networks_real() -> Vec<WifiNetwork> {
    let mut networks = Vec::new();

    let _ = Command::new("nmcli").args(["dev", "wifi", "rescan"]).output();

    let out = Command::new("nmcli")
    .args([
        "-t", "-f",
        "IN-USE,BSSID,SSID,MODE,CHAN,FREQ,RATE,SIGNAL,BARS,SECURITY",
        "dev", "wifi", "list",
    ])
    .output();

    if let Ok(o) = out {
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

    let o = Command::new("nmcli")
    .args(&args)
    .output()
    .map_err(|e| e.to_string())?;

    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).to_string())
    }
}

#[tauri::command]
fn disconnect_wifi() -> Result<(), String> {
    Command::new("nmcli")
    .args(["dev", "disconnect", "wlan0"])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn toggle_wifi(enabled: bool) {
    let state = if enabled { "on" } else { "off" };
    let _ = Command::new("nmcli")
    .args(["radio", "wifi", state])
    .spawn();
}

// ── Bluetooth ──────────────────────────────────────────────────────────────

#[tauri::command]
fn get_bluetooth_devices_real() -> Vec<BluetoothDevice> {
    let mut devices = Vec::new();

    let out = Command::new("bluetoothctl").arg("devices").output();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if !line.starts_with("Device ") { continue; }
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() < 3 { continue; }

            let mac = parts[1].to_string();
            let name = parts[2].to_string();

            let info = Command::new("bluetoothctl")
            .args(["info", &mac])
            .output()
            .map(|i| String::from_utf8_lossy(&i.stdout).to_string())
            .unwrap_or_default();

            let connected = info.contains("Connected: yes");
            let paired = info.contains("Paired: yes");
            let trusted = info.contains("Trusted: yes");

            let device_type = info.lines()
            .find(|l| l.trim_start().starts_with("Icon:"))
            .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
            .unwrap_or("unknown".to_string());

            let battery = info.lines()
            .find(|l| l.trim_start().starts_with("Battery Percentage:"))
            .and_then(|l| {
                l.split('(').nth(1)
                .and_then(|s| s.trim_end_matches(')').parse::<u8>().ok())
            });

            devices.push(BluetoothDevice { name, mac, device_type, connected, paired, trusted, battery });
        }
    }
    devices
}

#[tauri::command]
fn bluetooth_connect(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl")
    .args(["connect", &mac])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn bluetooth_disconnect(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl")
    .args(["disconnect", &mac])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn bluetooth_pair(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl")
    .args(["pair", &mac])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Power Profiles ─────────────────────────────────────────────────────────

#[tauri::command]
fn get_power_profiles() -> Vec<PowerProfile> {
    let mut profiles = Vec::new();

    let out = Command::new("powerprofilesctl")
    .arg("list")
    .output();

    if let Ok(o) = out {
        let text = String::from_utf8_lossy(&o.stdout);
        let mut active = "";

        for line in text.lines() {
            if line.contains("*") {
                active = line.split_whitespace().next().unwrap_or("");
            }
        }

        if text.contains("power-saver") {
            profiles.push(PowerProfile {
                name: "power-saver".to_string(),
                          active: active == "power-saver:",
                          icon: Some("Battery".to_string()),
                          description: "Oszczędzanie energii".to_string(),
            });
        }
        if text.contains("balanced") {
            profiles.push(PowerProfile {
                name: "balanced".to_string(),
                          active: active == "balanced:",
                          icon: Some("Wind".to_string()),
                          description: "Zrównoważony".to_string(),
            });
        }
        if text.contains("performance") {
            profiles.push(PowerProfile {
                name: "performance".to_string(),
                          active: active == "performance:",
                          icon: Some("Zap".to_string()),
                          description: "Wydajność".to_string(),
            });
        }
    }

    if profiles.is_empty() {
        profiles.push(PowerProfile {
            name: "power-saver".to_string(),
                      active: false,
                      icon: Some("Battery".to_string()),
                      description: "Oszczędzanie energii".to_string(),
        });
        profiles.push(PowerProfile {
            name: "balanced".to_string(),
                      active: true,
                      icon: Some("Wind".to_string()),
                      description: "Zrównoważony".to_string(),
        });
        profiles.push(PowerProfile {
            name: "performance".to_string(),
                      active: false,
                      icon: Some("Zap".to_string()),
                      description: "Wydajność".to_string(),
        });
    }

    profiles
}

#[tauri::command]
fn set_power_profile(profile: String) -> Result<(), String> {
    Command::new("powerprofilesctl")
    .args(["set", &profile])
    .spawn()
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Brightness ─────────────────────────────────────────────────────────────

#[tauri::command]
fn set_brightness(level: i32) {
    if Command::new("brightnessctl")
        .args(["set", &format!("{}%", level)])
        .spawn()
        .is_err()
        {
            let _ = Command::new("sh")
            .arg("-c")
            .arg(format!("xrandr --output $(xrandr | grep ' connected' | head -1 | cut -d' ' -f1) --brightness {:.2}", level as f32 / 100.0))
            .spawn();
        }
}

// ── System ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_processes() -> Vec<ProcessEntry> {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_processes();

    let mut procs: Vec<ProcessEntry> = sys.processes().iter().map(|(pid, p)| ProcessEntry {
        pid: pid.to_string(),
                                                                  name: p.name().to_string(),
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
        "flameshot gui -p '{path}' 2>/dev/null || \
scrot '{path}' 2>/dev/null || \
gnome-screenshot -f '{path}' 2>/dev/null || \
spectacle -b -o '{path}' 2>/dev/null",
path = path
    );
    let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
}

// ── Wallpapers ─────────────────────────────────────────────────────────────

#[tauri::command]
fn get_wallpapers() -> Vec<String> {
    let mut wallpapers: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let default_path = std::path::Path::new("/usr/share/wallpapers/default.png");
    if default_path.exists() {
        wallpapers.push(format!("file://{}", default_path.to_string_lossy()));
        seen.insert("default.png".to_string());
    }

    let search_patterns = [
        "/usr/share/wallpapers/*.png",
        "/usr/share/wallpapers/*.jpg",
        "/usr/share/wallpapers/*.jpeg",
        "/usr/share/wallpapers/**/*.png",
        "/usr/share/wallpapers/**/*.jpg",
        "/usr/share/backgrounds/*.png",
        "/usr/share/backgrounds/*.jpg",
        "/usr/share/backgrounds/**/*.png",
        "/usr/share/backgrounds/**/*.jpg",
    ];

    for pat in &search_patterns {
        if let Ok(entries) = glob(pat) {
            for entry in entries.filter_map(Result::ok) {
                let fname = entry.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
                if !seen.contains(&fname) {
                    seen.insert(fname.clone());
                    wallpapers.push(format!("file://{}", entry.to_string_lossy()));
                }
            }
        }
    }

    if wallpapers.is_empty() {
        wallpapers.push("file:///usr/share/wallpapers/default.png".to_string());
    }

    wallpapers
}

#[tauri::command]
fn get_wallpaper_preview(path: String) -> Result<String, String> {
    let path = path.replace("file://", "");
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("File not found".to_string());
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };
    let base64 = BASE64.encode(data);
    Ok(format!("data:{};base64,{}", mime, base64))
}

#[tauri::command]
fn load_distro_info() -> std::collections::HashMap<String, String> {
    let mut info = std::collections::HashMap::new();
    info.insert("Name".to_string(), "HackerOS".to_string());
    info.insert("Version".to_string(), "0.2.0-alpha".to_string());
    info.insert("Copyright".to_string(), "© 2026 HackerOS Team".to_string());

    let paths = [
        "/etc/xdg/kcm-about-distrorc",
        "/etc/os-release",
    ];
    for p in &paths {
        if let Ok(content) = fs::read_to_string(p) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    let v = v.trim_matches('"').to_string();
                    info.entry(k.trim().to_string()).or_insert(v);
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

// ── Window state persistence ───────────────────────────────────────────────

#[tauri::command]
fn save_window_state(windows: Vec<cache::WindowCache>) {
    cache::save_window_state(&windows);
}

#[tauri::command]
fn load_window_state() -> Vec<cache::WindowCache> {
    cache::load_window_state()
}

// ── External window tracking ────────────────────────────────────────────────

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

// ── File operations for Explorer ───────────────────────────────────────────

#[tauri::command]
fn read_file_as_data_url(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("File not found".to_string());
    }
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("txt") | Some("md") | Some("rs") | Some("ts") | Some("js") | Some("py") | Some("toml") => "text/plain",
        _ => "application/octet-stream",
    };
    let base64 = BASE64.encode(data);
    Ok(format!("data:{};base64,{}", mime, base64))
}

#[tauri::command]
fn create_folder(path: String, name: String) -> Result<(), String> {
    let full_path = PathBuf::from(path).join(name);
    fs::create_dir_all(full_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn copy_file(src: String, dest: String) -> Result<(), String> {
    fs::copy(src, dest).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn move_file(src: String, dest: String) -> Result<(), String> {
    fs::rename(src, dest).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Terminal execution (simple command) ─────────────────────────────────────

#[tauri::command]
async fn execute_command(command: String) -> Result<CommandOutput, String> {
    let output = TokioCommand::new("sh")
    .arg("-c")
    .arg(&command)
    .output()
    .await
    .map_err(|e| e.to_string())?;

    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
       stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

// ── Live terminal (bash process) ───────────────────────────────────────────

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
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = window_clone.emit("terminal-output", serde_json::json!({
                "type": "stdout",
                "data": line
            }));
        }
    });

    let window_clone = window.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = window_clone.emit("terminal-output", serde_json::json!({
                "type": "stderr",
                "data": line
            }));
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

// ── Desktop path detection ─────────────────────────────────────────────────

#[tauri::command]
fn get_default_desktop_path() -> String {
    if let Ok(output) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let desktop = home.join("Desktop");
    if desktop.exists() {
        return desktop.to_string_lossy().to_string();
    }
    let pulpit = home.join("Pulpit");
    if pulpit.exists() {
        return pulpit.to_string_lossy().to_string();
    }
    "HOME/Desktop".to_string()
}

#[tauri::command]
fn create_text_file(path: String, name: String, content: String) -> Result<(), String> {
    let full_path = PathBuf::from(path).join(name);
    fs::write(full_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Clipboard history ─────────────────────────────────────────────────────

#[tauri::command]
fn get_clipboard_history() -> Vec<ClipboardItem> {
    let path = clipboard_history_path();
    fs::read_to_string(&path)
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default()
}

#[tauri::command]
fn add_to_clipboard_history(content: String) {
    let path = clipboard_history_path();
    let mut history: Vec<ClipboardItem> = get_clipboard_history();
    let new_item = ClipboardItem {
        id: chrono::Utc::now().timestamp_millis().to_string(),
        content,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };
    history.insert(0, new_item);
    history.truncate(50);
    let _ = fs::write(path, serde_json::to_string(&history).unwrap());
}

#[tauri::command]
fn clear_clipboard_history() {
    let path = clipboard_history_path();
    let _ = fs::write(path, "[]");
}

// ── Night Light ──────────────────────────────────────────────────────────

#[tauri::command]
fn set_night_light_enabled(enabled: bool) -> Result<(), String> {
    let session = session::detect_session();
    match session {
        session::SessionType::WaylandClient => {
            if enabled {
                Command::new("wlr-randr")
                .arg("--output")
                .arg("eDP-1")
                .arg("--gamma")
                .arg("1.0:0.8:0.6")
                .spawn()
                .map_err(|e| e.to_string())?;
            } else {
                Command::new("wlr-randr")
                .arg("--output")
                .arg("eDP-1")
                .arg("--gamma")
                .arg("1.0:1.0:1.0")
                .spawn()
                .map_err(|e| e.to_string())?;
            }
        }
        session::SessionType::X11Client => {
            let temp = if enabled { 4000 } else { 6500 };
            let factor = temp as f32 / 6500.0;
            let r = 1.0;
            let g = factor * 0.8;
            let b = factor * 0.6;
            Command::new("xrandr")
            .args(["--output", "eDP-1", "--gamma", &format!("{:.2}:{:.2}:{:.2}", r, g, b)])
            .spawn()
            .map_err(|e| e.to_string())?;
        }
        _ => return Err("No display server".to_string()),
    }
    Ok(())
}

#[tauri::command]
fn set_night_light_temperature(temperature: u32) -> Result<(), String> {
    let session = session::detect_session();
    let factor = temperature as f32 / 6500.0;
    let r = 1.0;
    let g = factor * 0.8;
    let b = factor * 0.6;
    match session {
        session::SessionType::WaylandClient => {
            Command::new("wlr-randr")
            .arg("--output")
            .arg("eDP-1")
            .arg("--gamma")
            .arg(format!("{:.2}:{:.2}:{:.2}", r, g, b))
            .spawn()
            .map_err(|e| e.to_string())?;
        }
        session::SessionType::X11Client => {
            Command::new("xrandr")
            .args(["--output", "eDP-1", "--gamma", &format!("{:.2}:{:.2}:{:.2}", r, g, b)])
            .spawn()
            .map_err(|e| e.to_string())?;
        }
        _ => return Err("No display server".to_string()),
    }
    Ok(())
}

// ── Notifications ────────────────────────────────────────────────────────

#[tauri::command]
fn get_notification_history() -> Vec<Notification> {
    let path = notifications_path();
    fs::read_to_string(&path)
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default()
}

#[tauri::command]
fn save_notification_history(notifications: Vec<Notification>) {
    let path = notifications_path();
    let _ = fs::write(path, serde_json::to_string(&notifications).unwrap());
}

// ── Custom themes ─────────────────────────────────────────────────────────

#[tauri::command]
fn get_custom_themes() -> Vec<cache::ThemeDefinition> {
    // Mock implementation – można rozwinąć
    vec![]
}

#[tauri::command]
fn save_custom_theme(theme: cache::ThemeDefinition) {
    // Save custom theme
    println!("Saving theme: {:?}", theme);
    // Tutaj rzeczywista implementacja zapisu do pliku
}

#[tauri::command]
fn delete_custom_theme(_theme_id: String) {
    // Delete custom theme
    println!("Deleting theme: {}", _theme_id);
    // Tutaj rzeczywista implementacja usuwania
}

// ── Panel ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn set_panel_enabled(enabled: bool) -> Result<(), String> {
    println!("Panel enabled: {}", enabled);
    // W przyszłości można uruchamiać/zatrzymywać proces panelu
    Ok(())
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    cache::ensure_dirs();

    // Wczytaj konfigurację i uruchom panel jeśli włączony
    let config = cache::load_user_config();
    let config_parsed: cache::UserConfig = serde_json::from_str(&config).unwrap_or_default();
    if config_parsed.panel_enabled {
        start_panel();
    }

    tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        // Session
        get_session_type,
        // Apps
        get_system_apps,
        get_recent_apps,
        record_app_launch,
        invalidate_app_cache,
        launch_process,
        // Files
        list_files,
        read_text_file,
        write_text_file,
        // System
        get_system_stats,
        get_processes,
        take_screenshot,
        get_wallpapers,
        get_wallpaper_preview,
        load_distro_info,
        system_power,
        // Audio
        get_audio_sinks,
        set_sink_volume,
        set_default_sink,
        toggle_sink_mute,
        set_volume,
        // Wi-Fi
        get_wifi_networks_real,
        connect_wifi_real,
        disconnect_wifi,
        toggle_wifi,
        // Bluetooth
        get_bluetooth_devices_real,
        bluetooth_connect,
        bluetooth_disconnect,
        bluetooth_pair,
        // Power profiles
        get_power_profiles,
        set_power_profile,
        // Brightness
        set_brightness,
        // Config & cache
        save_config,
        load_config,
        save_window_state,
        load_window_state,
        // Window tracking
        get_external_windows,
        focus_external_window,
        minimize_external_window,
        close_external_window,
        // File operations for Explorer
        read_file_as_data_url,
        create_folder,
        delete_file,
        copy_file,
        move_file,
        // Terminal execution
        execute_command,
        spawn_terminal,
        write_to_terminal,
        // Desktop path
        get_default_desktop_path,
        create_text_file,
        // Clipboard history
        get_clipboard_history,
        add_to_clipboard_history,
        clear_clipboard_history,
        // Night Light
        set_night_light_enabled,
        set_night_light_temperature,
        // Notifications
        get_notification_history,
        save_notification_history,
        // Custom themes
        get_custom_themes,
        save_custom_theme,
        delete_custom_theme,
        // Panel
        set_panel_enabled,
    ])
    .run(tauri::generate_context!())
    .expect("error while running Blue Environment");
}

fn start_panel() {
    // Spawn the blue-panel binary
    let panel_path = std::env::current_exe()
    .unwrap()
    .parent()
    .unwrap()
    .join("blue-panel");
    if panel_path.exists() {
        let _ = Command::new(panel_path).spawn();
        println!("Panel started");
    } else {
        eprintln!("Panel binary not found at {:?}", panel_path);
    }
}
