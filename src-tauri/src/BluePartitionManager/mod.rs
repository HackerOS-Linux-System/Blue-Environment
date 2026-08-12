use serde::Serialize;
use std::process::Command;

/// One row in the partition table view — either a whole disk (`kind ==
/// "disk"`) or a partition/child device nested under one. Mirrors the shape
/// `lsblk -J` returns so we don't have to invent our own device model.
#[derive(Serialize, Clone, Debug)]
pub struct BpmDevice {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
    pub fstype: Option<String>,
    pub label: Option<String>,
    pub mountpoint: Option<String>,
    pub model: Option<String>,
    pub uuid: Option<String>,
    pub removable: bool,
    pub read_only: bool,
    pub children: Vec<BpmDevice>,
}

fn parse_size(v: &serde_json::Value) -> u64 {
    // lsblk -b prints SIZE as a bare integer string when -J/-b are combined,
    // but some distros' util-linux builds still quote it — accept both.
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

fn parse_bool_field(v: &serde_json::Value) -> bool {
    matches!(v.as_str(), Some("1")) || v.as_bool().unwrap_or(false)
}

fn parse_device(v: &serde_json::Value) -> BpmDevice {
    let name = v["name"].as_str().unwrap_or("").to_string();
    let children = v["children"]
        .as_array()
        .map(|arr| arr.iter().map(parse_device).collect())
        .unwrap_or_default();

    BpmDevice {
        path: format!("/dev/{name}"),
        name,
        kind: v["type"].as_str().unwrap_or("part").to_string(),
        size_bytes: parse_size(&v["size"]),
        fstype: v["fstype"].as_str().filter(|s| !s.is_empty()).map(String::from),
        label: v["label"].as_str().filter(|s| !s.is_empty()).map(String::from),
        mountpoint: v["mountpoint"].as_str().filter(|s| !s.is_empty()).map(String::from),
        model: v["model"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        uuid: v["uuid"].as_str().filter(|s| !s.is_empty()).map(String::from),
        removable: parse_bool_field(&v["rm"]),
        read_only: parse_bool_field(&v["ro"]),
        children,
    }
}

/// Lists every block device (disks + their partitions, nested) using
/// `lsblk`, the same source of truth Blue Installer already relies on for
/// disk detection — see BlueInstallerApp::installer_list_disks.
#[tauri::command]
pub async fn bpm_list_devices() -> Result<Vec<BpmDevice>, String> {
    let output = Command::new("lsblk")
        .args(["-b", "-J", "-o", "NAME,SIZE,TYPE,FSTYPE,LABEL,MOUNTPOINT,MODEL,UUID,RM,RO"])
        .output()
        .map_err(|e| format!("lsblk failed to start: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "lsblk exited with an error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse lsblk output: {e}"))?;

    let devices = json["blockdevices"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(parse_device)
        // Only show real disks/loop/rom devices at the top level; their
        // partitions already come through as `children`.
        .filter(|d| matches!(d.kind.as_str(), "disk" | "loop" | "rom"))
        .collect();

    Ok(devices)
}

/// Mounts a partition through udisks2 (no root required for removable /
/// user-owned media — this is the same mechanism GNOME Files & Dolphin use).
#[tauri::command]
pub async fn bpm_mount(device: String) -> Result<String, String> {
    let output = Command::new("udisksctl")
        .args(["mount", "-b", &device])
        .output()
        .map_err(|e| format!("Failed to launch udisksctl: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Unmounts a partition through udisks2.
#[tauri::command]
pub async fn bpm_unmount(device: String) -> Result<(), String> {
    let output = Command::new("udisksctl")
        .args(["unmount", "-b", &device])
        .output()
        .map_err(|e| format!("Failed to launch udisksctl: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

/// Formats a partition. Destructive and privileged, so it goes through
/// `pkexec` for a real authentication prompt — same pattern as
/// `installer_run` in BlueInstallerApp. `label` is optional.
#[tauri::command]
pub async fn bpm_format(device: String, fstype: String, label: Option<String>) -> Result<(), String> {
    let mkfs_bin = match fstype.as_str() {
        "ext4" => "mkfs.ext4",
        "btrfs" => "mkfs.btrfs",
        "xfs" => "mkfs.xfs",
        "fat32" | "vfat" => "mkfs.vfat",
        "ntfs" => "mkfs.ntfs",
        "swap" => "mkswap",
        other => return Err(format!("Unsupported filesystem: {other}")),
    };

    let mut args: Vec<String> = Vec::new();
    if let Some(l) = label.filter(|l| !l.is_empty()) {
        match fstype.as_str() {
            "ext4" | "btrfs" | "swap" => { args.push("-L".into()); args.push(l); }
            "xfs" => { args.push("-L".into()); args.push(l); }
            "fat32" | "vfat" => { args.push("-n".into()); args.push(l); }
            "ntfs" => { args.push("-L".into()); args.push(l); }
            _ => {}
        }
    }
    if fstype == "fat32" || fstype == "vfat" { args.push("-F".into()); args.push("32".into()); }
    if matches!(fstype.as_str(), "ext4" | "xfs" | "vfat" | "fat32" | "ntfs") { args.push("-f".into()); }
    args.push(device.clone());

    let output = Command::new("pkexec")
        .arg(mkfs_bin)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to launch pkexec/{mkfs_bin}: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

/// Renames the filesystem label of an already-formatted partition.
#[tauri::command]
pub async fn bpm_set_label(device: String, fstype: String, label: String) -> Result<(), String> {
    let (bin, args): (&str, Vec<String>) = match fstype.as_str() {
        "ext4" => ("e2label", vec![device.clone(), label]),
        "btrfs" => ("btrfs", vec!["filesystem".into(), "label".into(), device.clone(), label]),
        "xfs" => ("xfs_admin", vec!["-L".into(), label, device.clone()]),
        "fat32" | "vfat" => ("fatlabel", vec![device.clone(), label]),
        "ntfs" => ("ntfslabel", vec![device.clone(), label]),
        other => return Err(format!("Relabeling {other} is not supported")),
    };

    let output = Command::new("pkexec")
        .arg(bin)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to launch pkexec/{bin}: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

// ── SMART health monitoring ──────────────────────────────────────────
// KDE Partition Manager has no built-in disk health view at all — you
// need a separate tool (gsmartcontrol, or the `smartctl` CLI directly).
// This surfaces the same data straight in Blue Partition Manager.

#[derive(Serialize, Clone, Debug)]
pub struct SmartAttribute {
    pub id: u32,
    pub name: String,
    pub value: i64,
    pub worst: i64,
    pub threshold: i64,
    pub raw: String,
    /// True if this attribute's normalized value has dropped to/below its
    /// threshold — smartctl's own pass/fail signal per-attribute.
    pub failing: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct SmartStatus {
    pub available: bool,
    /// None when `available` is false (smartctl missing, permission
    /// denied, or the device doesn't support SMART — common for USB
    /// flash drives and some NVMe-over-USB enclosures).
    pub healthy: Option<bool>,
    pub temperature_celsius: Option<i64>,
    pub power_on_hours: Option<i64>,
    pub power_cycle_count: Option<i64>,
    pub attributes: Vec<SmartAttribute>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub error: Option<String>,
}

/// Reads SMART health data for a whole-disk device (not a partition —
/// SMART lives at the physical drive level) via `smartctl -a -j`, which
/// works for both classic ATA attribute tables and NVMe's own health log
/// under one uniform JSON shape, so this doesn't need two separate code
/// paths for spinning disks/SATA SSDs vs. NVMe.
#[tauri::command]
pub async fn bpm_smart_status(device: String) -> SmartStatus {
    let empty = |err: &str| SmartStatus {
        available: false, healthy: None, temperature_celsius: None,
        power_on_hours: None, power_cycle_count: None, attributes: vec![],
        model: None, serial: None, error: Some(err.to_string()),
    };

    if Command::new("which").arg("smartctl").output().map(|o| !o.status.success()).unwrap_or(true) {
        return empty("smartctl not installed (part of smartmontools) — install it to see disk health.");
    }

    // `-a` (all info) `-j` (JSON) — needs root for direct ATA/NVMe
    // passthrough on most systems, hence pkexec. Read-only operation.
    let output = match Command::new("pkexec").args(["smartctl", "-a", "-j", &device]).output() {
        Ok(o) => o,
        Err(e) => return empty(&format!("Failed to launch smartctl: {e}")),
    };

    let json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(j) => j,
        Err(_) => return empty("Device doesn't support SMART, or access was denied."),
    };

    if json["smart_support"]["available"].as_bool() == Some(false) {
        return empty("This device doesn't report SMART data (common for USB flash drives).");
    }

    let healthy = json["smart_status"]["passed"].as_bool();
    let temperature_celsius = json["temperature"]["current"].as_i64();
    let power_on_hours = json["power_on_time"]["hours"].as_i64();
    let power_cycle_count = json["power_cycle_count"].as_i64();
    let model = json["model_name"].as_str().map(String::from);
    let serial = json["serial_number"].as_str().map(String::from);

    // Classic ATA SMART attribute table (SATA HDDs/SSDs). NVMe drives
    // instead report a flat health log with no per-attribute table —
    // `attributes` is simply empty for those, and the top-level
    // healthy/temperature/power fields above (which NVMe's JSON also
    // populates under the same keys) carry the useful signal instead.
    let attributes: Vec<SmartAttribute> = json["ata_smart_attributes"]["table"]
        .as_array()
        .map(|arr| arr.iter().map(|a| {
            let value = a["value"].as_i64().unwrap_or(0);
            let threshold = a["thresh"].as_i64().unwrap_or(0);
            SmartAttribute {
                id: a["id"].as_u64().unwrap_or(0) as u32,
                name: a["name"].as_str().unwrap_or("Unknown").replace('_', " "),
                value,
                worst: a["worst"].as_i64().unwrap_or(0),
                threshold,
                raw: a["raw"]["string"].as_str().unwrap_or("").to_string(),
                failing: threshold > 0 && value <= threshold,
            }
        }).collect())
        .unwrap_or_default();

    SmartStatus {
        available: true, healthy, temperature_celsius, power_on_hours,
        power_cycle_count, attributes, model, serial, error: None,
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct BenchmarkResult {
    pub read_mb_per_sec: f64,
    pub sample_size_mb: f64,
}

/// A quick, read-only sequential-read benchmark — `dd` reading a chunk
/// straight off the block device with caches bypassed (`iflag=direct`),
/// timed on our side. Not as rigorous as `fio`, but needs no extra
/// dependency and gives a genuinely useful ballpark figure (HDD vs SSD
/// vs NVMe are trivially distinguishable from the result), which is more
/// than KDE Partition Manager offers — it has no benchmarking at all.
#[tauri::command]
pub async fn bpm_benchmark_read(device: String) -> Result<BenchmarkResult, String> {
    const SAMPLE_MB: u64 = 256;
    let start = std::time::Instant::now();
    let output = Command::new("dd")
        .args([
            &format!("if={device}"),
            "of=/dev/null",
            "bs=1M",
            &format!("count={SAMPLE_MB}"),
            "iflag=direct",
        ])
        .output()
        .map_err(|e| format!("Failed to run dd: {e}"))?;

    if !output.status.success() {
        // `iflag=direct` can fail on filesystems/devices that don't
        // support O_DIRECT (some loopback/virtual devices) — retry once
        // without it rather than just failing outright.
        let fallback = Command::new("dd")
            .args([&format!("if={device}"), "of=/dev/null", "bs=1M", &format!("count={SAMPLE_MB}")])
            .output()
            .map_err(|e| format!("Failed to run dd: {e}"))?;
        if !fallback.status.success() {
            return Err(String::from_utf8_lossy(&fallback.stderr).trim().to_string());
        }
    }

    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    Ok(BenchmarkResult { read_mb_per_sec: SAMPLE_MB as f64 / elapsed, sample_size_mb: SAMPLE_MB as f64 })
}
