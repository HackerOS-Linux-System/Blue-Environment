use crate::types::*;
use crate::session;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[tauri::command]
pub fn get_system_stats() -> SystemStats {
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
pub fn get_network_disk_rates() -> (f32, f32, f32, f32) {
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
pub fn read_proc_diskstats() -> Option<(u64, u64)> {
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

pub fn get_battery_info() -> (f32, bool) {
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

pub fn get_brightness() -> i32 {
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

pub fn get_pipewire_volume() -> Option<i32> {
    let out = Command::new("pactl").args(["get-sink-volume", "@DEFAULT_SINK@"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split('%').next()?.rsplit('/').next()?.trim().parse().ok()
}

pub fn get_alsa_volume() -> i32 {
    Command::new("sh")
    .arg("-c")
    .arg("amixer get Master 2>/dev/null | grep -o '[0-9]*%' | head -1")
    .output()
    .map(|o| String::from_utf8_lossy(&o.stdout).replace('%', "").trim().parse().unwrap_or(50))
    .unwrap_or(50)
}

#[tauri::command]
pub fn get_audio_sinks() -> Vec<AudioSink> {
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
pub fn set_sink_volume(sink_name: String, volume: f32) -> Result<(), String> {
    Command::new("pactl").args(["set-sink-volume", &sink_name, &format!("{}%", volume.clamp(0.0, 150.0) as u32)]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_default_sink(sink_name: String) -> Result<(), String> {
    Command::new("pactl").args(["set-default-sink", &sink_name]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_sink_mute(sink_name: String) -> Result<(), String> {
    Command::new("pactl").args(["set-sink-mute", &sink_name, "toggle"]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_volume(level: i32) {
    let _ = Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", level.clamp(0, 150))]).spawn();
}
