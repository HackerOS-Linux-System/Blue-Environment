use crate::types::*;
use std::process::Command;

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
    // Previously hardcoded "wlan0" — wrong on most modern distros, which
    // use predictable network interface names (wlp3s0, wlo1, ...), so
    // this silently did nothing on the majority of real systems. Ask
    // nmcli for the actually-connected Wi-Fi device instead of assuming
    // a name.
    let list = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "dev"])
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&list.stdout);
    let device = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 3 && parts[1] == "wifi" && parts[2] == "connected" {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| "no connected Wi-Fi device found".to_string())?;

    Command::new("nmcli")
        .args(["dev", "disconnect", &device])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_wifi(enabled: bool) {
    let _ = Command::new("nmcli").args(["radio", "wifi", if enabled { "on" } else { "off" }]).spawn();
}

/// Reads the *actual* Wi-Fi radio power state (`nmcli radio wifi`,
/// which prints exactly `enabled` or `disabled`) — as opposed to
/// "currently connected to a network", which is a completely different
/// thing the frontend was previously (wrongly) treating as the same
/// concept. See ControlCenter.svelte's fix for the full bug this closes:
/// the on/off toggle was derived from connection status, not radio
/// power, so it could show "off" while the radio was genuinely on (and
/// therefore still connectable/scannable) any time the device simply
/// wasn't associated with a network at that moment.
#[tauri::command]
pub fn get_wifi_radio_enabled() -> bool {
    Command::new("nmcli")
        .args(["radio", "wifi"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(true) // fail open to "enabled" — matches nmcli's own default assumption absent evidence otherwise, and avoids permanently locking the UI into a false "off" state on a transient command failure
}

/// Lists every *saved* Wi-Fi connection profile (nmcli calls these
/// "connections" — distinct from the live scan results
/// `get_wifi_networks_real` returns, which only show currently-visible
/// networks). This is what "forget network"/"edit saved connections"
/// needs: a network can be saved (and therefore forgettable/editable)
/// without being in range right now, and conversely a network can be in
/// range without ever having been saved.
#[tauri::command]
pub fn get_saved_wifi_connections() -> Vec<String> {
    match Command::new("nmcli").args(["-t", "-f", "NAME,TYPE", "connection", "show"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                let parts = split_nmcli_terse(line);
                if parts.get(1).map(|t| t == "802-11-wireless").unwrap_or(false) {
                    parts.first().cloned()
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// "Forget network" — deletes the saved connection profile (stored
/// credentials/settings) for `ssid` entirely, same as NetworkManager's
/// own "Forget" action. This is different from `disconnect_wifi`, which
/// only drops the *current* session but leaves the saved password in
/// place so it auto-reconnects again later.
#[tauri::command]
pub fn forget_wifi_network(ssid: String) -> Result<(), String> {
    let o = Command::new("nmcli").args(["connection", "delete", &ssid]).output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).to_string()) }
}

/// Renames a saved connection profile — the practical extent of "editing"
/// a saved Wi-Fi connection nmcli exposes without opening a full
/// NetworkManager settings editor; changing the stored password itself
/// requires reconnecting with `connect_wifi_real`, which overwrites it.
#[tauri::command]
pub fn rename_saved_wifi_connection(old_name: String, new_name: String) -> Result<(), String> {
    let o = Command::new("nmcli").args(["connection", "modify", &old_name, "connection.id", &new_name])
        .output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).to_string()) }
}

/// Updates the stored password on a saved connection *without* going
/// through the full disconnect/rescan/reconnect flow `connect_wifi_real`
/// needs — `nmcli connection modify` writes the new PSK directly into the
/// existing profile, then `connection up` re-applies it immediately if
/// the network is in range. This is the "change password" affordance
/// nmcli actually offers; there's no more direct single-command way to
/// edit a saved secret than modify-then-reapply.
#[tauri::command]
pub fn update_saved_wifi_password(name: String, password: String) -> Result<(), String> {
    let modify = Command::new("nmcli")
        .args(["connection", "modify", &name, "wifi-sec.key-mgmt", "wpa-psk", "wifi-sec.psk", &password])
        .output().map_err(|e| e.to_string())?;
    if !modify.status.success() {
        return Err(String::from_utf8_lossy(&modify.stderr).to_string());
    }
    // Re-apply immediately so the new password takes effect right away
    // if the network is currently in range; if it's out of range this
    // simply fails silently here and the new password is used the next
    // time NetworkManager auto-connects to it.
    let _ = Command::new("nmcli").args(["connection", "up", &name]).output();
    Ok(())
}

/// Brings up a saved connection *by profile name* rather than by
/// scanning for its SSID first (`connect_wifi_real` requires the network
/// to currently show up in a scan). This is what makes a saved-but-out-
/// of-range network's "Connect" button actually able to do something
/// once you're back in range, without needing a fresh scan to list it.
#[tauri::command]
pub fn connect_saved_wifi_connection(name: String) -> Result<(), String> {
    let o = Command::new("nmcli").args(["connection", "up", &name]).output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).to_string()) }
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

/// "Forget device" — unpairs and removes the stored pairing/trust
/// entirely (bluetoothctl's own `remove`), rather than just disconnecting
/// the current session. Distinct from `bluetooth_disconnect` the same
/// way `forget_wifi_network` is distinct from `disconnect_wifi` above.
#[tauri::command]
pub fn bluetooth_forget(mac: String) -> Result<(), String> {
    let o = Command::new("bluetoothctl").args(["remove", &mac]).output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).to_string()) }
}

/// Best-effort live RSSI for a *currently connected* Bluetooth device.
///
/// Unlike Wi-Fi (where `nmcli` always reports a signal percentage for
/// every visible network), BlueZ only exposes RSSI for a device while
/// it's actively connected, and only if the adapter/controller
/// populated that property at all — plenty of controllers just don't.
/// `bluetoothctl info <mac>` prints an `RSSI:` line when it's available;
/// this returns `None` rather than a fake number when it isn't, so the
/// frontend can honestly hide the indicator instead of showing a
/// meaningless fixed value.
#[tauri::command]
pub fn get_bluetooth_rssi(mac: String) -> Option<i32> {
    let o = Command::new("bluetoothctl").args(["info", &mac]).output().ok()?;
    let text = String::from_utf8_lossy(&o.stdout);
    text.lines()
        .find(|l| l.trim_start().starts_with("RSSI:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|s| s.trim().split_whitespace().next())
        .and_then(|s| s.parse::<i32>().ok())
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
