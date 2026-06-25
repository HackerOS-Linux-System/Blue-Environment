#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod session;
mod cache;
mod apps;
mod window_tracker;
mod ai;
mod packages;
mod icon_resolver;
#[path = "CameraApp/mod.rs"]
mod camera_app;
#[path = "BlueWebApp/mod.rs"]
mod blue_web_app;
#[path = "BlueCodeApp/mod.rs"]
mod blue_code_app;
#[path = "TerminalApp/mod.rs"]
mod terminal_app;
use terminal_app::{spawn_terminal, write_to_terminal, pty_create, pty_write, pty_resize, pty_close};
#[path = "ExplolerApp/mod.rs"]
mod exploler_app;
#[path = "BlueScreenshot/mod.rs"]
mod blue_screenshot;
#[path = "SystemMonitorApp/mod.rs"]
mod system_monitor_app;
#[path = "BlueArchiveApp/mod.rs"]
mod blue_archive_app;
#[path = "MailApp/mod.rs"]
mod mail_app;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use glob::glob;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cache::CachedApp;
use camera_app::{camera_list_devices, camera_check_available, camera_capture_frame, camera_capture_photo, camera_record_video};
use blue_web_app::{web_open_native, web_fetch_site_info};
use blue_code_app::{start_language_server, stop_language_server};
use tokio::process::{Command as TokioCommand};
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
    net_rx_mb: f32,
    net_tx_mb: f32,
    disk_read_mb: f32,
    disk_write_mb: f32,
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
// resolve_path is the canonical implementation in ExplolerApp/mod.rs.
// This local alias lets any remaining code in main.rs that calls
// resolve_path() still compile without modification.
fn resolve_path(path: &str) -> PathBuf {
    exploler_app::resolve_path(path)
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
    let (net_rx_mb, net_tx_mb, disk_read_mb, disk_write_mb) = get_network_disk_rates();

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
        net_rx_mb,
        net_tx_mb,
        disk_read_mb,
        disk_write_mb,
    }
}

/// Measures real network throughput (via the `sysinfo` crate's Networks
/// API) and disk I/O throughput (via /proc/diskstats) over a short
/// sampling window, returned in MB/s.
///
/// This replaces what used to be a `Math.random()` placeholder on the
/// frontend side (System Monitor's "Network" and "Disk I/O" cards showed
/// fabricated random numbers whenever the backend didn't supply
/// netRx/diskRead, which it never did — those fields didn't even exist
/// on this struct before). Both measurements need two samples to compute
/// a rate, hence the short blocking sleep here; at a 2s frontend polling
/// interval this adds negligible overhead.
fn get_network_disk_rates() -> (f32, f32, f32, f32) {
    use sysinfo::Networks;

    let mut networks = Networks::new_with_refreshed_list();
    let disk_before = read_proc_diskstats();

    std::thread::sleep(std::time::Duration::from_millis(200));

    networks.refresh();
    let disk_after = read_proc_diskstats();

    let mut rx_bytes: u64 = 0;
    let mut tx_bytes: u64 = 0;
    for (_name, data) in &networks {
        rx_bytes += data.received();
        tx_bytes += data.transmitted();
    }

    let (read_sectors, write_sectors) = match (disk_before, disk_after) {
        (Some((rb, wb)), Some((ra, wa))) => (ra.saturating_sub(rb), wa.saturating_sub(wb)),
        _ => (0, 0),
    };

    let elapsed_secs = 0.2_f32;
    let net_rx_mb    = (rx_bytes as f32 / 1_048_576.0) / elapsed_secs;
    let net_tx_mb    = (tx_bytes as f32 / 1_048_576.0) / elapsed_secs;
    // Sectors are always 512 bytes per the kernel's block layer ABI,
    // regardless of the device's actual physical sector size.
    let disk_read_mb  = (read_sectors as f32 * 512.0 / 1_048_576.0) / elapsed_secs;
    let disk_write_mb = (write_sectors as f32 * 512.0 / 1_048_576.0) / elapsed_secs;

    (net_rx_mb, net_tx_mb, disk_read_mb, disk_write_mb)
}

/// Reads cumulative (read_sectors, written_sectors) totals across all
/// whole block devices from /proc/diskstats. Only devices that have a
/// corresponding /sys/block entry are counted — that's the standard way
/// (used by tools like iostat) to count each physical disk's I/O exactly
/// once instead of double-counting it once for the disk and again for
/// each of its partitions, which all report overlapping cumulative
/// totals in /proc/diskstats.
fn read_proc_diskstats() -> Option<(u64, u64)> {
    let content = fs::read_to_string("/proc/diskstats").ok()?;
    let mut total_read = 0u64;
    let mut total_write = 0u64;

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 { continue; }
        let dev_name = fields[2];
        if dev_name.starts_with("loop") || dev_name.starts_with("ram") { continue; }
        if !PathBuf::from("/sys/block").join(dev_name).exists() { continue; }

        let sectors_read: u64    = fields.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
        let sectors_written: u64 = fields.get(9).and_then(|s| s.parse().ok()).unwrap_or(0);
        total_read  += sectors_read;
        total_write += sectors_written;
    }
    Some((total_read, total_write))
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

/// Splits an `nmcli -t` (terse) output line into fields.
///
/// nmcli's terse mode escapes literal `:` and `\` characters inside field
/// values with a leading backslash (this matters a lot for BSSID, which is
/// a colon-separated MAC address). A naive `line.split(':')` treats those
/// escaped colons as field separators too, shifting every field after the
/// BSSID — which is exactly why SSIDs were showing up as mangled
/// fragments like `95\` with the signal always reading 0%.
fn split_nmcli_terse(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(escaped) = chars.next() {
                current.push(escaped);
            }
        } else if c == ':' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    fields.push(current);
    fields
}

#[tauri::command]
fn get_wifi_networks_real() -> Vec<WifiNetwork> {
    let mut networks = Vec::new();
    let _ = Command::new("nmcli").args(["dev", "wifi", "rescan"]).output();
    if let Ok(o) = Command::new("nmcli").args(["-t", "-f", "IN-USE,BSSID,SSID,MODE,CHAN,FREQ,RATE,SIGNAL,BARS,SECURITY", "dev", "wifi", "list"]).output() {
        let text = String::from_utf8_lossy(&o.stdout);
        let mut seen = std::collections::HashSet::new();
        for line in text.lines() {
            let parts = split_nmcli_terse(line);
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
// take_screenshot delegated to blue_screenshot::take_screenshot via generate_handler!
// get_processes delegated to system_monitor_app::get_processes via generate_handler!

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

// ── File operations (delegated to ExplolerApp/mod.rs) ─────────────────────
// All file commands (#[tauri::command]) live exclusively in ExplolerApp/mod.rs
// and are registered via exploler_app:: prefix in generate_handler!.
// Having them here too caused E0255/E0659 duplicate macro errors.

// ── Terminal ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn execute_command(command: String) -> Result<CommandOutput, String> {
    let output = TokioCommand::new("sh").arg("-c").arg(&command).output().await.map_err(|e| e.to_string())?;
    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
       stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
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

// ── Package managers ───────────────────────────────────────────────────────
//
// These used to be hardcoded stubs (`vec![]` / `Ok(false)`) in this file
// while the *real* implementation in packages.rs was only ever wired up
// from lib.rs (the mobile entry point) — meaning Blue Software on desktop,
// which runs this binary, always showed an empty package list and every
// install/remove/update silently "succeeded" at doing nothing. Wired up
// for real now, matching what lib.rs already did correctly.
//
// Also: this targets Fedora-based LegendaryOS, which has no `apt` at
// all — the previous apt-based implementation could never have worked
// here. Package management now goes through `dnf`/`rpm` instead, and the
// `snap` source has been dropped entirely (Fedora doesn't ship snapd by
// default and it isn't part of the supported package stack).

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

// ── Panel ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn set_panel_enabled(enabled: bool) -> Result<(), String> {
    println!("Panel enabled: {}", enabled);
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
    .manage(terminal_app::new_pty_sessions())
    .invoke_handler(tauri::generate_handler![
        get_session_type,
        get_system_apps, get_recent_apps, record_app_launch, invalidate_app_cache, launch_process,
        get_external_windows, focus_external_window, minimize_external_window, close_external_window, embed_external_window,
        exploler_app::list_files, exploler_app::read_text_file, exploler_app::write_text_file, exploler_app::git_status,
        get_system_stats, system_monitor_app::get_processes,
        read_config_file, write_config_file, read_cache_file, write_cache_file,
        blue_screenshot::take_screenshot, get_wallpapers, get_wallpaper_preview, load_distro_info, system_power,
        get_audio_sinks, set_sink_volume, set_default_sink, toggle_sink_mute, set_volume,
        get_wifi_networks_real, connect_wifi_real, disconnect_wifi, toggle_wifi,
        get_bluetooth_devices_real, bluetooth_connect, bluetooth_disconnect, bluetooth_pair,
        get_power_profiles, set_power_profile,
        set_brightness,
        save_config, load_config, save_window_state, load_window_state,
        exploler_app::read_file_as_data_url, exploler_app::create_folder, exploler_app::delete_file, exploler_app::copy_file, exploler_app::move_file,
        execute_command, pty_create, pty_write, pty_resize, pty_close, spawn_terminal, write_to_terminal,
        exploler_app::get_default_desktop_path, exploler_app::create_text_file, exploler_app::get_username, exploler_app::get_hostname, exploler_app::get_home_path,
        get_clipboard_history, add_to_clipboard_history, clear_clipboard_history,
        set_night_light_enabled, set_night_light_temperature,
        get_notification_history, save_notification_history,
        get_custom_themes, save_custom_theme, delete_custom_theme,
        ai_call, get_ai_config, save_ai_config,
        get_dnf_packages, get_flatpak_packages, get_appimage_packages,
        install_dnf_package, remove_dnf_package, update_dnf_package,
        install_flatpak_package, remove_flatpak_package, update_flatpak_package,
        install_appimage, remove_appimage, update_appimage,
        set_panel_enabled,
        camera_list_devices, camera_check_available, camera_capture_frame, camera_capture_photo, camera_record_video,
        web_open_native, web_fetch_site_info,
        start_language_server, stop_language_server,
        blue_archive_app::archive_list, blue_archive_app::archive_extract, blue_archive_app::archive_create,
        mail_app::mail_get_accounts, mail_app::mail_save_account, mail_app::mail_delete_account,
        mail_app::mail_fetch_inbox, mail_app::mail_send, mail_app::mail_mark_read, mail_app::mail_move_message,
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
