pub mod qemu_args;

use qemu_args::{build_qemu_args, build_qemu_img_create_args, OsType, VmConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;

fn virt_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BLUE_VIRT_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local/share/Blue-Environment/blue-virt")
}
fn vms_config_path() -> PathBuf {
    virt_dir().join("vms.json")
}
fn disks_dir() -> PathBuf {
    virt_dir().join("disks")
}
fn runtime_dir() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("blue-virt")
}
fn monitor_socket_path(vm_id: &str) -> PathBuf {
    runtime_dir().join(format!("{vm_id}-monitor.sock"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VmStatus {
    Stopped,
    Running,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VmSummary {
    #[serde(flatten)]
    pub config: VmConfig,
    pub status: VmStatus,
}

/// Running VM processes, keyed by VM id — the only piece of state that
/// genuinely can't be persisted to disk and reloaded (a `Child` handle
/// is only meaningful for the process that spawned it), unlike
/// `VmConfig`s themselves, which live in `vms.json`.
static RUNNING: Mutex<Option<HashMap<String, Child>>> = Mutex::new(None);

fn with_running<T>(f: impl FnOnce(&mut HashMap<String, Child>) -> T) -> T {
    let mut guard = RUNNING.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().unwrap())
}

fn read_configs() -> Vec<VmConfig> {
    fs::read_to_string(vms_config_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_configs(configs: &[VmConfig]) -> Result<(), String> {
    fs::create_dir_all(virt_dir()).map_err(|e| e.to_string())?;
    fs::write(vms_config_path(), serde_json::to_string_pretty(configs).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// `/dev/kvm` accessible means real hardware-accelerated virtualization
/// is available; anything else means QEMU falls back to pure software
/// emulation (still functionally correct, just far slower — see
/// `qemu_args.rs`'s `-cpu qemu64` fallback for that path).
#[tauri::command]
pub fn bv_is_kvm_available() -> bool {
    std::path::Path::new("/dev/kvm").exists()
        && fs::OpenOptions::new().read(true).write(true).open("/dev/kvm").is_ok()
}

#[tauri::command]
pub fn bv_list_vms() -> Vec<VmSummary> {
    let configs = read_configs();
    with_running(|running| {
        configs
            .into_iter()
            .map(|config| {
                let status = if running.contains_key(&config.id) { VmStatus::Running } else { VmStatus::Stopped };
                VmSummary { config, status }
            })
            .collect()
    })
}

/// Creates a new VM: allocates a real `qcow2` disk image via
/// `qemu-img create` and saves the configuration. Does not start it —
/// see [`bv_start_vm`].
#[tauri::command]
pub fn bv_create_vm(
    name: String,
    os_type: OsType,
    cpu_cores: u32,
    memory_mb: u32,
    disk_size_gb: u32,
    iso_path: Option<String>,
) -> Result<VmConfig, String> {
    let id = format!("vm-{}", chrono::Utc::now().timestamp_millis());
    fs::create_dir_all(disks_dir()).map_err(|e| e.to_string())?;
    let disk_path = disks_dir().join(format!("{id}.qcow2"));

    let args = build_qemu_img_create_args(disk_path.to_str().ok_or("invalid disk path")?, disk_size_gb);
    let output = Command::new("qemu-img")
        .args(&args)
        .output()
        .map_err(|e| format!("failed to run qemu-img (is QEMU installed?): {e}"))?;
    if !output.status.success() {
        return Err(format!("qemu-img create failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let config = VmConfig {
        id,
        name,
        os_type,
        cpu_cores: cpu_cores.max(1),
        memory_mb: memory_mb.max(256),
        disk_path: disk_path.to_string_lossy().to_string(),
        disk_size_gb,
        iso_path,
        use_kvm: bv_is_kvm_available(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut configs = read_configs();
    configs.push(config.clone());
    write_configs(&configs)?;

    Ok(config)
}

#[tauri::command]
pub fn bv_delete_vm(id: String) -> Result<(), String> {
    // Refuse to delete a running VM's disk out from under it rather
    // than silently stopping it first — deleting is destructive and
    // shouldn't have an implicit "and also stop it" side effect a
    // person didn't ask for.
    if with_running(|r| r.contains_key(&id)) {
        return Err("Stop the VM before deleting it".to_string());
    }

    let mut configs = read_configs();
    let Some(pos) = configs.iter().position(|c| c.id == id) else {
        return Err("VM not found".to_string());
    };
    let config = configs.remove(pos);
    write_configs(&configs)?;

    let _ = fs::remove_file(&config.disk_path); // best-effort — config removal is the source of truth either way

    Ok(())
}

/// Spawns a real `qemu-system-x86_64` process for `id` — see module
/// doc's "What's not real yet" for the one part of this file not
/// executed against a real QEMU binary in this environment.
#[tauri::command]
pub fn bv_start_vm(id: String) -> Result<(), String> {
    if with_running(|r| r.contains_key(&id)) {
        return Err("VM is already running".to_string());
    }
    let configs = read_configs();
    let config = configs.iter().find(|c| c.id == id).ok_or("VM not found")?;

    fs::create_dir_all(runtime_dir()).map_err(|e| e.to_string())?;
    let monitor_path = monitor_socket_path(&id);
    let _ = fs::remove_file(&monitor_path); // stale socket from an unclean previous shutdown

    let args = build_qemu_args(config, monitor_path.to_str().ok_or("invalid monitor socket path")?);
    let child = Command::new("qemu-system-x86_64")
        .args(&args)
        .spawn()
        .map_err(|e| format!("failed to launch qemu-system-x86_64 (is QEMU installed?): {e}"))?;

    with_running(|running| running.insert(id, child));
    Ok(())
}

/// Stops VM `id` — sends `system_powerdown` over the QEMU monitor
/// socket for a graceful guest shutdown unless `force` is `true` (or
/// the monitor connection itself fails), in which case the process is
/// killed directly. See module doc's "What's real" section for why the
/// monitor path is preferred.
#[tauri::command]
pub fn bv_stop_vm(id: String, force: bool) -> Result<(), String> {
    if !force {
        if let Ok(mut stream) = UnixStream::connect(monitor_socket_path(&id)) {
            // QEMU's human monitor protocol is line-based plain text —
            // `system_powerdown\n` is the exact command that triggers
            // an ACPI power button event to the guest, the same signal
            // a real power button press sends.
            let _ = stream.write_all(b"system_powerdown\n");
            // Give the guest OS a real chance to shut down cleanly
            // before this function returns — a caller polling
            // `bv_list_vms` afterward should see it transition to
            // Stopped once the process actually exits (see the process
            // reaping below), not still show Running because we
            // returned immediately after only *asking* it to power off.
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    }

    with_running(|running| {
        if let Some(mut child) = running.remove(&id) {
            match child.try_wait() {
                Ok(Some(_)) => {} // already exited cleanly from the powerdown request above
                _ => {
                    let _ = child.kill(); // graceful attempt didn't finish in time, or `force` was requested
                    let _ = child.wait();
                }
            }
        }
    });
    let _ = fs::remove_file(monitor_socket_path(&id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn isolated_test_env() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("blue-virt-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("BLUE_VIRT_DIR", &dir);
        dir
    }

    fn sample_config(id: &str) -> VmConfig {
        VmConfig {
            id: id.to_string(),
            name: "Test VM".to_string(),
            os_type: OsType::Linux,
            cpu_cores: 2,
            memory_mb: 2048,
            disk_path: "/tmp/fake.qcow2".to_string(),
            disk_size_gb: 20,
            iso_path: None,
            use_kvm: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn configs_round_trip_through_disk() {
        let dir = isolated_test_env();
        write_configs(&[sample_config("vm1"), sample_config("vm2")]).unwrap();
        let loaded = read_configs();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "vm1");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn list_vms_reports_stopped_for_everything_not_in_the_running_map() {
        let dir = isolated_test_env();
        write_configs(&[sample_config("vm1")]).unwrap();

        let list = bv_list_vms();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, VmStatus::Stopped);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_an_unknown_vm_id_fails_cleanly() {
        let dir = isolated_test_env();
        assert!(bv_delete_vm("does-not-exist".to_string()).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_removes_config_and_disk_file() {
        let dir = isolated_test_env();
        let mut config = sample_config("vm1");
        let fake_disk = dir.join("fake_disk.qcow2");
        fs::write(&fake_disk, b"not a real qcow2 but good enough for this test").unwrap();
        config.disk_path = fake_disk.to_string_lossy().to_string();
        write_configs(&[config]).unwrap();

        bv_delete_vm("vm1".to_string()).unwrap();

        assert!(read_configs().is_empty());
        assert!(!fake_disk.exists(), "the disk image file should be removed too, not just the config entry");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn kvm_availability_check_does_not_panic_regardless_of_environment() {
        // Can't assert a specific true/false result (depends on the
        // machine this test runs on / whether /dev/kvm exists and is
        // accessible), but it must never panic — a VM manager failing
        // to even *check* for hardware acceleration shouldn't take the
        // rest of the app down with it.
        let _ = bv_is_kvm_available();
    }
}
