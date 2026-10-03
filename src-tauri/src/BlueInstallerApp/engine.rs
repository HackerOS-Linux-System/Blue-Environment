use super::error::{InstallError, InstallResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const TARGET_ROOT: &str = "/mnt/blue-install";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionPlanEntry {
    pub id: String,
    pub role: String,       // esp | root | home | swap | other
    pub filesystem: String, // fat32 | ext4 | btrfs | xfs | swap
    pub mountpoint: String,
    pub size_mib: Option<u64>, // None = remaining space
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallConfig {
    pub disk: String,
    pub confirm_erase: String,
    pub disk_mode: String, // erase | manual
    pub partitions: Vec<PartitionPlanEntry>,
    pub locale: String,
    pub keyboard_layout: String,
    pub timezone: String,
    pub hostname: String,
    pub username: String,
    pub full_name: String,
    #[serde(default)]
    pub password: String,
    pub auto_login: bool,
    pub user_dirs: UserDirNames,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDirNames {
    pub desktop: String,
    pub documents: String,
    pub downloads: String,
    pub music: String,
    pub pictures: String,
    pub videos: String,
    pub templates: String,
    pub public: String,
}

pub type Progress<'a> = dyn Fn(u8, &str) + Send + Sync + 'a;

struct PlannedPartition {
    entry: PartitionPlanEntry,
    /// 1-based partition number as created on the disk.
    number: u32,
    device: String,
}

fn run(cmd: &mut Command) -> InstallResult<String> {
    let out = cmd.output().map_err(|e| InstallError::io(&format!("running {:?}", cmd.get_program()), e))?;
    if !out.status.success() {
        return Err(InstallError::command_failed(cmd, &out));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn require_tool(bin: &str, install_hint: &str) -> InstallResult<()> {
    let found = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false);
    if found {
        Ok(())
    } else {
        Err(InstallError::new("missing_tool", format!("Required tool \"{bin}\" was not found on this live system"))
            .hint(format!("Install it in the live session first: {install_hint}")))
    }
}

fn is_efi_boot() -> bool {
    Path::new("/sys/firmware/efi").is_dir()
}

fn default_partition_plan() -> Vec<PartitionPlanEntry> {
    vec![
        PartitionPlanEntry { id: "p-esp".into(), role: "esp".into(), filesystem: "fat32".into(), mountpoint: "/boot/efi".into(), size_mib: Some(512) },
        PartitionPlanEntry { id: "p-root".into(), role: "root".into(), filesystem: "ext4".into(), mountpoint: "/".into(), size_mib: None },
    ]
}

/// Validates the target disk is a real, whole disk (not a partition, not
/// already mounted) and that `confirm_erase` matches — the same
/// belt-and-braces check the old script description promised, kept here.
fn validate_target(cfg: &InstallConfig) -> InstallResult<()> {
    if cfg.disk != cfg.confirm_erase {
        return Err(InstallError::new("confirm_mismatch", "The confirmation disk path does not match the selected disk")
            .hint("This is a safety check — nothing was touched."));
    }
    let path = Path::new(&cfg.disk);
    if !path.exists() {
        return Err(InstallError::new("disk_not_found", format!("Disk \"{}\" does not exist", cfg.disk)));
    }
    // A partition (e.g. /dev/sdb1) is never a valid install target — only whole disks.
    let name = cfg.disk.rsplit('/').next().unwrap_or("");
    if Path::new(&format!("/sys/class/block/{name}/partition")).exists() {
        return Err(InstallError::new("not_a_disk", format!("\"{}\" is a partition, not a whole disk", cfg.disk))
            .hint("Pick the whole disk (for example /dev/sdb, not /dev/sdb1)."));
    }
    // NOTE: a mounted target disk is no longer an error here. Desktop
    // auto-mounters (udisks etc.) routinely mount every partition of a
    // plugged-in disk, and asking the person to unmount things by hand is
    // not acceptable for an installer — `release_disk` below does it for
    // them. It only refuses when the disk is the live boot medium itself.
    if cfg.username.trim().is_empty() {
        return Err(InstallError::new("bad_config", "Username is empty"));
    }
    if !cfg.username.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_') || !cfg.username.as_bytes()[0].is_ascii_lowercase() {
        return Err(InstallError::new("bad_config", "Username must start with a lowercase letter and contain only lowercase letters, digits, - or _"));
    }
    if cfg.password.is_empty() {
        return Err(InstallError::new("bad_config", "Password is empty"));
    }
    if cfg.hostname.trim().is_empty() {
        return Err(InstallError::new("bad_config", "Hostname is empty"));
    }
    Ok(())
}

/// Partition device path for partition number `n` on `disk` — handles the
/// nvme/mmcblk "p" infix (`/dev/nvme0n1p1`) vs plain (`/dev/sda1`).
fn part_device(disk: &str, n: u32) -> String {
    let last = disk.rsplit('/').next().unwrap_or(disk);
    if last.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("{disk}p{n}")
    } else {
        format!("{disk}{n}")
    }
}

// ── Releasing the target disk ─────────────────────────────────────────────
//
// Previously the installer simply failed with
//   "/dev/sdb" (or a partition on it) is currently mounted
//   Unmount it first, or pick a different disk.
// whenever any partition of the chosen disk was mounted — which happens
// all the time (the desktop auto-mounts USB/SATA disks, a previous failed
// install may have left /mnt/blue-install mounted, swap may be active).
// Now the installer unmounts/deactivates everything itself and only
// refuses for the one case that truly cannot work: the disk we are
// running the live system from.

/// True when `dev` is `disk` itself or one of its partitions
/// (`/dev/sdb` → `/dev/sdb1`; `/dev/nvme0n1` → `/dev/nvme0n1p2`).
/// A plain prefix match is wrong: `/dev/sda` is a prefix of `/dev/sdaa`,
/// and `/dev/nvme0n1` of `/dev/nvme0n10`.
fn is_on_disk(dev: &str, disk: &str) -> bool {
    if dev == disk {
        return true;
    }
    let Some(rest) = dev.strip_prefix(disk) else { return false };
    let disk_ends_with_digit = disk.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false);
    let digits = if disk_ends_with_digit { rest.strip_prefix('p') } else { Some(rest.strip_prefix('p').unwrap_or(rest)) };
    matches!(digits, Some(d) if !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// `/proc/mounts` escapes space, tab, newline and backslash as octal (`\040`).
fn unescape_mount_field(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 3 < b.len() && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c)) {
            let v = (b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0');
            out.push(v);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Parses `/proc/mounts` content into `(device, mountpoint)` pairs. Device
/// paths that are symlinks (`/dev/disk/by-uuid/…`, `/dev/mapper/…`) are
/// resolved so they can be compared against `/dev/sdXN`.
fn parse_mounts(content: &str) -> Vec<(String, String)> {
    content
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let dev = unescape_mount_field(it.next()?);
            let mp = unescape_mount_field(it.next()?);
            let dev = if dev.starts_with("/dev/") {
                fs::canonicalize(&dev).map(|p| p.to_string_lossy().to_string()).unwrap_or(dev)
            } else {
                dev
            };
            Some((dev, mp))
        })
        .collect()
}

/// Mount points that mean "this disk holds the running live system" — it
/// cannot be unmounted, and wiping it would pull the rug from under the
/// installer itself.
fn is_live_boot_mountpoint(mp: &str) -> bool {
    matches!(mp, "/" | "/usr" | "/boot" | "/cdrom")
        || ["/run/live", "/lib/live", "/run/archiso", "/run/initramfs/live", "/usr/lib/live", "/cdrom/"]
            .iter()
            .any(|p| mp.starts_with(p))
}

/// Block devices stacked on top of `dev` (LUKS / LVM / md), found through
/// sysfs `holders`, so e.g. `/dev/mapper/vg-root` on `/dev/sdb2` is
/// released too.
fn holders_of(dev: &str) -> Vec<String> {
    let name = dev.rsplit('/').next().unwrap_or("");
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(format!("/sys/class/block/{name}/holders")) {
        for e in rd.flatten() {
            let h = e.file_name().to_string_lossy().to_string();
            out.extend(holders_of(&format!("/dev/{h}")));
            out.push(format!("/dev/{h}"));
        }
    }
    out
}

fn partitions_of(disk: &str) -> Vec<String> {
    let name = disk.rsplit('/').next().unwrap_or("");
    let mut out = vec![disk.to_string()];
    if let Ok(rd) = fs::read_dir(format!("/sys/class/block/{name}")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with(name) && Path::new(&format!("/sys/class/block/{name}/{n}/partition")).exists() {
                out.push(format!("/dev/{n}"));
            }
        }
    }
    out
}

/// Everything (devices) that belongs to the disk: itself, its partitions
/// and anything stacked on them.
fn disk_device_set(disk: &str) -> Vec<String> {
    let mut all = Vec::new();
    for p in partitions_of(disk) {
        for h in holders_of(&p) {
            if !all.contains(&h) {
                all.push(h);
            }
        }
        if !all.contains(&p) {
            all.push(p);
        }
    }
    all
}

/// Unmounts every filesystem and deactivates all swap that lives on `disk`
/// (or on devices stacked on it), and clears a stale `/mnt/blue-install`
/// from an earlier attempt. Returns an error only when that is impossible:
/// the disk carries the live system, or something refuses to let go.
pub fn release_disk(disk: &str, progress: &Progress, first_pass: bool) -> InstallResult<()> {
    if first_pass {
        progress(1, "Releasing the target disk (unmounting)…");
    }

    // 1) A previous (failed/cancelled) run may have left /mnt/blue-install
    //    and its bind mounts behind.
    let stale_prefix = TARGET_ROOT.to_string();
    let mut stale: Vec<String> = parse_mounts(&fs::read_to_string("/proc/mounts").unwrap_or_default())
        .into_iter()
        .map(|(_, mp)| mp)
        .filter(|mp| *mp == stale_prefix || mp.starts_with(&format!("{stale_prefix}/")))
        .collect();
    stale.sort_by_key(|m| std::cmp::Reverse(m.len()));
    for mp in stale {
        let _ = Command::new("umount").arg("-l").arg(&mp).output();
    }

    // 2) Several passes: unmounting can reveal another mount underneath, and
    //    auto-mounters occasionally re-mount right after udev events.
    for pass in 0..4 {
        let devices = disk_device_set(disk);
        let on_disk = |dev: &str| devices.iter().any(|d| d == dev) || is_on_disk(dev, disk);

        let mounts = parse_mounts(&fs::read_to_string("/proc/mounts").unwrap_or_default());
        let mut mine: Vec<(String, String)> = mounts.into_iter().filter(|(d, _)| on_disk(d.as_str())).collect();

        if let Some((dev, mp)) = mine.iter().find(|(_, mp)| is_live_boot_mountpoint(mp)) {
            return Err(InstallError::new(
                "disk_is_live_medium",
                format!("\"{disk}\" is the disk this live system is running from ({dev} is mounted at {mp})"),
            )
            .hint("Choose a different disk. If you want to install onto this very disk, boot the live image from another USB stick or DVD."));
        }

        let swaps: Vec<String> = fs::read_to_string("/proc/swaps")
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
            .map(|d| fs::canonicalize(&d).map(|p| p.to_string_lossy().to_string()).unwrap_or(d))
            .filter(|d| on_disk(d.as_str()))
            .collect();

        if mine.is_empty() && swaps.is_empty() {
            break;
        }
        if pass == 3 {
            let left: Vec<String> = mine.iter().map(|(d, m)| format!("{d} → {m}")).chain(swaps.iter().map(|s| format!("{s} (swap)"))).collect();
            return Err(InstallError::new("disk_busy", format!("Could not release \"{disk}\" — something is still using it"))
                .hint("Close any program that has files open on this disk and try again, or pick a different disk.")
                .detail(left.join("\n")));
        }

        for s in &swaps {
            let _ = Command::new("swapoff").arg(s).output();
        }
        // Deepest mount point first, so /mnt/x/sub goes before /mnt/x.
        mine.sort_by_key(|(_, mp)| std::cmp::Reverse(mp.matches('/').count()));
        for (_, mp) in &mine {
            let plain = Command::new("umount").arg(mp).output();
            let ok = plain.map(|o| o.status.success()).unwrap_or(false);
            if !ok {
                // Busy (a file manager window, a terminal cd'd into it…):
                // detach lazily — the filesystem disappears from the tree
                // immediately and the kernel finishes the unmount when the
                // last user lets go. Nothing here may destroy user data;
                // the disk is about to be wiped on explicit confirmation.
                let _ = Command::new("umount").arg("-l").arg(mp).output();
            }
        }
        let _ = Command::new("udevadm").arg("settle").output();
        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    // 3) Stacked devices (LUKS/LVM/md) on the disk: close them so the
    //    kernel drops its hold, best effort.
    for h in disk_device_set(disk).into_iter().filter(|d| d.starts_with("/dev/dm-") || d.starts_with("/dev/md")) {
        if h.starts_with("/dev/dm-") {
            let _ = Command::new("dmsetup").args(["remove", "--force", "--retry"]).arg(&h).output();
        } else {
            let _ = Command::new("mdadm").args(["--stop"]).arg(&h).output();
        }
    }

    // 4) Drop old signatures so a former filesystem / RAID / LVM label on
    //    the disk can't be re-detected and auto-mounted mid-install.
    //    ONLY on the first pass — the second pass runs after the new GPT
    //    has been written and must never wipe it.
    if first_pass {
        let _ = Command::new("wipefs").args(["--all", "--force"]).arg(disk).output();
    }
    let _ = Command::new("blockdev").arg("--flushbufs").arg(disk).output();
    let _ = Command::new("udevadm").arg("settle").output();
    Ok(())
}

fn partition_disk(cfg: &InstallConfig, progress: &Progress) -> InstallResult<Vec<PlannedPartition>> {
    progress(2, "Partitioning disk…");
    let plan = if cfg.disk_mode == "manual" && !cfg.partitions.is_empty() { cfg.partitions.clone() } else { default_partition_plan() };
    if !plan.iter().any(|p| p.role == "esp") {
        return Err(InstallError::new("bad_partition_plan", "The partition plan has no EFI System Partition"));
    }
    if !plan.iter().any(|p| p.mountpoint == "/") {
        return Err(InstallError::new("bad_partition_plan", "The partition plan has no partition mounted at /"));
    }
    if plan.iter().filter(|p| p.size_mib.is_none()).count() > 1 {
        return Err(InstallError::new("bad_partition_plan", "Only one partition can use the remaining space"));
    }

    run(Command::new("parted").arg("-s").arg(&cfg.disk).args(["mklabel", "gpt"]))?;

    let mut start_mib: u64 = 1; // leave 1 MiB for GPT alignment
    let mut planned = Vec::new();
    for (i, entry) in plan.iter().enumerate() {
        let end = match entry.size_mib {
            Some(sz) => format!("{}MiB", start_mib + sz),
            None => "100%".to_string(),
        };
        let fs_type = match entry.filesystem.as_str() {
            "fat32" => "fat32",
            "ext4" => "ext4",
            "btrfs" => "btrfs",
            "xfs" => "xfs",
            "swap" => "linux-swap",
            other => return Err(InstallError::new("bad_partition_plan", format!("Unknown filesystem \"{other}\""))),
        };
        run(Command::new("parted").arg("-s").arg(&cfg.disk).args(["mkpart", "primary", fs_type, &format!("{start_mib}MiB"), &end]))?;
        let number = (i + 1) as u32;
        if entry.role == "esp" {
            run(Command::new("parted").arg("-s").arg(&cfg.disk).args(["set", &number.to_string(), "esp", "on"]))?;
        }
        planned.push(PlannedPartition { entry: entry.clone(), number, device: part_device(&cfg.disk, number) });
        start_mib = match entry.size_mib {
            Some(sz) => start_mib + sz,
            None => start_mib, // last one, no next
        };
    }
    let _ = run(Command::new("partprobe").arg(&cfg.disk));
    std::thread::sleep(std::time::Duration::from_millis(800));
    for p in &planned {
        for _ in 0..20 {
            if Path::new(&p.device).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if !Path::new(&p.device).exists() {
            return Err(InstallError::new("partition_missing", format!("Partition {} did not appear after partitioning", p.device)));
        }
    }
    Ok(planned)
}

/// Destroys any leftover filesystem/RAID/LVM signature inside a freshly
/// created partition.
///
/// This is what fixed
///   mount: /mnt/blue-install: fsconfig() failed: Can't find a SQUASHFS superblock on sda2
/// The new GPT's partition 2 landed on top of an old disk's data (typically
/// a previously written live image, which starts with a squashfs magic at
/// offset 0). `mkfs.ext4` only rewrites its own structures, so the stale
/// squashfs magic survived, libblkid saw TWO filesystems, and a plain `mount`
/// guessed the wrong one. Wiping signatures + zeroing the head of the
/// partition makes the result unambiguous (and `mount` below also passes an
/// explicit `-t`).
fn scrub_partition(device: &str) {
    let _ = Command::new("wipefs").args(["--all", "--force"]).arg(device).output();
    if let Ok(mut f) = fs::OpenOptions::new().write(true).open(device) {
        use std::io::Write;
        let zeros = vec![0u8; 1024 * 1024];
        for _ in 0..4 {
            if f.write_all(&zeros).is_err() {
                break;
            }
        }
        let _ = f.sync_all();
    }
}

/// The kernel filesystem type name `mount -t` needs for a plan entry.
fn mount_fstype(filesystem: &str) -> &'static str {
    match filesystem {
        "fat32" => "vfat",
        "btrfs" => "btrfs",
        "xfs" => "xfs",
        _ => "ext4",
    }
}

fn format_partitions(planned: &[PlannedPartition], progress: &Progress) -> InstallResult<()> {
    progress(15, "Formatting partitions…");
    for p in planned {
        scrub_partition(&p.device);
    }
    let _ = Command::new("udevadm").arg("settle").output();
    for p in planned {
        match p.entry.filesystem.as_str() {
            "fat32" => run(Command::new("mkfs.fat").args(["-F", "32", "-n", "EFI"]).arg(&p.device))?,
            "ext4" => run(Command::new("mkfs.ext4").args(["-F", "-L"]).arg(label_for(&p.entry.role)).arg(&p.device))?,
            "btrfs" => run(Command::new("mkfs.btrfs").args(["-f", "-L"]).arg(label_for(&p.entry.role)).arg(&p.device))?,
            "xfs" => run(Command::new("mkfs.xfs").args(["-f", "-L"]).arg(label_for(&p.entry.role)).arg(&p.device))?,
            "swap" => run(Command::new("mkswap").arg(&p.device))?,
            other => return Err(InstallError::new("bad_partition_plan", format!("Unknown filesystem \"{other}\""))),
        };
    }
    let _ = Command::new("udevadm").arg("settle").output();
    Ok(())
}

fn label_for(role: &str) -> &'static str {
    match role {
        "root" => "BLUEROOT",
        "home" => "BLUEHOME",
        _ => "BLUEDATA",
    }
}

fn mount_target(planned: &[PlannedPartition], progress: &Progress) -> InstallResult<()> {
    progress(25, "Mounting target filesystem…");
    let target = PathBuf::from(TARGET_ROOT);
    fs::create_dir_all(&target).map_err(|e| InstallError::io("creating the mount point", e))?;

    // Mount / first, then everything else, shallowest mountpoint first, so
    // e.g. /boot/efi mounts onto an already-mounted root.
    let mut sorted: Vec<&PlannedPartition> = planned.iter().filter(|p| p.entry.filesystem != "swap").collect();
    sorted.sort_by_key(|p| if p.entry.mountpoint == "/" { 0 } else { p.entry.mountpoint.matches('/').count() });

    for p in &sorted {
        let mp = target.join(p.entry.mountpoint.trim_start_matches('/'));
        fs::create_dir_all(&mp).map_err(|e| InstallError::io(&format!("creating {}", mp.display()), e))?;
        run(Command::new("mount").arg("-t").arg(mount_fstype(&p.entry.filesystem)).arg(&p.device).arg(&mp))?;
    }
    for p in planned {
        if p.entry.filesystem == "swap" {
            let _ = run(Command::new("swapon").arg(&p.device));
        }
    }
    Ok(())
}

/// Copies the running live system into the target — the same technique
/// used by other live-installer projects (MX Linux's minstall, antiX,
/// Refracta): the live root (`/`) already IS the fully assembled system
/// (live-boot's squashfs + overlay merged), so an rsync of `/` with the
/// pseudo-filesystems, the boot medium and live-only state excluded
/// reproduces it on disk without needing a second separate rootfs image.
fn copy_system(progress: &Progress) -> InstallResult<()> {
    progress(30, "Copying system files…");
    require_tool("rsync", "apt install rsync")?;
    let excludes = [
        "/proc/*", "/sys/*", "/dev/*", "/run/*", "/tmp/*",
        "/mnt/*", "/media/*", "/lost+found",
        "/var/lib/live/*", "/lib/live/*", "/var/cache/apt/archives/*.deb",
        "/swapfile",
        // the live user's home is recreated fresh for the new account below
        "/home/*",
        "/root/.cache/*",
    ];
    let mut cmd = Command::new("rsync");
    cmd.args(["-aHAX", "--info=progress2", "--numeric-ids"]);
    for e in excludes {
        cmd.arg(format!("--exclude={e}"));
    }
    cmd.arg("/").arg(format!("{TARGET_ROOT}/"));
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| InstallError::io("starting rsync", e))?;
    // rsync's --info=progress2 prints a single updating line; just drain it
    // (a nicer live percentage isn't worth parsing rsync's transfer-rate
    // format here) and report coarse progress based on elapsed time instead.
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = [0u8; 4096];
            let mut r = stdout;
            loop {
                match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        });
    }
    let status = child.wait().map_err(|e| InstallError::io("waiting for rsync", e))?;
    if !status.success() {
        return Err(InstallError::new("copy_failed", "Copying the system to disk failed (rsync exited with an error)"));
    }
    for dir in ["proc", "sys", "dev", "run", "tmp", "mnt", "media"] {
        let _ = fs::create_dir_all(format!("{TARGET_ROOT}/{dir}"));
    }
    Ok(())
}

fn write_fstab(planned: &[PlannedPartition]) -> InstallResult<()> {
    let mut out = String::from("# /etc/fstab — generated by Blue Installer\n# <device>\t<mount>\t<type>\t<options>\t<dump>\t<pass>\n");
    for p in planned {
        let uuid_out = Command::new("blkid").args(["-s", "UUID", "-o", "value"]).arg(&p.device).output();
        let uuid = uuid_out.ok().and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None });
        let Some(uuid) = uuid else { continue };
        let (fstype, opts, pass) = match p.entry.filesystem.as_str() {
            "fat32" => ("vfat", "umask=0077", 2),
            "ext4" => ("ext4", "defaults", if p.entry.mountpoint == "/" { 1 } else { 2 }),
            "btrfs" => ("btrfs", "defaults", 0),
            "xfs" => ("xfs", "defaults", 0),
            "swap" => ("swap", "sw", 0),
            _ => continue,
        };
        let mp = if p.entry.filesystem == "swap" { "none".to_string() } else { p.entry.mountpoint.clone() };
        out.push_str(&format!("UUID={uuid}\t{mp}\t{fstype}\t{opts}\t0\t{pass}\n"));
    }
    fs::write(format!("{TARGET_ROOT}/etc/fstab"), out).map_err(|e| InstallError::io("writing fstab", e))
}

fn bind_mount_chroot_dirs() -> InstallResult<()> {
    for (src, dst, is_proc) in [("/dev", "dev", false), ("/dev/pts", "dev/pts", false), ("/proc", "proc", true), ("/sys", "sys", false), ("/run", "run", false)] {
        let target = format!("{TARGET_ROOT}/{dst}");
        fs::create_dir_all(&target).ok();
        let mut cmd = Command::new("mount");
        if is_proc {
            cmd.args(["-t", "proc", "proc", &target]);
        } else {
            cmd.arg("--bind").arg(src).arg(&target);
        }
        run(&mut cmd)?;
    }
    Ok(())
}

fn unmount_all(planned: &[PlannedPartition]) {
    for d in ["run", "sys", "dev/pts", "dev", "proc"] {
        let _ = Command::new("umount").arg("-lf").arg(format!("{TARGET_ROOT}/{d}")).status();
    }
    let mut mps: Vec<&str> = planned.iter().filter(|p| p.entry.filesystem != "swap").map(|p| p.entry.mountpoint.as_str()).collect();
    mps.sort_by_key(|m| std::cmp::Reverse(m.matches('/').count()));
    for mp in mps {
        let full = if mp == "/" { TARGET_ROOT.to_string() } else { format!("{TARGET_ROOT}{mp}") };
        let _ = Command::new("umount").arg("-lf").arg(&full).status();
    }
    for p in planned {
        if p.entry.filesystem == "swap" {
            let _ = Command::new("swapoff").arg(&p.device).status();
        }
    }
    let _ = Command::new("umount").arg("-lf").arg(TARGET_ROOT).status();
}

/// Runs `sh -c script` inside the target root via `chroot`.
fn chroot_sh(script: &str) -> InstallResult<String> {
    let mut cmd = Command::new("chroot");
    cmd.arg(TARGET_ROOT).arg("/bin/sh").arg("-c").arg(script);
    run(&mut cmd)
}

fn configure_system(cfg: &InstallConfig, progress: &Progress) -> InstallResult<()> {
    progress(70, "Configuring the new system…");

    fs::write(format!("{TARGET_ROOT}/etc/hostname"), format!("{}\n", cfg.hostname)).map_err(|e| InstallError::io("writing hostname", e))?;
    let hosts = format!("127.0.0.1\tlocalhost\n127.0.1.1\t{h}\n\n::1\tlocalhost ip6-localhost ip6-loopback\nff02::1\tip6-allnodes\nff02::2\tip6-allrouters\n", h = cfg.hostname);
    fs::write(format!("{TARGET_ROOT}/etc/hosts"), hosts).map_err(|e| InstallError::io("writing hosts", e))?;

    // Locale.
    fs::create_dir_all(format!("{TARGET_ROOT}/etc/default")).ok();
    fs::write(format!("{TARGET_ROOT}/etc/default/locale"), format!("LANG={}\n", cfg.locale)).map_err(|e| InstallError::io("writing locale", e))?;
    if Path::new(&format!("{TARGET_ROOT}/etc/locale.gen")).exists() {
        let _ = chroot_sh(&format!("sed -i 's/^# *{loc}/{loc}/' /etc/locale.gen && locale-gen", loc = shell_escape_regex(&cfg.locale)));
    }

    // Keyboard.
    let kb = format!("XKBMODEL=\"pc105\"\nXKBLAYOUT=\"{}\"\nXKBVARIANT=\"\"\nXKBOPTIONS=\"\"\n\nBACKSPACE=\"guess\"\n", cfg.keyboard_layout);
    fs::write(format!("{TARGET_ROOT}/etc/default/keyboard"), kb).map_err(|e| InstallError::io("writing keyboard config", e))?;

    // Timezone.
    let tz_path = format!("/usr/share/zoneinfo/{}", cfg.timezone);
    if Path::new(&format!("{TARGET_ROOT}{tz_path}")).exists() {
        let _ = fs::remove_file(format!("{TARGET_ROOT}/etc/localtime"));
        let _ = std::os::unix::fs::symlink(&tz_path, format!("{TARGET_ROOT}/etc/localtime"));
        fs::write(format!("{TARGET_ROOT}/etc/timezone"), format!("{}\n", cfg.timezone)).ok();
    }

    // Machine id — the live image's must not be reused verbatim on every install.
    let _ = fs::remove_file(format!("{TARGET_ROOT}/etc/machine-id"));
    let _ = chroot_sh("systemd-machine-id-setup 2>/dev/null || dbus-uuidgen --ensure=/etc/machine-id 2>/dev/null || true");

    progress(78, "Removing the live session account…");
    remove_live_account(cfg)?;

    progress(82, "Creating your user account…");
    create_user_account(cfg)?;

    Ok(())
}

fn shell_escape_regex(s: &str) -> String {
    s.replace('.', "\\.").replace('/', "\\/")
}

/// Same intent as HackerOS's own `remove-live-user.sh` (used by
/// Calamares-based editions) — kept as an independent, self-contained
/// implementation here since Blue Environment is a separate project and
/// must not depend on a file from another repository at build/run time.
/// This is exactly the fix for: "after installing I end up with two user
/// accounts — the live one and the one I just created".
fn remove_live_account(cfg: &InstallConfig) -> InstallResult<()> {
    let live_user = "user";
    if live_user == cfg.username {
        // The person chose the same name as the live account — nothing to
        // remove, create_user_account below will just take it over cleanly
        // (delete-then-recreate) so its home/permissions end up correct.
        let _ = chroot_sh(&format!("userdel -rf {live_user} 2>/dev/null; true"));
        return Ok(());
    }
    let _ = chroot_sh(&format!("rm -f /etc/sudoers.d/live-user; pkill -u {live_user} 2>/dev/null; userdel -rf {live_user} 2>/dev/null; groupdel {live_user} 2>/dev/null; true"));
    for f in ["/usr/lib/sddm/sddm.conf.d/autologin.conf", "/etc/sddm.conf.d/autologin.conf", "/etc/sddm.conf"] {
        let full = format!("{TARGET_ROOT}{f}");
        if let Ok(content) = fs::read_to_string(&full) {
            if content.contains(&format!("User={live_user}")) {
                let _ = fs::remove_file(&full);
            }
        }
    }
    let _ = fs::remove_file(format!("{TARGET_ROOT}/etc/skel/.config/Blue-Environment/.live"));
    Ok(())
}

fn create_user_account(cfg: &InstallConfig) -> InstallResult<()> {
    let quoted_user = shell_quote(&cfg.username);
    let create = format!(
        "useradd -m -s /bin/bash -c {full} -G sudo,audio,video,plugdev,netdev,input,render {user}",
        full = shell_quote(&cfg.full_name),
        user = quoted_user,
    );
    chroot_sh(&create).map_err(|e| e.hint("Creating the user account failed — see detail for the exact error."))?;

    // Password over chroot's stdin via chpasswd, never as a chroot argument.
    let mut cmd = Command::new("chroot");
    cmd.arg(TARGET_ROOT).arg("chpasswd");
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| InstallError::io("running chpasswd", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "{}:{}", cfg.username, cfg.password);
    }
    let out = child.wait_with_output().map_err(|e| InstallError::io("waiting for chpasswd", e))?;
    if !out.status.success() {
        return Err(InstallError::new("useradd_failed", "Could not set the account password").detail(String::from_utf8_lossy(&out.stderr).to_string()));
    }

    // Localized XDG user directories.
    let home = format!("/home/{}", cfg.username);
    for name in [&cfg.user_dirs.desktop, &cfg.user_dirs.documents, &cfg.user_dirs.downloads, &cfg.user_dirs.music, &cfg.user_dirs.pictures, &cfg.user_dirs.videos, &cfg.user_dirs.templates, &cfg.user_dirs.public] {
        let _ = chroot_sh(&format!("mkdir -p {}/{} && chown {}:{} {}/{}", shell_quote(&home), shell_quote(name), quoted_user, quoted_user, shell_quote(&home), shell_quote(name)));
    }
    let user_dirs_conf = format!(
        "XDG_DESKTOP_DIR=\"$HOME/{}\"\nXDG_DOCUMENTS_DIR=\"$HOME/{}\"\nXDG_DOWNLOAD_DIR=\"$HOME/{}\"\nXDG_MUSIC_DIR=\"$HOME/{}\"\nXDG_PICTURES_DIR=\"$HOME/{}\"\nXDG_VIDEOS_DIR=\"$HOME/{}\"\nXDG_TEMPLATES_DIR=\"$HOME/{}\"\nXDG_PUBLICSHARE_DIR=\"$HOME/{}\"\n",
        cfg.user_dirs.desktop, cfg.user_dirs.documents, cfg.user_dirs.downloads, cfg.user_dirs.music,
        cfg.user_dirs.pictures, cfg.user_dirs.videos, cfg.user_dirs.templates, cfg.user_dirs.public,
    );
    let config_dir = format!("{TARGET_ROOT}{home}/.config");
    fs::create_dir_all(&config_dir).ok();
    fs::write(format!("{config_dir}/user-dirs.dirs"), user_dirs_conf).ok();
    let _ = chroot_sh(&format!("chown -R {user}:{user} {home}", user = quoted_user, home = shell_quote(&home)));

    if cfg.auto_login {
        let conf = format!("[Autologin]\nUser={}\nSession=blue-environment.desktop\nRelogin=true\n\n[General]\nNumlock=none\n", cfg.username);
        let dir = format!("{TARGET_ROOT}/usr/lib/sddm/sddm.conf.d");
        if Path::new(&format!("{TARGET_ROOT}/usr/lib/sddm")).exists() {
            fs::create_dir_all(&dir).ok();
            fs::write(format!("{dir}/autologin.conf"), conf).ok();
        }
    }
    Ok(())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn install_bootloader(cfg: &InstallConfig, progress: &Progress) -> InstallResult<()> {
    progress(90, "Installing the bootloader…");
    if is_efi_boot() {
        let target = format!("{TARGET_ROOT}/boot/efi");
        if !Path::new(&target).exists() {
            return Err(InstallError::new("no_esp", "No EFI System Partition was mounted at /boot/efi"));
        }
        chroot_sh("grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=Blue --recheck")
            .map_err(|e| e.hint("EFI bootloader install failed. Common cause: the ESP is too small (needs ≥ 260 MiB) or isn't FAT32."))?;
    } else {
        chroot_sh(&format!("grub-install --target=i386-pc --recheck {}", cfg.disk))
            .map_err(|e| e.hint("BIOS/legacy bootloader install failed."))?;
    }
    let _ = chroot_sh("update-grub 2>&1 || grub-mkconfig -o /boot/grub/grub.cfg 2>&1");
    Ok(())
}

/// Runs the full install. Always attempts to unmount the target on the way
/// out, success or failure, so a failed install doesn't leave `/mnt/blue-
/// install` holding the disk busy for a retry.
pub fn run_install(cfg: &InstallConfig, progress: &Progress) -> InstallResult<()> {
    validate_target(cfg)?;
    for (bin, pkg) in [("parted", "parted"), ("mkfs.ext4", "e2fsprogs"), ("mkfs.fat", "dosfstools"), ("rsync", "rsync"), ("chroot", "coreutils")] {
        require_tool(bin, pkg)?;
    }

    release_disk(&cfg.disk, progress, true)?;
    let planned = partition_disk(cfg, progress)?;
    let result = (|| -> InstallResult<()> {
        // Right after partitioning, udev may let a desktop auto-mounter
        // grab a brand-new partition before mkfs runs — release again.
        release_disk(&cfg.disk, progress, false)?;
        format_partitions(&planned, progress)?;
        mount_target(&planned, progress)?;
        copy_system(progress)?;
        write_fstab(&planned)?;
        bind_mount_chroot_dirs()?;
        configure_system(cfg, progress)?;
        install_bootloader(cfg, progress)?;
        progress(99, "Finishing up…");
        Ok(())
    })();
    unmount_all(&planned);
    result?;
    progress(100, "Installation complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_on_disk_matches_only_real_partitions() {
        assert!(is_on_disk("/dev/sdb", "/dev/sdb"));
        assert!(is_on_disk("/dev/sdb1", "/dev/sdb"));
        assert!(is_on_disk("/dev/sdb12", "/dev/sdb"));
        assert!(!is_on_disk("/dev/sdba", "/dev/sdb")); // another disk
        assert!(!is_on_disk("/dev/sdb", "/dev/sda"));
        assert!(is_on_disk("/dev/nvme0n1p2", "/dev/nvme0n1"));
        assert!(!is_on_disk("/dev/nvme0n10", "/dev/nvme0n1")); // another namespace
        assert!(!is_on_disk("/dev/nvme0n10p1", "/dev/nvme0n1"));
        assert!(is_on_disk("/dev/mmcblk0p1", "/dev/mmcblk0"));
    }

    #[test]
    fn mount_fields_are_unescaped() {
        assert_eq!(unescape_mount_field("/media/My\\040Disk"), "/media/My Disk");
        assert_eq!(unescape_mount_field("/plain"), "/plain");
        assert_eq!(unescape_mount_field("/x\\134y"), "/x\\y");
    }

    #[test]
    fn live_boot_mountpoints_are_detected() {
        assert!(is_live_boot_mountpoint("/"));
        assert!(is_live_boot_mountpoint("/run/live/medium"));
        assert!(is_live_boot_mountpoint("/lib/live/mount/medium"));
        assert!(is_live_boot_mountpoint("/cdrom"));
        assert!(!is_live_boot_mountpoint("/media/user/DATA"));
        assert!(!is_live_boot_mountpoint("/run/media/user/USB"));
    }

    #[test]
    fn parse_mounts_reads_pairs() {
        let m = parse_mounts("/dev/sdb1 /media/a\\040b ext4 rw 0 0\ntmpfs /run tmpfs rw 0 0\n");
        assert_eq!(m[0].1, "/media/a b");
        assert_eq!(m[1], ("tmpfs".to_string(), "/run".to_string()));
    }

    #[test]
    fn mount_fstype_is_explicit() {
        assert_eq!(mount_fstype("fat32"), "vfat");
        assert_eq!(mount_fstype("ext4"), "ext4");
        assert_eq!(mount_fstype("btrfs"), "btrfs");
        assert_eq!(mount_fstype("xfs"), "xfs");
    }
}
