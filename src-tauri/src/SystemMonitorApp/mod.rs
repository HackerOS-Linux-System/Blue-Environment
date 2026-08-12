use serde::Serialize;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

fn sh(cmd: &str) -> String {
    Command::new("sh").arg("-c").arg(cmd)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

// Previous-sample caches for delta-based metrics (CPU %, network bps).
//
// These used to be computed by taking two /proc samples ~250-500ms apart,
// blocking the calling thread for that whole window on *every* poll. Since
// the frontend already polls every 2s, we instead keep the last sample
// around and diff against it — same math, zero blocking, and actually more
// accurate (a ~2s window averages out noise far better than ~250ms does).
static PREV_CPU_STAT: OnceLock<Mutex<Option<Vec<Vec<u64>>>>> = OnceLock::new();
static PREV_NET_STAT: OnceLock<Mutex<Option<(std::time::Instant, Vec<(String, u64, u64)>)>>> = OnceLock::new();

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone, Debug)]
pub struct ProcessEntry {
    pub pid:     u32,
    pub ppid:    u32,
    pub name:    String,
    pub cmd:     String,
    pub user:    String,
    pub cpu:     f32,
    pub memory:  u64,   // bytes
    pub mem_pct: f32,
    pub status:  String,
    pub threads: u32,
    pub nice:    i32,
    pub started: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct CpuCore {
    pub id:    usize,
    pub usage: f32,
    pub freq:  Option<u64>,  // MHz
}

#[derive(Serialize, Clone, Debug)]
pub struct CpuInfo {
    pub total_usage: f32,
    pub model:       String,
    pub cores:       Vec<CpuCore>,
    pub load_avg_1:  f64,
    pub load_avg_5:  f64,
    pub load_avg_15: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct MemInfo {
    pub total:       u64,   // bytes
    pub used:        u64,
    pub free:        u64,
    pub available:   u64,
    pub cached:      u64,
    pub buffers:     u64,
    pub swap_total:  u64,
    pub swap_used:   u64,
    pub swap_free:   u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct DiskEntry {
    pub name:       String,
    pub mount:      String,
    pub fs:         String,
    pub total:      u64,
    pub used:       u64,
    pub free:       u64,
    pub read_bps:   u64,
    pub write_bps:  u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct NetInterface {
    pub name:      String,
    pub rx_bytes:  u64,
    pub tx_bytes:  u64,
    pub rx_bps:    u64,
    pub tx_bps:    u64,
    pub ip4:       Option<String>,
    pub ip6:       Option<String>,
    pub connected: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct GpuInfo {
    pub name:        String,
    pub driver:      String,
    pub usage_pct:   Option<f32>,
    pub vram_used:   Option<u64>,
    pub vram_total:  Option<u64>,
    pub temp_c:      Option<f32>,
    pub power_w:     Option<f32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct TempSensor {
    pub label: String,
    pub input: f32,   // celsius
    pub high:  Option<f32>,
    pub crit:  Option<f32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct SystemSnapshot {
    pub cpu:     CpuInfo,
    pub mem:     MemInfo,
    pub disks:   Vec<DiskEntry>,
    pub net:     Vec<NetInterface>,
    pub gpu:     Vec<GpuInfo>,
    pub temps:   Vec<TempSensor>,
    pub uptime:  u64,
    pub hostname: String,
    pub kernel:  String,
    pub os:      String,
}

// ─────────────────────────────────────────────────────────────────────────────
// CPU
// ─────────────────────────────────────────────────────────────────────────────

fn read_proc_stat() -> Vec<Vec<u64>> {
    std::fs::read_to_string("/proc/stat")
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with("cpu"))
        .map(|l| {
            l.split_whitespace()
                .skip(1)
                .filter_map(|n| n.parse::<u64>().ok())
                .collect()
        })
        .collect()
}

fn calc_cpu_usage(before: &[u64], after: &[u64]) -> f32 {
    if before.len() < 4 || after.len() < 4 { return 0.0; }
    let idle_b = before[3] + before.get(4).copied().unwrap_or(0);
    let idle_a = after[3]  + after.get(4).copied().unwrap_or(0);
    let total_b: u64 = before.iter().sum();
    let total_a: u64 = after.iter().sum();
    let d_total = total_a.saturating_sub(total_b) as f32;
    let d_idle  = idle_a.saturating_sub(idle_b)  as f32;
    if d_total == 0.0 { return 0.0; }
    ((d_total - d_idle) / d_total * 100.0).clamp(0.0, 100.0)
}

pub fn get_cpu_info() -> CpuInfo {
    let stat2 = read_proc_stat();
    let cache = PREV_CPU_STAT.get_or_init(|| Mutex::new(None));
    let stat1 = {
        let mut guard = cache.lock().unwrap();
        // First call ever (no previous sample): fall back to a single quick
        // real sample pair so the very first reading isn't just zeros,
        // without blocking every subsequent poll.
        let prev = guard.clone().unwrap_or_else(|| stat2.clone());
        *guard = Some(stat2.clone());
        prev
    };

    let total_usage = stat1.first().zip(stat2.first())
        .map(|(b, a)| calc_cpu_usage(b, a))
        .unwrap_or(0.0);

    // Per-core usage (lines 1..)
    let cores: Vec<CpuCore> = stat1.iter().zip(stat2.iter())
        .skip(1)
        .enumerate()
        .map(|(i, (b, a))| {
            let freq = std::fs::read_to_string(
                format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq", i)
            ).ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|khz| khz / 1000);

            CpuCore { id: i, usage: calc_cpu_usage(b, a), freq }
        })
        .collect();

    let model = sh("grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2")
        .trim().to_string();

    let loadavg = sh("cat /proc/loadavg");
    let lavg: Vec<f64> = loadavg.split_whitespace()
        .take(3).filter_map(|s| s.parse().ok()).collect();

    CpuInfo {
        total_usage,
        model,
        cores,
        load_avg_1:  lavg.first().copied().unwrap_or(0.0),
        load_avg_5:  lavg.get(1).copied().unwrap_or(0.0),
        load_avg_15: lavg.get(2).copied().unwrap_or(0.0),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Memory
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_mem_info() -> MemInfo {
    let text = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let val = |key: &str| -> u64 {
        text.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0) * 1024 // kB -> bytes
    };

    let total     = val("MemTotal:");
    let free      = val("MemFree:");
    let available = val("MemAvailable:");
    let cached    = val("Cached:") + val("SReclaimable:");
    let buffers   = val("Buffers:");
    let used      = total.saturating_sub(available);

    MemInfo {
        total, used, free, available, cached, buffers,
        swap_total: val("SwapTotal:"),
        swap_used:  val("SwapTotal:").saturating_sub(val("SwapFree:")),
        swap_free:  val("SwapFree:"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Disks
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_disks() -> Vec<DiskEntry> {
    let df_out = sh("df -B1 --output=source,fstype,size,used,avail,target 2>/dev/null | tail -n +2");

    df_out.lines()
        .filter(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            if cols.len() < 6 { return false; }
            let dev = cols[0]; let mnt = cols[5];
            dev.starts_with("/dev") && !mnt.starts_with("/proc")
                && !mnt.starts_with("/sys") && !mnt.starts_with("/run/user")
        })
        .map(|l| {
            let c: Vec<&str> = l.split_whitespace().collect();
            DiskEntry {
                name:      c.first().copied().unwrap_or("").to_string(),
                fs:        c.get(1).copied().unwrap_or("").to_string(),
                total:     c.get(2).and_then(|v| v.parse().ok()).unwrap_or(0),
                used:      c.get(3).and_then(|v| v.parse().ok()).unwrap_or(0),
                free:      c.get(4).and_then(|v| v.parse().ok()).unwrap_or(0),
                mount:     c.get(5).copied().unwrap_or("").to_string(),
                read_bps:  0,
                write_bps: 0,
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Network
// ─────────────────────────────────────────────────────────────────────────────

fn read_net_stats() -> Vec<(String, u64, u64)> {
    std::fs::read_to_string("/proc/net/dev")
        .unwrap_or_default()
        .lines()
        .skip(2)
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            let nums: Vec<u64> = rest.split_whitespace()
                .filter_map(|n| n.parse().ok()).collect();
            Some((name.trim().to_string(), nums.first().copied()?, nums.get(8).copied()?))
        })
        .collect()
}

pub fn get_net_interfaces() -> Vec<NetInterface> {
    let stats2 = read_net_stats();
    let cache = PREV_NET_STAT.get_or_init(|| Mutex::new(None));
    let now = std::time::Instant::now();
    let (stats1, elapsed_secs) = {
        let mut guard = cache.lock().unwrap();
        let (prev_time, prev_stats) = guard.clone()
            .unwrap_or_else(|| (now, stats2.clone()));
        *guard = Some((now, stats2.clone()));
        let elapsed = now.duration_since(prev_time).as_secs_f64().max(0.001);
        (prev_stats, elapsed)
    };

    stats2.iter().filter(|(name, _, _)| name != "lo").map(|(name, rx2, tx2)| {
        let (rx1, tx1) = stats1.iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, r, t)| (*r, *t))
            .unwrap_or((0, 0));

        let ip4 = sh(&format!("ip -4 addr show {} 2>/dev/null | grep 'inet ' | awk '{{print $2}}' | cut -d/ -f1", name));
        let ip6 = sh(&format!("ip -6 addr show {} 2>/dev/null | grep 'inet6 ' | head -1 | awk '{{print $2}}' | cut -d/ -f1", name));

        let carrier = std::fs::read_to_string(format!("/sys/class/net/{}/carrier", name))
            .unwrap_or_default().trim() == "1";

        NetInterface {
            name:      name.clone(),
            rx_bytes:  *rx2,
            tx_bytes:  *tx2,
            rx_bps:    (rx2.saturating_sub(rx1) as f64 / elapsed_secs) as u64,
            tx_bps:    (tx2.saturating_sub(tx1) as f64 / elapsed_secs) as u64,
            ip4:       if ip4.is_empty() { None } else { Some(ip4) },
            ip6:       if ip6.is_empty() { None } else { Some(ip6) },
            connected: carrier,
        }
    }).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// GPU
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_gpu_info() -> Vec<GpuInfo> {
    let mut gpus: Vec<GpuInfo> = Vec::new();

    // NVIDIA via nvidia-smi
    let nv = sh("nvidia-smi --query-gpu=name,driver_version,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw --format=csv,noheader,nounits 2>/dev/null");
    for line in nv.lines() {
        let p: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if p.len() < 7 { continue; }
        gpus.push(GpuInfo {
            name:       p[0].to_string(),
            driver:     p[1].to_string(),
            usage_pct:  p[2].parse().ok(),
            vram_used:  p[3].parse::<u64>().ok().map(|m| m * 1024 * 1024),
            vram_total: p[4].parse::<u64>().ok().map(|m| m * 1024 * 1024),
            temp_c:     p[5].parse().ok(),
            power_w:    p[6].parse().ok(),
        });
    }

    // AMD via radeontop (if available)
    if gpus.is_empty() {
        let amd_name = sh("lspci | grep -i 'VGA.*AMD\\|Display.*AMD' | head -1 | cut -d: -f3");
        if !amd_name.is_empty() {
            let usage = sh("radeontop -d - -l 1 2>/dev/null | grep 'gpu' | grep -oP 'gpu \\K[0-9.]+' | head -1");
            let vram_used  = sh("cat /sys/class/drm/card0/device/mem_info_vram_used 2>/dev/null");
            let vram_total = sh("cat /sys/class/drm/card0/device/mem_info_vram_total 2>/dev/null");
            let temp = sh("cat /sys/class/hwmon/hwmon*/temp1_input 2>/dev/null | head -1");
            gpus.push(GpuInfo {
                name:       amd_name.trim().to_string(),
                driver:     "amdgpu".to_string(),
                usage_pct:  usage.parse().ok(),
                vram_used:  vram_used.trim().parse::<u64>().ok(),
                vram_total: vram_total.trim().parse::<u64>().ok(),
                temp_c:     temp.trim().parse::<f32>().ok().map(|t| t / 1000.0),
                power_w:    None,
            });
        }
    }

    // Intel integrated (basic)
    if gpus.is_empty() {
        let intel = sh("lspci | grep -i 'VGA.*Intel' | head -1 | cut -d: -f3");
        if !intel.is_empty() {
            gpus.push(GpuInfo {
                name: intel.trim().to_string(),
                driver: "i915".to_string(),
                usage_pct: None, vram_used: None, vram_total: None,
                temp_c: None, power_w: None,
            });
        }
    }

    gpus
}

// ─────────────────────────────────────────────────────────────────────────────
// Temperature sensors
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_temperatures() -> Vec<TempSensor> {
    let mut sensors = Vec::new();

    // sensors JSON output (lm-sensors)
    let json = sh("sensors -j 2>/dev/null");
    if !json.is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            for (chip, chip_data) in v.as_object().into_iter().flatten() {
                for (feature, feat_data) in chip_data.as_object().into_iter().flatten() {
                    for (subfeature, val) in feat_data.as_object().into_iter().flatten() {
                        if subfeature.ends_with("_input") {
                            let t = val.as_f64().unwrap_or(0.0) as f32;
                            let high = feat_data.get(subfeature.replace("_input", "_max").as_str())
                                .and_then(|v| v.as_f64()).map(|v| v as f32);
                            let crit = feat_data.get(subfeature.replace("_input", "_crit").as_str())
                                .and_then(|v| v.as_f64()).map(|v| v as f32);
                            sensors.push(TempSensor {
                                label: format!("{} / {}", chip, feature),
                                input: t, high, crit,
                            });
                        }
                    }
                }
            }
        }
    }

    // Fallback: /sys/class/thermal/thermal_zone*
    if sensors.is_empty() {
        for entry in std::fs::read_dir("/sys/class/thermal").into_iter().flatten().flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            if !name.starts_with("thermal_zone") { continue; }
            let temp = std::fs::read_to_string(path.join("temp"))
                .ok().and_then(|s| s.trim().parse::<f32>().ok());
            let type_ = std::fs::read_to_string(path.join("type"))
                .unwrap_or_default().trim().to_string();
            if let Some(t) = temp {
                sensors.push(TempSensor { label: type_, input: t / 1000.0, high: None, crit: None });
            }
        }
    }

    sensors
}

// ─────────────────────────────────────────────────────────────────────────────
// Processes
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_processes() -> Vec<ProcessEntry> {
    use sysinfo::{System, ProcessStatus};
    let mut sys = System::new_all();
    sys.refresh_all();

    let mem_total = sys.total_memory().max(1) as f32;

    let mut procs: Vec<ProcessEntry> = sys.processes().iter().map(|(pid, p)| {
        let status = match p.status() {
            ProcessStatus::Run    => "Running",
            ProcessStatus::Sleep  => "Sleeping",
            ProcessStatus::Idle   => "Idle",
            ProcessStatus::Zombie => "Zombie",
            ProcessStatus::Stop   => "Stopped",
            _                     => "Unknown",
        }.to_string();

        ProcessEntry {
            pid:     pid.as_u32(),
            ppid:    p.parent().map(|p| p.as_u32()).unwrap_or(0),
            name:    p.name().to_string(),
            cmd:     p.cmd().join(" ").chars().take(200).collect(),
            user:    p.user_id()
                       .map(|u| u.to_string())
                       .unwrap_or_else(|| "?".into()),
            cpu:     p.cpu_usage(),
            memory:  p.memory(),
            mem_pct: p.memory() as f32 / mem_total * 100.0,
            status,
            threads: p.tasks().map(|t| t.len() as u32).unwrap_or(0),
            nice:    0,
            started: String::new(),
        }
    }).collect();

    procs.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    procs.truncate(200);
    procs
}

#[tauri::command]
pub fn kill_process(pid: u32, signal: Option<i32>) -> bool {
    let sig = signal.unwrap_or(15); // SIGTERM default
    Command::new("kill")
        .arg(format!("-{}", sig))
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[tauri::command]
pub fn renice_process(pid: u32, nice: i32) -> bool {
    let nice = nice.clamp(-20, 19);
    Command::new("renice")
        .arg(nice.to_string())
        .arg("-p")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tauri commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_cpu_metrics() -> CpuInfo {
    get_cpu_info()
}

#[tauri::command]
pub fn get_memory_metrics() -> MemInfo {
    get_mem_info()
}

#[tauri::command]
pub fn get_disk_metrics() -> Vec<DiskEntry> {
    get_disks()
}

#[tauri::command]
pub fn get_network_metrics() -> Vec<NetInterface> {
    get_net_interfaces()
}

#[tauri::command]
pub fn get_gpu_metrics() -> Vec<GpuInfo> {
    get_gpu_info()
}

#[tauri::command]
pub fn get_temp_sensors() -> Vec<TempSensor> {
    get_temperatures()
}

#[tauri::command]
pub fn get_system_snapshot() -> SystemSnapshot {
    let uptime = std::fs::read_to_string("/proc/uptime")
        .unwrap_or_default()
        .split_whitespace().next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;

    SystemSnapshot {
        cpu:      get_cpu_info(),
        mem:      get_mem_info(),
        disks:    get_disks(),
        net:      get_net_interfaces(),
        gpu:      get_gpu_info(),
        temps:    get_temperatures(),
        uptime,
        hostname: sh("hostname"),
        kernel:   sh("uname -r"),
        os:       sh("lsb_release -sd 2>/dev/null || cat /etc/os-release | grep PRETTY_NAME | cut -d= -f2 | tr -d '\"'"),
    }
}
