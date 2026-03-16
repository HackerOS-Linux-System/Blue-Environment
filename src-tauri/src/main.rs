#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod session;
mod cache;
mod apps;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use glob::glob;
use cache::CachedApp;

// ── Structs ────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: String,
    mime_type: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProcessEntry {
    pid: String,
    name: String,
    cpu: f32,
    memory: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct WifiNetwork {
    ssid: String,
    signal: u8,
    secure: bool,
    in_use: bool,
    bssid: String,
    frequency: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct BluetoothDevice {
    name: String,
    mac: String,
    device_type: String,
    connected: bool,
    paired: bool,
    trusted: bool,
    battery: Option<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AudioSink {
    id: u32,
    name: String,
    description: String,
    volume: f32,
    muted: bool,
    is_default: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AudioSource {
    id: u32,
    name: String,
    description: String,
    volume: f32,
    muted: bool,
    is_default: bool,
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
                // Pass through current WAYLAND_DISPLAY
                cmd.arg(format!("{} &", command));
            }
            session::SessionType::X11Client => {
                cmd.arg(format!("{} &", command));
            }
            session::SessionType::Tty => {
                // When in TTY, set our Wayland socket so apps connect to our compositor
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
                    format!("{:.1} MB", meta.len() as f64 / 1_048_576.0)
                };

                // Simple MIME detection by extension
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

                entries.push(FileEntry { name, path: entry.path().to_string_lossy().to_string(), is_dir, size, mime_type });
            }
        }
    }

    // Dirs first, then files, both sorted
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
    fs::write(path, content).map_err(|e| e.to_string())
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

    let battery = get_battery_info();

    SystemStats {
        cpu: sys.global_cpu_info().cpu_usage(),
        ram: (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0,
        battery: battery.0,
        is_charging: battery.1,
        volume,
        brightness: get_brightness(),
        wifi_ssid,
        kernel,
        session_type: session::session_info(),
    }
}

fn get_battery_info() -> (f32, bool) {
    // Try /sys/class/power_supply
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
    (100.0, true) // Desktop — assume plugged in
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
    -1 // Not available
}

// ── PipeWire/PulseAudio audio ──────────────────────────────────────────────

fn get_pipewire_volume() -> Option<i32> {
    // Try pactl (works with both PulseAudio and PipeWire-pulse)
    let out = Command::new("pactl")
    .args(["get-sink-volume", "@DEFAULT_SINK@"])
    .output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    // Parse "Volume: front-left: 65536 / 100% ..."
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

    // Rescan first (non-blocking)
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
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

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

            // Get detailed info
            let info = Command::new("bluetoothctl")
            .args(["info", &mac])
            .output()
            .map(|i| String::from_utf8_lossy(&i.stdout).to_string())
            .unwrap_or_default();

            let connected = info.contains("Connected: yes");
            let paired = info.contains("Paired: yes");
            let trusted = info.contains("Trusted: yes");

            // Device type from Icon field
            let device_type = info.lines()
            .find(|l| l.trim_start().starts_with("Icon:"))
            .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
            .unwrap_or("unknown".to_string());

            // Battery level (if supported)
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

// ── Brightness ─────────────────────────────────────────────────────────────

#[tauri::command]
fn set_brightness(level: i32) {
    // Try brightnessctl first, then xrandr
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

#[tauri::command]
fn get_wallpapers() -> Vec<String> {
    let mut wallpapers = vec![
        "https://images.unsplash.com/photo-1451187580459-43490279c0fa?q=80&w=2072".to_string(),
    ];
    let search_patterns = [
        "/usr/share/wallpapers/**/*.{jpg,png,jpeg}",
        "/usr/share/backgrounds/**/*.{jpg,png,jpeg}",
    ];
    for pat in &search_patterns {
        if let Ok(entries) = glob(pat) {
            for entry in entries.filter_map(Result::ok) {
                wallpapers.push(format!("file://{}", entry.to_string_lossy()));
            }
        }
    }
    wallpapers
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
    cache::save_user_config(&config);
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

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    // Ensure all cache/config dirs exist
    cache::ensure_dirs();

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
        load_distro_info,
        system_power,
        // Audio (PipeWire/PulseAudio)
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
        // Brightness
        set_brightness,
        // Config & cache
        save_config,
        load_config,
        save_window_state,
        load_window_state,
    ])
    .run(tauri::generate_context!())
    .expect("error while running Blue Environment");
}
