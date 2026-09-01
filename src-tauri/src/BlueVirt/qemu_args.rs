use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OsType {
    Linux,
    Windows,
    Bsd,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmConfig {
    pub id: String,
    pub name: String,
    pub os_type: OsType,
    pub cpu_cores: u32,
    pub memory_mb: u32,
    pub disk_path: String,
    pub disk_size_gb: u32,
    /// Install media — set when a VM is first created (to boot the
    /// installer) and typically cleared afterward by the person once
    /// the OS is installed (Blue Virt doesn't do that automatically;
    /// see `mod.rs`'s module doc).
    pub iso_path: Option<String>,
    pub use_kvm: bool,
    pub created_at: String,
}

/// Builds the full `qemu-system-x86_64` argument list for `vm`,
/// including the QEMU monitor's Unix socket path (`monitor_socket_path`
/// — see `mod.rs`'s `bv_stop_vm` for what that socket is actually used
/// for: sending a graceful `system_powerdown` rather than only ever
/// being able to hard-kill the process).
pub fn build_qemu_args(vm: &VmConfig, monitor_socket_path: &str) -> Vec<String> {
    let mut args = Vec::new();

    args.push("-name".to_string());
    args.push(vm.name.clone());

    args.push("-m".to_string());
    args.push(vm.memory_mb.to_string());

    args.push("-smp".to_string());
    args.push(vm.cpu_cores.to_string());

    if vm.use_kvm {
        args.push("-enable-kvm".to_string());
        args.push("-cpu".to_string());
        args.push("host".to_string());
    } else {
        // Software emulation (TCG) fallback — no `-enable-kvm`, and a
        // generic `-cpu` that doesn't assume host CPU passthrough
        // (which `-cpu host` requires KVM for) works correctly under
        // pure emulation too, just far slower — see `mod.rs`'s
        // `bv_is_kvm_available` for when this path is chosen.
        args.push("-cpu".to_string());
        args.push("qemu64".to_string());
    }

    args.push("-drive".to_string());
    args.push(format!("file={},format=qcow2,if=virtio", vm.disk_path));

    if let Some(iso) = &vm.iso_path {
        args.push("-cdrom".to_string());
        args.push(iso.clone());
        // Boot from the optical drive first whenever install media is
        // attached — matches every other desktop VM tool's default
        // (a fresh VM created with an ISO attached is assumed to be
        // for installing that ISO, not booting an as-yet-empty disk).
        args.push("-boot".to_string());
        args.push("d".to_string());
    } else {
        args.push("-boot".to_string());
        args.push("c".to_string());
    }

    // Networking: a plain user-mode NIC (`-net user`/`-nic user`) needs
    // no host-side setup (no bridge/tap device, no root privileges) —
    // the right default for "just give this VM internet access
    // without asking the person to configure host networking first".
    // Its real limitation (no direct inbound connections to the VM
    // without explicit port forwarding, which nothing here sets up
    // yet) is real, separate follow-up work for anyone wanting to run
    // a server inside a Blue Virt VM.
    args.push("-nic".to_string());
    args.push("user,model=virtio-net-pci".to_string());

    args.push("-monitor".to_string());
    args.push(format!("unix:{monitor_socket_path},server,nowait"));

    // A real GTK window per running VM — the "GNOME Boxes" comparison
    // this app is modeled on also just opens a window per VM rather
    // than embedding the display inside its own UI; embedding would
    // need SPICE protocol integration on top of this, which is real,
    // separate follow-up work (see mod.rs's module doc).
    args.push("-display".to_string());
    args.push("gtk".to_string());

    args
}

/// `qemu-img create -f qcow2 <path> <size>G` argument list — same
/// "pure, testable arg-building" split as [`build_qemu_args`].
pub fn build_qemu_img_create_args(disk_path: &str, size_gb: u32) -> Vec<String> {
    vec![
        "create".to_string(),
        "-f".to_string(),
        "qcow2".to_string(),
        disk_path.to_string(),
        format!("{size_gb}G"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vm() -> VmConfig {
        VmConfig {
            id: "vm1".to_string(),
            name: "Test VM".to_string(),
            os_type: OsType::Linux,
            cpu_cores: 4,
            memory_mb: 4096,
            disk_path: "/home/user/.local/share/blue-virt/vm1.qcow2".to_string(),
            disk_size_gb: 40,
            iso_path: None,
            use_kvm: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn includes_name_memory_and_cpu_flags() {
        let args = build_qemu_args(&sample_vm(), "/tmp/mon.sock");
        assert_eq!(args_value_after(&args, "-name"), Some("Test VM".to_string()));
        assert_eq!(args_value_after(&args, "-m"), Some("4096".to_string()));
        assert_eq!(args_value_after(&args, "-smp"), Some("4".to_string()));
    }

    #[test]
    fn kvm_enabled_adds_enable_kvm_and_host_cpu() {
        let args = build_qemu_args(&sample_vm(), "/tmp/mon.sock");
        assert!(args.contains(&"-enable-kvm".to_string()));
        assert_eq!(args_value_after(&args, "-cpu"), Some("host".to_string()));
    }

    #[test]
    fn kvm_disabled_falls_back_to_software_emulation_cleanly() {
        let mut vm = sample_vm();
        vm.use_kvm = false;
        let args = build_qemu_args(&vm, "/tmp/mon.sock");
        assert!(!args.contains(&"-enable-kvm".to_string()), "must not request KVM when unavailable");
        assert_eq!(args_value_after(&args, "-cpu"), Some("qemu64".to_string()), "must not use -cpu host without KVM");
    }

    #[test]
    fn iso_attached_boots_from_cdrom_first() {
        let mut vm = sample_vm();
        vm.iso_path = Some("/isos/debian.iso".to_string());
        let args = build_qemu_args(&vm, "/tmp/mon.sock");
        assert_eq!(args_value_after(&args, "-cdrom"), Some("/isos/debian.iso".to_string()));
        assert_eq!(args_value_after(&args, "-boot"), Some("d".to_string()));
    }

    #[test]
    fn no_iso_boots_from_disk() {
        let args = build_qemu_args(&sample_vm(), "/tmp/mon.sock");
        assert!(!args.contains(&"-cdrom".to_string()));
        assert_eq!(args_value_after(&args, "-boot"), Some("c".to_string()));
    }

    #[test]
    fn disk_path_uses_virtio_and_qcow2_format() {
        let args = build_qemu_args(&sample_vm(), "/tmp/mon.sock");
        let drive = args_value_after(&args, "-drive").unwrap();
        assert!(drive.contains("format=qcow2"));
        assert!(drive.contains("if=virtio"));
        assert!(drive.contains(&sample_vm().disk_path));
    }

    #[test]
    fn monitor_socket_path_is_included_for_graceful_shutdown_support() {
        let args = build_qemu_args(&sample_vm(), "/run/user/1000/blue-virt/vm1-monitor.sock");
        let monitor = args_value_after(&args, "-monitor").unwrap();
        assert!(monitor.contains("/run/user/1000/blue-virt/vm1-monitor.sock"));
        assert!(monitor.starts_with("unix:"));
    }

    #[test]
    fn qemu_img_create_args_are_well_formed() {
        let args = build_qemu_img_create_args("/vms/disk.qcow2", 60);
        assert_eq!(args, vec!["create", "-f", "qcow2", "/vms/disk.qcow2", "60G"]);
    }

    /// Small test helper: returns the value immediately following a
    /// given flag in a flat args list (`["-m", "4096", ...]` ->
    /// `args_value_after(&args, "-m") == Some("4096")`).
    fn args_value_after(args: &[String], flag: &str) -> Option<String> {
        args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
    }
}
