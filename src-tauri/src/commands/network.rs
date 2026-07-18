use crate::types::*;
use crate::{apps, cache, ai, packages, window_tracker};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as TokioCommand;
use serde::{Serialize, Deserialize};

/// Splits an `nmcli -t` (terse) output line into fields.
///
/// nmcli's terse mode escapes literal `:` and `\` characters inside field
/// values with a leading backslash (this matters a lot for BSSID, which is
/// a colon-separated MAC address). A naive `line.split(':')` treats those
/// escaped colons as field separators too, shifting every field after the
/// BSSID — which is exactly why SSIDs were showing up as mangled
/// fragments like `95\` with the signal always reading 0%.
pub fn split_nmcli_terse(line: &str) -> Vec<String> {
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
pub fn get_wifi_networks_real() -> Vec<WifiNetwork> {
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
pub fn connect_wifi_real(ssid: String, password: String) -> Result<String, String> {
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
pub fn disconnect_wifi() -> Result<(), String> {
    Command::new("nmcli").args(["dev", "disconnect", "wlan0"]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_wifi(enabled: bool) {
    let _ = Command::new("nmcli").args(["radio", "wifi", if enabled { "on" } else { "off" }]).spawn();
}

#[tauri::command]
pub fn get_bluetooth_devices_real() -> Vec<BluetoothDevice> {
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
pub fn bluetooth_connect(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl").args(["connect", &mac]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn bluetooth_disconnect(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl").args(["disconnect", &mac]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn bluetooth_pair(mac: String) -> Result<(), String> {
    Command::new("bluetoothctl").args(["pair", &mac]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Android-style "Mobile data" toggle — only relevant on laptops with a
/// WWAN/cellular modem (increasingly common on business laptops with a
/// SIM/eSIM slot). Detected via ModemManager (`mmcli -L`); if no modem is
/// present, `has_cellular_modem` returns false and the frontend simply
/// never shows the toggle — matching "jesli laptop tego nie ma, to nie ma
/// w Blue Environment".
#[tauri::command]
pub fn has_cellular_modem() -> bool {
    let output = Command::new("mmcli").arg("-L").output();
    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            o.status.success() && text.contains("/Modem/")
        }
        Err(_) => false, // ModemManager not installed - no WWAN hardware to speak of
    }
}

#[tauri::command]
pub fn get_cellular_status() -> Result<serde_json::Value, String> {
    let list = Command::new("mmcli").arg("-L").output().map_err(|e| e.to_string())?;
    let list_text = String::from_utf8_lossy(&list.stdout);
    let modem_path = list_text
        .lines()
        .find(|l| l.contains("/Modem/"))
        .and_then(|l| l.split_whitespace().next())
        .ok_or("No modem found")?;

    let info = Command::new("mmcli").arg("-m").arg(modem_path).output().map_err(|e| e.to_string())?;
    let info_text = String::from_utf8_lossy(&info.stdout);

    let connected = info_text.contains("state: connected") || info_text.contains("state: 'connected'");
    let signal = info_text
        .lines()
        .find(|l| l.to_lowercase().contains("signal quality"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|s| s.trim().split_whitespace().next())
        .and_then(|s| s.trim_matches(|c: char| !c.is_numeric()).parse::<u8>().ok())
        .unwrap_or(0);
    let carrier = info_text
        .lines()
        .find(|l| l.to_lowercase().contains("operator name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().trim_matches('\'').to_string())
        .unwrap_or_default();

    Ok(serde_json::json!({ "connected": connected, "signal": signal, "carrier": carrier, "modem_path": modem_path }))
}

#[tauri::command]
pub fn set_cellular_enabled(enabled: bool) -> Result<(), String> {
    let list = Command::new("mmcli").arg("-L").output().map_err(|e| e.to_string())?;
    let list_text = String::from_utf8_lossy(&list.stdout);
    let modem_path = list_text
        .lines()
        .find(|l| l.contains("/Modem/"))
        .and_then(|l| l.split_whitespace().next())
        .ok_or("No modem found")?;

    let flag = if enabled { "-e" } else { "-d" };
    Command::new("mmcli").arg("-m").arg(modem_path).arg(flag)
        .output().map_err(|e| e.to_string())?;
    Ok(())
}
