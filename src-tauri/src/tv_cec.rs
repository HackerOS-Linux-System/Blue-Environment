use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CecStatus {
    /// `/dev/cecN` nodes found, sorted.
    pub adapters: Vec<String>,
    /// Whether `cec-ctl` is installed.
    pub tool_installed: bool,
}

/// `cec0`, `cec12`… under `dev_dir` → `/dev/cec0`-style paths, sorted by number.
/// Anything else in the directory (`video0`, `cecx`, `cec`) is ignored.
pub fn list_cec_devices(dev_dir: &Path) -> Vec<String> {
    let mut found: Vec<(u32, String)> = std::fs::read_dir(dev_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let n: u32 = name.strip_prefix("cec")?.parse().ok()?;
                    Some((n, dev_dir.join(&name).to_string_lossy().into_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

/// Only plain `/dev/cecN` is accepted from the frontend — no other path may
/// ever reach a command line.
pub fn is_valid_cec_device(path: &str) -> bool {
    path.strip_prefix("/dev/cec")
        .map(|n| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

/// Arguments for `cec-ctl`: claim a logical address as a playback device and
/// announce ourselves by name.
pub fn cec_enable_args(device: &str) -> Vec<String> {
    vec![
        "-d".into(), device.into(),
        "--playback".into(),
        "--osd-name".into(), "Blue Environment".into(),
    ]
}

fn cec_ctl_installed() -> bool {
    Command::new("cec-ctl")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[tauri::command(async)]
pub fn tv_cec_status() -> CecStatus {
    CecStatus { adapters: list_cec_devices(Path::new("/dev")), tool_installed: cec_ctl_installed() }
}

/// Registers the adapter so the TV remote starts working. `device` must be one
/// of the nodes reported by [`tv_cec_status`]. Gives `cec-ctl` five seconds.
#[tauri::command(async)]
pub fn tv_cec_enable(device: String) -> Result<String, String> {
    if !is_valid_cec_device(&device) {
        return Err("Not a CEC device.".to_string());
    }
    if !Path::new(&device).exists() {
        return Err("That CEC adapter is not present.".to_string());
    }
    let mut child = Command::new("cec-ctl")
        .args(cec_enable_args(&device))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "cec-ctl is not installed (package v4l-utils).".to_string()
            } else {
                format!("Could not run cec-ctl: {e}")
            }
        })?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child.wait_with_output().map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(if status.success() { &out.stdout } else { &out.stderr }).trim().to_string();
                return if status.success() { Ok(text) } else { Err(if text.is_empty() { "cec-ctl failed.".to_string() } else { text }) };
            }
            Ok(None) if started.elapsed() > Duration::from_secs(5) => {
                let _ = child.kill();
                return Err("cec-ctl did not finish in time.".to_string());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_sorts_cec_nodes_ignoring_everything_else() {
        let dir = std::env::temp_dir().join(format!("blue-cec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["cec10", "cec2", "cec0", "cecx", "cec", "video0", "xcec1"] {
            std::fs::write(dir.join(n), "").unwrap();
        }
        let found = list_cec_devices(&dir);
        let names: Vec<&str> = found.iter().map(|p| p.rsplit('/').next().unwrap()).collect();
        assert_eq!(names, vec!["cec0", "cec2", "cec10"]);
        assert!(list_cec_devices(&dir.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_plain_cec_nodes_are_valid() {
        assert!(is_valid_cec_device("/dev/cec0"));
        assert!(is_valid_cec_device("/dev/cec12"));
        for bad in ["/dev/cec", "/dev/cec0; rm -rf /", "/dev/cec../x", "cec0", "/tmp/cec0", "/dev/cec1234", "/dev/video0", ""] {
            assert!(!is_valid_cec_device(bad), "{bad}");
        }
    }

    #[test]
    fn enable_args_claim_a_playback_address() {
        let a = cec_enable_args("/dev/cec0");
        assert_eq!(&a[..2], ["-d", "/dev/cec0"]);
        assert!(a.contains(&"--playback".to_string()));
        assert!(a.iter().any(|x| x == "Blue Environment"));
    }

    #[test]
    fn rejects_before_running_anything() {
        assert!(tv_cec_enable("/dev/../etc/passwd".into()).is_err());
        assert!(tv_cec_enable("/dev/cec999".into()).is_err()); // valid shape, no such node → clean error, no process
    }
}
