use serde::{Deserialize, Serialize};
use std::process::Command;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DisplayInfo {
    pub name:         String,
    pub width:        u32,
    pub height:       u32,
    pub refresh_rate: f64,
    pub scale:        f64,
    pub x:            i32,
    pub y:            i32,
    pub primary:      bool,
    pub connected:    bool,
    pub modes:        Vec<DisplayMode>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DisplayMode {
    pub width:        u32,
    pub height:       u32,
    pub refresh_rate: f64,
    pub current:      bool,
    pub preferred:    bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WifiNetwork {
    pub ssid:     String,
    pub strength: u8,
    pub security: String,
    pub connected: bool,
    pub saved:    bool,
    pub bssid:    Option<String>,
    pub freq:     Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BluetoothDevice {
    pub address:  String,
    pub name:     String,
    pub paired:   bool,
    pub connected: bool,
    pub device_type: String,
    pub trusted:  bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BatteryInfo {
    pub present:    bool,
    pub percentage: u8,
    pub charging:   bool,
    pub time_to_full: Option<u32>,
    pub time_to_empty: Option<u32>,
    pub health:     Option<u8>,
    pub voltage:    Option<f64>,
    pub technology: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PowerProfile {
    pub id:          String,
    pub name:        String,
    pub description: String,
    pub active:      bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserAccount {
    pub username:   String,
    pub realname:   String,
    pub uid:        u32,
    pub gid:        u32,
    pub shell:      String,
    pub home:       String,
    pub groups:     Vec<String>,
    pub is_admin:   bool,
    pub avatar_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemInfo {
    pub hostname:      String,
    pub os_name:       String,
    pub os_version:    String,
    pub kernel:        String,
    pub desktop:       String,
    pub compositor:    String,
    pub cpu:           String,
    pub cpu_cores:     u32,
    pub memory_total:  u64,
    pub memory_used:   u64,
    pub disk_total:    u64,
    pub disk_used:     u64,
    pub uptime:        u64,
    pub wayland_socket: Option<String>,
    pub blue_version:  String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NightLightConfig {
    pub enabled:     bool,
    pub temperature: u32,    // 1000-6500K
    pub schedule:    String, // "manual" | "sunset"
    pub start_hour:  f32,
    pub end_hour:    f32,
    pub active_now:  bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PanelConfig {
    pub enabled:   bool,
    pub position:  String,
    pub size:      u32,
    pub opacity:   f32,
    pub auto_hide: bool,
    pub show_clock: bool,
    pub show_tray:  bool,
    pub show_workspace: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResult {
    pub success: bool,
    pub message: Option<String>,
}

impl SettingsResult {
    pub fn ok() -> Self { Self { success: true, message: None } }
    pub fn err(msg: impl Into<String>) -> Self { Self { success: false, message: Some(msg.into()) } }
}

fn sh(cmd: &str) -> Result<String, String> {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| e.to_string())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn sh_ok(cmd: &str) -> bool {
    Command::new("sh").arg("-c").arg(cmd).output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// Display commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_displays() -> Vec<DisplayInfo> {
    // Try wlr-randr first (Wayland), fall back to xrandr (X11)
    let output = sh("wlr-randr 2>/dev/null || xrandr 2>/dev/null").unwrap_or_default();

    let mut displays: Vec<DisplayInfo> = Vec::new();

    // Simple parser — reads "OUTPUT connected/disconnected WxH+X+Y" lines
    // A production implementation would use the Wayland output management protocol
    // or parse xrandr more carefully. This gives useful data for the common case.
    let mut current: Option<DisplayInfo> = None;

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();

        // Detect output line: "eDP-1 connected 1920x1080+0+0 ..."
        if parts.len() >= 2 && (parts[1] == "connected" || parts[1] == "disconnected") {
            if let Some(d) = current.take() {
                displays.push(d);
            }
            let connected = parts[1] == "connected";
            let name = parts[0].to_string();

            // Parse geometry: WxH+X+Y
            let (w, h, x, y, rr) = if parts.len() > 2 {
                parse_xrandr_geometry(parts[2])
            } else {
                (0, 0, 0, 0, 60.0)
            };

            current = Some(DisplayInfo {
                name,
                width: w,
                height: h,
                refresh_rate: rr,
                scale: 1.0,
                x,
                y,
                primary: parts.contains(&"primary"),
                connected,
                modes: Vec::new(),
            });
        } else if current.is_some() && !line.starts_with(' ') {
            // Sub-line for a mode
            if let Some(ref mut d) = current {
                if let Some(mode) = parse_xrandr_mode(line) {
                    d.modes.push(mode);
                }
            }
        }
    }

    if let Some(d) = current {
        displays.push(d);
    }

    // Fallback if parsing yields nothing
    if displays.is_empty() {
        displays.push(DisplayInfo {
            name: "eDP-1".into(),
            width: 1920,
            height: 1080,
            refresh_rate: 60.0,
            scale: 1.0,
            x: 0,
            y: 0,
            primary: true,
            connected: true,
            modes: vec![
                DisplayMode { width: 1920, height: 1080, refresh_rate: 60.0, current: true, preferred: true },
                DisplayMode { width: 1280, height: 720, refresh_rate: 60.0, current: false, preferred: false },
            ],
        });
    }

    displays
}

fn parse_xrandr_geometry(s: &str) -> (u32, u32, i32, i32, f64) {
    // "1920x1080+0+0"
    let parts: Vec<&str> = s.splitn(2, '+').collect();
    let res: Vec<&str> = parts[0].split('x').collect();
    let w = res.first().and_then(|v| v.parse().ok()).unwrap_or(0);
    let h = res.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let coords: Vec<i32> = parts.iter().skip(1).filter_map(|v| v.parse().ok()).collect();
    let x = coords.first().copied().unwrap_or(0);
    let y = coords.get(1).copied().unwrap_or(0);
    (w, h, x, y, 60.0)
}

fn parse_xrandr_mode(line: &str) -> Option<DisplayMode> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() < 2 { return None; }
    let res: Vec<&str> = parts[0].split('x').collect();
    let w: u32 = res.first()?.parse().ok()?;
    let h: u32 = res.get(1)?.parse().ok()?;
    let rr: f64 = parts.get(1)?.trim_end_matches('*').trim_end_matches('+').parse().ok()?;
    let current  = parts.iter().any(|p| p.contains('*'));
    let preferred = parts.iter().any(|p| p.contains('+'));
    Some(DisplayMode { width: w, height: h, refresh_rate: rr, current, preferred })
}

#[tauri::command]
pub fn settings_set_brightness(value: u8) -> SettingsResult {
    // Try brightnessctl, fall back to /sys/class/backlight/*
    let pct = value.min(100);
    if sh_ok(&format!("brightnessctl set {}% 2>/dev/null", pct)) {
        return SettingsResult::ok();
    }
    let bl_dir = sh("ls /sys/class/backlight/ 2>/dev/null | head -1").unwrap_or_default();
    if !bl_dir.is_empty() {
        let max_raw = sh(&format!("cat /sys/class/backlight/{}/max_brightness", bl_dir)).unwrap_or("255".into());
        let max: u64 = max_raw.trim().parse().unwrap_or(255);
        let target = (max * pct as u64) / 100;
        if sh_ok(&format!("echo {} > /sys/class/backlight/{}/brightness", target, bl_dir)) {
            return SettingsResult::ok();
        }
    }
    SettingsResult::err("No backlight control found — install brightnessctl")
}

#[tauri::command]
pub fn settings_get_brightness() -> u8 {
    // brightnessctl
    if let Ok(out) = sh("brightnessctl -m 2>/dev/null | cut -d, -f4 | tr -d '%'") {
        if let Ok(v) = out.trim().parse::<u8>() { return v; }
    }
    // /sys fallback
    let bl_dir = sh("ls /sys/class/backlight/ 2>/dev/null | head -1").unwrap_or_default();
    if !bl_dir.is_empty() {
        let cur = sh(&format!("cat /sys/class/backlight/{}/brightness", bl_dir)).unwrap_or_default();
        let max = sh(&format!("cat /sys/class/backlight/{}/max_brightness", bl_dir)).unwrap_or("255".into());
        if let (Ok(c), Ok(m)) = (cur.trim().parse::<u64>(), max.trim().parse::<u64>()) {
            if m > 0 { return ((c * 100) / m) as u8; }
        }
    }
    100
}

#[tauri::command]
pub fn settings_set_display_scale(output: String, scale: f64) -> SettingsResult {
    // Wayland: wlr-randr
    if sh_ok(&format!("wlr-randr --output {} --scale {} 2>/dev/null", output, scale)) {
        return SettingsResult::ok();
    }
    // X11: xrandr scale (approximate)
    let inv = 1.0 / scale;
    if sh_ok(&format!("xrandr --output {} --scale {}x{} 2>/dev/null", output, inv, inv)) {
        return SettingsResult::ok();
    }
    SettingsResult::err("Failed to set display scale")
}

#[tauri::command]
pub fn settings_set_resolution(output: String, width: u32, height: u32, refresh: f64) -> SettingsResult {
    if sh_ok(&format!(
        "wlr-randr --output {} --mode {}x{}@{}Hz 2>/dev/null",
        output, width, height, refresh
    )) {
        return SettingsResult::ok();
    }
    if sh_ok(&format!(
        "xrandr --output {} --mode {}x{} --rate {} 2>/dev/null",
        output, width, height, refresh
    )) {
        return SettingsResult::ok();
    }
    SettingsResult::err("Failed to set resolution")
}

// ─────────────────────────────────────────────────────────────────────────────
// Wi-Fi commands (nmcli)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_wifi_networks() -> Vec<WifiNetwork> {
    let mut nets: Vec<WifiNetwork> = Vec::new();

    let active = sh("nmcli -t -f NAME connection show --active 2>/dev/null").unwrap_or_default();
    let active_ssids: Vec<&str> = active.lines().collect();

    let output = match sh(
        "nmcli -t -f SSID,SIGNAL,SECURITY,BSSID,FREQ,IN-USE device wifi list 2>/dev/null"
    ) {
        Ok(o) => o,
        Err(_) => return nets,
    };

    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(6, ':').collect();
        if parts.len() < 3 { continue; }
        let ssid = parts[0].to_string();
        if ssid.is_empty() { continue; }
        let strength: u8 = parts[1].parse().unwrap_or(0);
        let security = if parts[2].is_empty() { "Open".to_string() } else { parts[2].to_string() };
        let bssid = parts.get(3).map(|s| s.to_string());
        let freq: Option<u32> = parts.get(4).and_then(|s| s.parse().ok());
        let in_use = parts.get(5).map(|s| *s == "*").unwrap_or(false);

        nets.push(WifiNetwork {
            ssid: ssid.clone(),
            strength,
            security,
            connected: in_use || active_ssids.contains(&ssid.as_str()),
            saved: false, // filled below
            bssid,
            freq,
        });
    }

    // Mark saved networks
    let saved = sh("nmcli -t -f NAME connection show 2>/dev/null").unwrap_or_default();
    let saved_names: Vec<&str> = saved.lines().collect();
    for net in &mut nets {
        if saved_names.contains(&net.ssid.as_str()) {
            net.saved = true;
        }
    }

    nets.sort_by(|a, b| b.strength.cmp(&a.strength));
    nets.dedup_by(|a, b| a.ssid == b.ssid);
    nets
}

#[tauri::command]
pub fn settings_wifi_connect(ssid: String, password: Option<String>) -> SettingsResult {
    let cmd = if let Some(pw) = password {
        format!("nmcli device wifi connect {} password {} 2>&1", 
            shell_escape(&ssid), shell_escape(&pw))
    } else {
        format!("nmcli device wifi connect {} 2>&1", shell_escape(&ssid))
    };
    match sh(&cmd) {
        Ok(out) if out.contains("successfully activated") => SettingsResult::ok(),
        Ok(out) => SettingsResult::err(out),
        Err(e)  => SettingsResult::err(e),
    }
}

#[tauri::command]
pub fn settings_wifi_disconnect(ssid: String) -> SettingsResult {
    match sh(&format!("nmcli connection down {} 2>&1", shell_escape(&ssid))) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

#[tauri::command]
pub fn settings_wifi_scan() -> SettingsResult {
    sh_ok("nmcli device wifi rescan 2>/dev/null");
    SettingsResult::ok()
}

#[tauri::command]
pub fn settings_wifi_toggle(enabled: bool) -> SettingsResult {
    let state = if enabled { "on" } else { "off" };
    match sh(&format!("nmcli radio wifi {} 2>&1", state)) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bluetooth commands (bluetoothctl)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_bluetooth_devices() -> Vec<BluetoothDevice> {
    let mut devices: Vec<BluetoothDevice> = Vec::new();

    let paired = sh("bluetoothctl paired-devices 2>/dev/null").unwrap_or_default();
    let conn   = sh("bluetoothctl info 2>/dev/null").unwrap_or_default();

    for line in paired.lines() {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 3 { continue; }
        let addr = parts[1].to_string();
        let name = parts[2].to_string();
        let connected = conn.contains(&addr);

        let info = sh(&format!("bluetoothctl info {} 2>/dev/null", addr)).unwrap_or_default();
        let trusted = info.contains("Trusted: yes");
        let device_type = detect_bt_type(&info);

        devices.push(BluetoothDevice {
            address: addr,
            name,
            paired: true,
            connected,
            device_type,
            trusted,
        });
    }

    // Also list available (not yet paired) from scan cache
    let avail = sh("bluetoothctl devices 2>/dev/null").unwrap_or_default();
    for line in avail.lines() {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 3 { continue; }
        let addr = parts[1].to_string();
        if devices.iter().any(|d| d.address == addr) { continue; }
        devices.push(BluetoothDevice {
            address: addr,
            name: parts[2].to_string(),
            paired: false,
            connected: false,
            device_type: "Unknown".into(),
            trusted: false,
        });
    }

    devices
}

fn detect_bt_type(info: &str) -> String {
    if info.contains("Icon: audio-headset")   { return "Headset".into(); }
    if info.contains("Icon: audio-headphones") { return "Headphones".into(); }
    if info.contains("Icon: input-keyboard")  { return "Keyboard".into(); }
    if info.contains("Icon: input-mouse")     { return "Mouse".into(); }
    if info.contains("Icon: phone")           { return "Phone".into(); }
    "Unknown".into()
}

#[tauri::command]
pub fn settings_bluetooth_pair(address: String) -> SettingsResult {
    let cmd = format!(
        "bluetoothctl pair {} && bluetoothctl trust {} && bluetoothctl connect {} 2>&1",
        address, address, address
    );
    match sh(&cmd) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

#[tauri::command]
pub fn settings_bluetooth_connect(address: String) -> SettingsResult {
    match sh(&format!("bluetoothctl connect {} 2>&1", address)) {
        Ok(out) if out.contains("Connection successful") => SettingsResult::ok(),
        Ok(out) => SettingsResult::err(out),
        Err(e)  => SettingsResult::err(e),
    }
}

#[tauri::command]
pub fn settings_bluetooth_disconnect(address: String) -> SettingsResult {
    match sh(&format!("bluetoothctl disconnect {} 2>&1", address)) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

#[tauri::command]
pub fn settings_bluetooth_remove(address: String) -> SettingsResult {
    match sh(&format!("bluetoothctl remove {} 2>&1", address)) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

#[tauri::command]
pub fn settings_bluetooth_scan(enable: bool) -> SettingsResult {
    let action = if enable { "scan on" } else { "scan off" };
    sh_ok(&format!("bluetoothctl {} &", action));
    SettingsResult::ok()
}

#[tauri::command]
pub fn settings_bluetooth_toggle(enabled: bool) -> SettingsResult {
    let state = if enabled { "power on" } else { "power off" };
    match sh(&format!("bluetoothctl {} 2>&1", state)) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Power / battery commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_battery() -> BatteryInfo {
    // Try /sys/class/power_supply/BAT*
    let bat = sh("ls /sys/class/power_supply/ 2>/dev/null | grep -i 'bat\\|acpi' | head -1")
        .unwrap_or_default();
    let bat = bat.trim();

    if bat.is_empty() {
        return BatteryInfo {
            present: false, percentage: 0, charging: false,
            time_to_full: None, time_to_empty: None,
            health: None, voltage: None, technology: None,
        };
    }

    let base = format!("/sys/class/power_supply/{}", bat);
    let read = |f: &str| sh(&format!("cat {}/{} 2>/dev/null", base, f)).unwrap_or_default();

    let cap: u8 = read("capacity").trim().parse().unwrap_or(0);
    let status = read("status");
    let charging = status.trim() == "Charging" || status.trim() == "Full";
    let health_str = read("health");
    let health = if health_str.trim() == "Good" { Some(100u8) } else { None };
    let voltage_uv: Option<u64> = read("voltage_now").trim().parse().ok();
    let voltage = voltage_uv.map(|v| v as f64 / 1_000_000.0);
    let tech = read("technology");
    let technology = if tech.trim().is_empty() { None } else { Some(tech.trim().to_string()) };

    // Estimate time remaining
    let charge_now: Option<i64> = read("charge_now").trim().parse().ok();
    let charge_full: Option<i64> = read("charge_full").trim().parse().ok();
    let current_now: Option<i64> = read("current_now").trim().parse().ok();
    let (time_to_full, time_to_empty) = estimate_battery_time(charging, charge_now, charge_full, current_now);

    BatteryInfo {
        present: true,
        percentage: cap,
        charging,
        time_to_full,
        time_to_empty,
        health,
        voltage,
        technology,
    }
}

fn estimate_battery_time(
    charging: bool,
    charge_now: Option<i64>,
    charge_full: Option<i64>,
    current: Option<i64>,
) -> (Option<u32>, Option<u32>) {
    let (cn, cf, curr) = match (charge_now, charge_full, current) {
        (Some(a), Some(b), Some(c)) if c > 0 => (a, b, c),
        _ => return (None, None),
    };
    if charging {
        let secs = ((cf - cn).max(0) * 3600) / curr;
        (Some(secs as u32), None)
    } else {
        let secs = (cn.max(0) * 3600) / curr;
        (None, Some(secs as u32))
    }
}

#[tauri::command]
pub fn settings_get_power_profiles() -> Vec<PowerProfile> {
    let active = sh("powerprofilesctl get 2>/dev/null")
        .unwrap_or_else(|_| "balanced".into());
    let active = active.trim();

    vec![
        PowerProfile {
            id: "power-saver".into(), name: "Power Saver".into(),
            description: "Reduces performance to extend battery life".into(),
            active: active == "power-saver",
        },
        PowerProfile {
            id: "balanced".into(), name: "Balanced".into(),
            description: "Balances performance and power consumption".into(),
            active: active == "balanced",
        },
        PowerProfile {
            id: "performance".into(), name: "Performance".into(),
            description: "Maximizes performance at the cost of battery life".into(),
            active: active == "performance",
        },
    ]
}

#[tauri::command]
pub fn settings_set_power_profile(profile: String) -> SettingsResult {
    if sh_ok(&format!("powerprofilesctl set {} 2>/dev/null", profile)) {
        return SettingsResult::ok();
    }
    // Fallback: cpupower
    let governor = match profile.as_str() {
        "power-saver"  => "powersave",
        "performance"  => "performance",
        _              => "schedutil",
    };
    if sh_ok(&format!("cpupower frequency-set -g {} 2>/dev/null", governor)) {
        return SettingsResult::ok();
    }
    SettingsResult::err("Cannot set power profile — install power-profiles-daemon or cpupower")
}

// ─────────────────────────────────────────────────────────────────────────────
// User accounts
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_current_user() -> UserAccount {
    let username = sh("whoami").unwrap_or_else(|_| "unknown".into());
    let username = username.trim().to_string();
    get_user_info(&username)
}

#[tauri::command]
pub fn settings_get_users() -> Vec<UserAccount> {
    let output = sh("getent passwd 2>/dev/null").unwrap_or_default();
    output.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 7 { return None; }
            let uid: u32 = parts[2].parse().ok()?;
            if uid < 1000 || uid > 60000 { return None; } // only human users
            Some(UserAccount {
                username:   parts[0].to_string(),
                realname:   parts[4].split(',').next().unwrap_or(parts[0]).to_string(),
                uid,
                gid:        parts[3].parse().unwrap_or(0),
                shell:      parts[6].to_string(),
                home:       parts[5].to_string(),
                groups:     get_user_groups(parts[0]),
                is_admin:   is_admin_user(parts[0]),
                avatar_path: find_avatar(parts[0], parts[5]),
            })
        })
        .collect()
}

fn get_user_info(username: &str) -> UserAccount {
    let line = sh(&format!("getent passwd {} 2>/dev/null", username)).unwrap_or_default();
    let parts: Vec<&str> = line.trim().split(':').collect();
    let home = parts.get(5).unwrap_or(&"/home/unknown").to_string();
    UserAccount {
        username: username.to_string(),
        realname: parts.get(4).and_then(|s| s.split(',').next()).unwrap_or(username).to_string(),
        uid: parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(1000),
        gid: parts.get(3).and_then(|v| v.parse().ok()).unwrap_or(1000),
        shell: parts.get(6).unwrap_or(&"/bin/bash").to_string(),
        home: home.clone(),
        groups: get_user_groups(username),
        is_admin: is_admin_user(username),
        avatar_path: find_avatar(username, &home),
    }
}

fn get_user_groups(username: &str) -> Vec<String> {
    sh(&format!("groups {} 2>/dev/null", username))
        .unwrap_or_default()
        .split_whitespace()
        .filter(|s| *s != username && *s != ":")
        .map(String::from)
        .collect()
}

fn is_admin_user(username: &str) -> bool {
    let groups = sh(&format!("groups {} 2>/dev/null", username)).unwrap_or_default();
    groups.contains("sudo") || groups.contains("wheel") || groups.contains("admin")
}

fn find_avatar(username: &str, home: &str) -> Option<String> {
    let candidates = [
        format!("{home}/.face"),
        format!("{home}/.face.icon"),
        format!("/var/lib/AccountsService/icons/{username}"),
        format!("/usr/share/pixmaps/faces/{username}.png"),
    ];
    candidates.into_iter().find(|p| std::path::Path::new(p).exists())
}

#[tauri::command]
pub fn settings_change_password(current: String, new_password: String) -> SettingsResult {
    // Use chpasswd via stdin - safer than passing on command line
    // NOTE: current-password verification happens via `passwd`'s own
    // interactive prompts below (current/new/confirm) — a wrong current
    // password makes `passwd` itself exit non-zero, which is already
    // handled in the match below. (A separate `su -c true` "pre-check"
    // used to sit here, but its result was never actually used, and
    // `su` doesn't verify a password fed through a plain piped stdin
    // like that anyway — it was dead code that looked like a security
    // check without being one.)
    // Best-effort via passwd
    let input = format!("{}\n{}\n{}\n", current, new_password, new_password);
    let result = Command::new("passwd")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    match result {
        Ok(mut child) => {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(input.as_bytes());
            }
            match child.wait() {
                Ok(s) if s.success() => SettingsResult::ok(),
                Ok(_)  => SettingsResult::err("Password change failed — check current password"),
                Err(e) => SettingsResult::err(format!("Failed to run passwd: {}", e)),
            }
        }
        Err(e) => SettingsResult::err(format!("Cannot run passwd: {}", e)),
    }
}

#[tauri::command]
pub fn settings_set_avatar(path: String) -> SettingsResult {
    let username = sh("whoami").unwrap_or_default();
    let username = username.trim();
    let home = sh(&format!("eval echo ~{}", username)).unwrap_or(format!("/home/{}", username));
    let dest = format!("{home}/.face");
    match sh(&format!("cp {} {} 2>&1", shell_escape(&path), shell_escape(&dest))) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// System info
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_system_info() -> SystemInfo {
    let hostname    = sh("hostname 2>/dev/null").unwrap_or_else(|_| "localhost".into());
    let os_name     = sh("lsb_release -si 2>/dev/null || cat /etc/os-release | grep ^NAME= | cut -d= -f2 | tr -d '\"'").unwrap_or_else(|_| "Linux".into());
    let os_version  = sh("lsb_release -sd 2>/dev/null || cat /etc/os-release | grep ^PRETTY_NAME= | cut -d= -f2 | tr -d '\"'").unwrap_or_else(|_| "Unknown".into());
    let kernel      = sh("uname -r 2>/dev/null").unwrap_or_else(|_| "Unknown".into());
    let cpu         = sh("grep 'model name' /proc/cpuinfo 2>/dev/null | head -1 | cut -d: -f2").unwrap_or_else(|_| "Unknown CPU".into());
    let cpu_cores: u32 = sh("nproc 2>/dev/null").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(1);

    let meminfo     = sh("cat /proc/meminfo 2>/dev/null").unwrap_or_default();
    let mem_total   = parse_meminfo_kb(&meminfo, "MemTotal");
    let mem_avail   = parse_meminfo_kb(&meminfo, "MemAvailable");
    let mem_used    = mem_total.saturating_sub(mem_avail);

    let disk_out    = sh("df -B1 / 2>/dev/null | tail -1").unwrap_or_default();
    let disk_parts: Vec<&str> = disk_out.split_whitespace().collect();
    let disk_total: u64 = disk_parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let disk_used:  u64 = disk_parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);

    let uptime_s = sh("cat /proc/uptime 2>/dev/null")
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;

    let wayland_socket = std::env::var("WAYLAND_DISPLAY").ok();
    let blue_version   = env!("CARGO_PKG_VERSION").to_string();

    SystemInfo {
        hostname: hostname.trim().to_string(),
        os_name: os_name.trim().to_string(),
        os_version: os_version.trim().to_string(),
        kernel: kernel.trim().to_string(),
        desktop: "Blue Environment".to_string(),
        compositor: "Blue Compositor (Smithay)".to_string(),
        cpu: cpu.trim().to_string(),
        cpu_cores,
        memory_total: mem_total * 1024,
        memory_used: mem_used * 1024,
        disk_total,
        disk_used,
        uptime: uptime_s,
        wayland_socket,
        blue_version,
    }
}

fn parse_meminfo_kb(meminfo: &str, key: &str) -> u64 {
    meminfo.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Night Light (wlsunset / redshift)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_night_light() -> NightLightConfig {
    // Check if wlsunset or redshift is running
    let wlsunset_running = sh_ok("pgrep -x wlsunset");
    let redshift_running = sh_ok("pgrep -x redshift");
    let enabled = wlsunset_running || redshift_running;

    // Read saved config
    let cfg_path = sh("echo ~/.config/blue-environment/night-light.json 2>/dev/null").unwrap_or_default();
    let cfg_data = sh(&format!("cat {} 2>/dev/null", cfg_path.trim())).unwrap_or_default();
    let saved: serde_json::Value = serde_json::from_str(&cfg_data).unwrap_or_default();

    NightLightConfig {
        enabled,
        temperature: saved["temperature"].as_u64().unwrap_or(3500) as u32,
        schedule: saved["schedule"].as_str().unwrap_or("manual").to_string(),
        start_hour: saved["start_hour"].as_f64().unwrap_or(20.0) as f32,
        end_hour: saved["end_hour"].as_f64().unwrap_or(7.0) as f32,
        active_now: enabled,
    }
}

#[tauri::command]
pub fn settings_set_night_light(config: NightLightConfig) -> SettingsResult {
    // Save config
    let cfg_dir = sh("echo ~/.config/blue-environment").unwrap_or_default();
    sh_ok(&format!("mkdir -p {}", cfg_dir.trim()));
    let json = serde_json::to_string(&config).unwrap_or_default();
    let cfg_path = format!("{}/night-light.json", cfg_dir.trim());
    sh_ok(&format!("echo '{}' > {}", json.replace('\'', "'\\''"), cfg_path));

    if !config.enabled {
        sh_ok("pkill -x wlsunset 2>/dev/null; pkill -x redshift 2>/dev/null");
        return SettingsResult::ok();
    }

    // Kill existing
    sh_ok("pkill -x wlsunset 2>/dev/null; pkill -x redshift 2>/dev/null");

    // Start wlsunset
    if sh_ok(&format!(
        "wlsunset -t {} -T 6500 -l 0 -L 0 & 2>/dev/null",
        config.temperature
    )) {
        return SettingsResult::ok();
    }

    // Fallback: redshift
    if sh_ok(&format!(
        "redshift -O {} &",
        config.temperature
    )) {
        return SettingsResult::ok();
    }

    SettingsResult::err("Neither wlsunset nor redshift found — install one of them")
}

// ─────────────────────────────────────────────────────────────────────────────
// Panel config (saved to config file, read by shell)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_panel_config() -> PanelConfig {
    let cfg_path = sh("echo ~/.config/blue-environment/panel.json 2>/dev/null").unwrap_or_default();
    let data = sh(&format!("cat {} 2>/dev/null", cfg_path.trim())).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();

    PanelConfig {
        enabled:    v["enabled"].as_bool().unwrap_or(true),
        position:   v["position"].as_str().unwrap_or("bottom").to_string(),
        size:       v["size"].as_u64().unwrap_or(40) as u32,
        opacity:    v["opacity"].as_f64().unwrap_or(0.9) as f32,
        auto_hide:  v["auto_hide"].as_bool().unwrap_or(false),
        show_clock: v["show_clock"].as_bool().unwrap_or(true),
        show_tray:  v["show_tray"].as_bool().unwrap_or(true),
        show_workspace: v["show_workspace"].as_bool().unwrap_or(true),
    }
}

#[tauri::command]
pub fn settings_save_panel_config(config: PanelConfig) -> SettingsResult {
    let cfg_dir = sh("echo ~/.config/blue-environment").unwrap_or_default();
    sh_ok(&format!("mkdir -p {}", cfg_dir.trim()));
    let json = serde_json::to_string_pretty(&config).unwrap_or_default();
    let cfg_path = format!("{}/panel.json", cfg_dir.trim());
    match sh(&format!("printf '%s' {} > {}", shell_escape(&json), cfg_path)) {
        Ok(_) => SettingsResult::ok(),
        Err(e) => SettingsResult::err(e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Compositor integration — live settings pushed to compositor via IPC
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct CompositorCommand {
    #[serde(rename = "type")]
    pub cmd_type: String,
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

#[tauri::command]
pub fn settings_send_to_compositor(command: CompositorCommand) -> SettingsResult {
    let socket_path = {
        let runtime = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        format!("{}/blue-compositor.sock", runtime)
    };

    let json = match serde_json::to_string(&command) {
        Ok(j) => j,
        Err(e) => return SettingsResult::err(format!("Serialization error: {}", e)),
    };

    use std::io::Write;
    use std::os::unix::net::UnixStream;

    match UnixStream::connect(&socket_path) {
        Ok(mut stream) => {
            let msg = format!("{}\n", json);
            match stream.write_all(msg.as_bytes()) {
                Ok(_) => SettingsResult::ok(),
                Err(e) => SettingsResult::err(format!("IPC write error: {}", e)),
            }
        }
        Err(_) => {
            // Compositor may not be running (nested dev session) — not fatal
            SettingsResult::ok()
        }
    }
}

#[tauri::command]
pub fn settings_set_workspace_count(count: usize) -> SettingsResult {
    settings_send_to_compositor(CompositorCommand {
        cmd_type: "set_workspace_count".into(),
        payload: serde_json::json!({ "count": count }),
    })
}

#[tauri::command]
pub fn settings_set_dpms_timeout(seconds: u64) -> SettingsResult {
    settings_send_to_compositor(CompositorCommand {
        cmd_type: "set_dpms_timeout".into(),
        payload: serde_json::json!({ "seconds": seconds }),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Wallpaper
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn settings_get_wallpapers(directory: Option<String>) -> Vec<String> {
    let dir = directory.unwrap_or_else(|| {
        sh("echo ~/Pictures 2>/dev/null").unwrap_or_else(|_| "~/Pictures".into())
    });
    let exts = "jpg jpeg png webp bmp svg";
    let mut paths: Vec<String> = Vec::new();
    for ext in exts.split_whitespace() {
        let found = sh(&format!("find {} -maxdepth 3 -iname '*.{}' 2>/dev/null | head -50", dir, ext))
            .unwrap_or_default();
        paths.extend(found.lines().map(String::from));
    }
    // Add system wallpapers
    let system_dirs = [
        "/usr/share/backgrounds",
        "/usr/share/wallpapers",
        "/usr/share/pixmaps",
    ];
    for sdir in &system_dirs {
        let found = sh(&format!(
            "find {} -maxdepth 3 \\( -iname '*.jpg' -o -iname '*.png' -o -iname '*.webp' \\) 2>/dev/null | head -30",
            sdir
        )).unwrap_or_default();
        paths.extend(found.lines().map(String::from));
    }
    paths.sort();
    paths.dedup();
    paths
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
