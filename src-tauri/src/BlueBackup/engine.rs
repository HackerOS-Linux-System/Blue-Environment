use chrono::{Datelike, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub const ROOT_DIR: &str = "Blue-Backup";
const NAME_PREFIX: &str = "snap-";
const TS_FORMAT: &str = "%Y%m%d-%H%M%S";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfig {
    pub destination: String,
    pub sources: Vec<String>,
    pub excludes: Vec<String>,
    pub keep_daily: u32,
    pub keep_weekly: u32,
    pub keep_monthly: u32,
    /// `off` | `hourly` | `daily` | `weekly`
    pub schedule: String,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            destination: String::new(),
            sources: dirs::home_dir().map(|h| vec![h.to_string_lossy().to_string()]).unwrap_or_default(),
            excludes: vec![".cache".into(), "node_modules".into(), ".local/share/Trash".into(), "*.tmp".into(), ".thumbnails".into(), "target".into()],
            keep_daily: 7,
            keep_weekly: 4,
            keep_monthly: 6,
            schedule: "off".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Directory name, e.g. `snap-20261016-093000`.
    pub name: String,
    /// ISO 8601 local time.
    pub created: String,
    pub transferred_bytes: u64,
    pub warnings: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// `prepare` | `copy` | `prune` | `done`
    pub phase: String,
    pub pct: u8,
    pub bytes: u64,
}

// ── Config validation ───────────────────────────────────────────────────

pub fn validate(cfg: &BackupConfig) -> Result<(), String> {
    if cfg.destination.trim().is_empty() {
        return Err("Choose where to store the backups".into());
    }
    let dest = Path::new(&cfg.destination);
    if !dest.is_absolute() {
        return Err("The backup location must be an absolute path".into());
    }
    if cfg.sources.is_empty() {
        return Err("Choose at least one folder to back up".into());
    }
    for s in &cfg.sources {
        let p = Path::new(s);
        if !p.is_absolute() || s.contains("..") {
            return Err(format!("\"{s}\" is not a valid absolute folder"));
        }
        if p.starts_with(dest) || dest.starts_with(p) && p != Path::new("/") && dest.starts_with(p) {
            // Backing up the destination into itself would recurse forever.
            if dest.starts_with(p) {
                return Err(format!("The backup location is inside \"{s}\" — choose a location outside the folders you back up"));
            }
        }
    }
    if !["off", "hourly", "daily", "weekly"].contains(&cfg.schedule.as_str()) {
        return Err("Unknown schedule".into());
    }
    for e in &cfg.excludes {
        if e.contains('\n') || e.len() > 200 {
            return Err("Invalid exclude pattern".into());
        }
    }
    Ok(())
}

/// Filesystems that cannot hold hard links (so `--link-dest` would silently
/// copy everything every time).
pub fn fs_supports_hardlinks(fstype: &str) -> bool {
    !matches!(fstype.trim().to_lowercase().as_str(), "msdos" | "vfat" | "fat" | "exfat" | "ntfs" | "fuseblk" | "cifs" | "smb2" | "smbfs" | "9p")
}

fn fs_type(path: &Path) -> Option<String> {
    let o = Command::new("stat").args(["-f", "-c", "%T", "--"]).arg(path).output().ok()?;
    if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None }
}

// ── Names & retention ───────────────────────────────────────────────────

pub fn snapshot_name(t: NaiveDateTime) -> String {
    format!("{NAME_PREFIX}{}", t.format(TS_FORMAT))
}

pub fn parse_snapshot_name(name: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(name.strip_prefix(NAME_PREFIX)?, TS_FORMAT).ok()
}

/// Names that must be deleted to satisfy the policy. The newest snapshot is
/// never removed. Rule: keep the latest snapshot of each of the last
/// `daily` days, `weekly` ISO weeks and `monthly` months that have one.
pub fn select_prunable(names: &[String], daily: u32, weekly: u32, monthly: u32) -> Vec<String> {
    let mut snaps: Vec<(&String, NaiveDateTime)> = names.iter().filter_map(|n| parse_snapshot_name(n).map(|t| (n, t))).collect();
    snaps.sort_by(|a, b| b.1.cmp(&a.1)); // newest first
    let mut keep: HashSet<&String> = HashSet::new();
    if let Some((n, _)) = snaps.first() {
        keep.insert(n);
    }
    let (mut days, mut weeks, mut months): (HashSet<_>, HashSet<_>, HashSet<_>) = (HashSet::new(), HashSet::new(), HashSet::new());
    for (n, t) in &snaps {
        let d = t.date();
        if (days.len() as u32) < daily && days.insert(d) {
            keep.insert(n);
        }
        let w = (d.iso_week().year(), d.iso_week().week());
        if (weeks.len() as u32) < weekly && weeks.insert(w) {
            keep.insert(n);
        }
        let m = (d.year(), d.month());
        if (months.len() as u32) < monthly && months.insert(m) {
            keep.insert(n);
        }
    }
    snaps.iter().filter(|(n, _)| !keep.contains(n)).map(|(n, _)| (*n).clone()).collect()
}

// ── rsync ───────────────────────────────────────────────────────────────

pub fn rsync_args(cfg: &BackupConfig, previous: Option<&Path>, target: &Path) -> Vec<String> {
    let mut a: Vec<String> = vec!["-aHR".into(), "--info=progress2".into(), "--no-inc-recursive".into(), "--stats".into()];
    for e in &cfg.excludes {
        a.push(format!("--exclude={e}"));
    }
    // Never descend into the backup store itself.
    a.push(format!("--exclude=/{}", cfg.destination.trim_start_matches('/')));
    if let Some(p) = previous {
        a.push(format!("--link-dest={}", p.display()));
    }
    a.push("--".into());
    for s in &cfg.sources {
        a.push(s.clone());
    }
    let mut t = target.to_string_lossy().to_string();
    if !t.ends_with('/') {
        t.push('/');
    }
    a.push(t);
    a
}

/// `  1,234,567  45%   50.12MB/s    0:00:10 (xfr#1234, to-chk=100/5000)`
pub fn parse_progress(line: &str) -> Option<(u64, u8)> {
    let mut it = line.split_whitespace();
    let bytes: u64 = it.next()?.replace(',', "").parse().ok()?;
    let pct: u8 = it.next()?.strip_suffix('%')?.parse().ok()?;
    Some((bytes, pct.min(100)))
}

pub fn parse_transferred(stats: &str) -> u64 {
    stats
        .lines()
        .find_map(|l| l.trim().strip_prefix("Total transferred file size:"))
        .and_then(|r| r.split_whitespace().next())
        .and_then(|n| n.replace(',', "").parse().ok())
        .unwrap_or(0)
}

// ── Layout on disk ──────────────────────────────────────────────────────

pub fn store_dir(cfg: &BackupConfig) -> PathBuf {
    Path::new(&cfg.destination).join(ROOT_DIR)
}

pub fn list_snapshots(cfg: &BackupConfig) -> Vec<Snapshot> {
    let Ok(rd) = std::fs::read_dir(store_dir(cfg)) else { return vec![] };
    let mut v: Vec<Snapshot> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let t = parse_snapshot_name(&name)?;
            let m: Option<Snapshot> = std::fs::read_to_string(e.path().join("manifest.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
            Some(Snapshot {
                created: t.format("%Y-%m-%dT%H:%M:%S").to_string(),
                transferred_bytes: m.as_ref().map(|m| m.transferred_bytes).unwrap_or(0),
                warnings: m.map(|m| m.warnings).unwrap_or(0),
                name,
            })
        })
        .collect();
    v.sort_by(|a, b| b.name.cmp(&a.name));
    v
}

fn snapshot_path(cfg: &BackupConfig, name: &str) -> Result<PathBuf, String> {
    if parse_snapshot_name(name).is_none() {
        return Err("Invalid snapshot name".into());
    }
    Ok(store_dir(cfg).join(name))
}

/// Rejects absolute paths and `..` so a request can never leave the snapshot.
pub fn safe_relative(rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel.trim_start_matches('/'));
    if p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err("Invalid path".into());
    }
    Ok(p.to_path_buf())
}

fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

struct Lock(PathBuf);
impl Lock {
    fn acquire(dir: &Path) -> Result<Lock, String> {
        let p = dir.join(".lock");
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Ok(pid) = s.trim().parse::<u32>() {
                if pid_alive(pid) && pid != std::process::id() {
                    return Err("Another backup is already running".into());
                }
            }
        }
        std::fs::write(&p, std::process::id().to_string()).map_err(|e| format!("Cannot lock the backup store: {e}"))?;
        Ok(Lock(p))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Runs one backup and applies the retention policy. `on_progress` is called
/// from this thread. Returns the new snapshot.
pub fn run_backup(cfg: &BackupConfig, cancel: &AtomicBool, on_progress: &mut dyn FnMut(Progress)) -> Result<Snapshot, String> {
    validate(cfg)?;
    let dest = Path::new(&cfg.destination);
    if !dest.is_dir() {
        return Err(format!("The backup location {} is not available — is the drive connected?", dest.display()));
    }
    if let Some(t) = fs_type(dest) {
        if !fs_supports_hardlinks(&t) {
            return Err(format!("The backup location uses {t}, which cannot store incremental snapshots. Use a drive formatted as ext4, btrfs or xfs."));
        }
    }
    let store = store_dir(cfg);
    std::fs::create_dir_all(&store).map_err(|e| format!("Cannot create {}: {e}", store.display()))?;
    let _lock = Lock::acquire(&store)?;
    on_progress(Progress { phase: "prepare".into(), pct: 0, bytes: 0 });

    // Leftovers from an interrupted run.
    if let Ok(rd) = std::fs::read_dir(&store) {
        for e in rd.filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().starts_with(".partial-") {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let previous = list_snapshots(cfg).first().map(|s| store.join(&s.name));
    let now = chrono::Local::now().naive_local();
    let name = snapshot_name(now);
    let partial = store.join(format!(".partial-{}", &name[NAME_PREFIX.len()..]));
    std::fs::create_dir_all(&partial).map_err(|e| e.to_string())?;

    let args = rsync_args(cfg, previous.as_deref(), &partial);
    let mut child = Command::new("rsync")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let _ = std::fs::remove_dir_all(&partial);
            format!("Cannot run rsync — install the `rsync` package ({e})")
        })?;
    let mut out = child.stdout.take().ok_or("no stdout")?;
    let mut err = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_string(&mut s);
        }
        s
    });

    let (mut buf, mut all) = (Vec::new(), String::new());
    let mut chunk = [0u8; 4096];
    let mut last_pct = 255u8;
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&partial);
            return Err("Cancelled".into());
        }
        let n = out.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        // progress2 separates updates with '\r'
        while let Some(pos) = buf.iter().position(|b| *b == b'\r' || *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&line).to_string();
            if let Some((bytes, pct)) = parse_progress(&text) {
                if pct != last_pct {
                    last_pct = pct;
                    on_progress(Progress { phase: "copy".into(), pct, bytes });
                }
            } else {
                all.push_str(&text);
            }
        }
    }
    all.push_str(&String::from_utf8_lossy(&buf));
    let status = child.wait().map_err(|e| e.to_string())?;
    let stderr_text = err_thread.join().unwrap_or_default();
    let code = status.code().unwrap_or(-1);
    // 23 = some files could not be read, 24 = files vanished while copying: still a usable snapshot.
    let warnings = if code == 23 || code == 24 { stderr_text.lines().filter(|l| !l.trim().is_empty()).count() as u32 } else { 0 };
    if !(code == 0 || code == 23 || code == 24) {
        let _ = std::fs::remove_dir_all(&partial);
        let first = stderr_text.lines().find(|l| !l.trim().is_empty()).unwrap_or("rsync failed");
        return Err(format!("Backup failed: {first}"));
    }

    let snap = Snapshot {
        name: name.clone(),
        created: now.format("%Y-%m-%dT%H:%M:%S").to_string(),
        transferred_bytes: parse_transferred(&all),
        warnings,
    };
    let _ = std::fs::write(partial.join("manifest.json"), serde_json::to_string_pretty(&snap).unwrap_or_default());
    std::fs::rename(&partial, store.join(&name)).map_err(|e| format!("Cannot finish the snapshot: {e}"))?;

    on_progress(Progress { phase: "prune".into(), pct: 100, bytes: snap.transferred_bytes });
    let names: Vec<String> = list_snapshots(cfg).into_iter().map(|s| s.name).collect();
    for old in select_prunable(&names, cfg.keep_daily, cfg.keep_weekly, cfg.keep_monthly) {
        let _ = delete_snapshot(cfg, &old);
    }
    on_progress(Progress { phase: "done".into(), pct: 100, bytes: snap.transferred_bytes });
    Ok(snap)
}

pub fn delete_snapshot(cfg: &BackupConfig, name: &str) -> Result<(), String> {
    let p = snapshot_path(cfg, name)?;
    if std::fs::remove_dir_all(&p).is_ok() {
        return Ok(());
    }
    // Directories copied read-only can't be removed until they're writable.
    let _ = Command::new("chmod").args(["-R", "u+w", "--"]).arg(&p).status();
    std::fs::remove_dir_all(&p).map_err(|e| format!("Cannot delete {name}: {e}"))
}

pub fn browse(cfg: &BackupConfig, snapshot: &str, rel: &str) -> Result<Vec<Entry>, String> {
    let dir = snapshot_path(cfg, snapshot)?.join(safe_relative(rel)?);
    let mut v: Vec<Entry> = std::fs::read_dir(&dir)
        .map_err(|e| format!("Cannot open {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != "manifest.json" || !rel.trim_matches('/').is_empty() && true)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if rel.trim_matches('/').is_empty() && name == "manifest.json" {
                return None;
            }
            let m = e.path().symlink_metadata().ok()?;
            Some(Entry {
                name,
                is_dir: m.is_dir(),
                size: if m.is_dir() { 0 } else { m.len() },
                modified: std::os::unix::fs::MetadataExt::mtime(&m),
            })
        })
        .collect();
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(v)
}

/// Copies `rel` out of a snapshot. By default into
/// `~/Restored from backup/<snapshot>/`; with `in_place` back to its original
/// location, keeping any file it replaces as `<name>.pre-restore`.
pub fn restore(cfg: &BackupConfig, snapshot: &str, rel: &str, in_place: bool) -> Result<String, String> {
    let rel_p = safe_relative(rel)?;
    if rel_p.as_os_str().is_empty() {
        return Err("Choose a file or folder to restore".into());
    }
    let src = snapshot_path(cfg, snapshot)?.join(&rel_p);
    if src.symlink_metadata().is_err() {
        return Err("That item does not exist in the snapshot".into());
    }
    let (dest_dir, extra): (PathBuf, Vec<&str>) = if in_place {
        (Path::new("/").join(rel_p.parent().unwrap_or(Path::new(""))), vec!["--backup", "--suffix=.pre-restore"])
    } else {
        let home = dirs::home_dir().ok_or("No home directory")?;
        (home.join("Restored from backup").join(snapshot), vec![])
    };
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("Cannot create {}: {e}", dest_dir.display()))?;
    let mut dest = dest_dir.to_string_lossy().to_string();
    if !dest.ends_with('/') {
        dest.push('/');
    }
    let o = Command::new("rsync").arg("-aH").args(&extra).arg("--").arg(&src).arg(&dest).output().map_err(|e| format!("Cannot run rsync: {e}"))?;
    if !o.status.success() {
        return Err(format!("Restore failed: {}", String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("")));
    }
    Ok(dest)
}

// ── Config file & schedule ──────────────────────────────────────────────

pub fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Blue-Environment").join("backup.json")
}

pub fn load_config() -> BackupConfig {
    std::fs::read_to_string(config_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_config(cfg: &BackupConfig) -> Result<(), String> {
    let p = config_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub fn timer_unit(schedule: &str) -> Option<String> {
    let cal = match schedule {
        "hourly" => "hourly",
        "daily" => "daily",
        "weekly" => "weekly",
        _ => return None,
    };
    Some(format!(
        "[Unit]\nDescription=Blue Backup schedule\n\n[Timer]\nOnCalendar={cal}\nPersistent=true\nRandomizedDelaySec=300\n\n[Install]\nWantedBy=timers.target\n"
    ))
}

pub fn service_unit(exe: &str) -> String {
    format!("[Unit]\nDescription=Blue Backup\nConditionACPower=true\n\n[Service]\nType=oneshot\nNice=10\nIOSchedulingClass=idle\nExecStart={} --backup-run\n", systemd_quote(exe))
}

fn systemd_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c)) { s.to_string() } else { format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")) }
}

/// Installs / removes the per-user systemd timer.
pub fn apply_schedule(schedule: &str) -> Result<(), String> {
    let dir = dirs::config_dir().ok_or("No config directory")?.join("systemd").join("user");
    let (svc, tmr) = (dir.join("blue-backup.service"), dir.join("blue-backup.timer"));
    let systemctl = |args: &[&str]| Command::new("systemctl").arg("--user").args(args).stdout(Stdio::null()).stderr(Stdio::null()).status();
    match timer_unit(schedule) {
        None => {
            let _ = systemctl(&["disable", "--now", "blue-backup.timer"]);
            let _ = std::fs::remove_file(&tmr);
            let _ = std::fs::remove_file(&svc);
            let _ = systemctl(&["daemon-reload"]);
            Ok(())
        }
        Some(timer) => {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            std::fs::write(&svc, service_unit(&exe.to_string_lossy())).map_err(|e| e.to_string())?;
            std::fs::write(&tmr, timer).map_err(|e| e.to_string())?;
            let _ = systemctl(&["daemon-reload"]);
            match systemctl(&["enable", "--now", "blue-backup.timer"]) {
                Ok(s) if s.success() => Ok(()),
                _ => Err("Could not enable the systemd user timer (is systemd --user running?)".into()),
            }
        }
    }
}

/// `blue-environment --backup-run` — what the timer starts.
pub fn run_cli() -> i32 {
    let cfg = load_config();
    let cancel = AtomicBool::new(false);
    let mut last = 255u8;
    match run_backup(&cfg, &cancel, &mut |p| {
        if p.pct != last || p.phase == "done" {
            last = p.pct;
            println!("[blue-backup] {} {}%", p.phase, p.pct);
        }
    }) {
        Ok(s) => {
            println!("[blue-backup] created {}", s.name);
            0
        }
        Err(e) => {
            eprintln!("[blue-backup] {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(y: i32, m: u32, d: u32, h: u32) -> String {
        snapshot_name(chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, 0, 0).unwrap())
    }

    #[test]
    fn names_round_trip() {
        let name = n(2026, 10, 16, 9);
        assert_eq!(name, "snap-20261016-090000");
        assert_eq!(parse_snapshot_name(&name).unwrap().format("%H").to_string(), "09");
        assert!(parse_snapshot_name("snap-xx").is_none());
        assert!(parse_snapshot_name("../etc").is_none());
    }

    #[test]
    fn retention_keeps_newest_and_one_per_period() {
        let mut all = vec![];
        for d in 1..=20 {
            all.push(n(2026, 9, d, 8));
            all.push(n(2026, 9, d, 20)); // two per day
        }
        let prune = select_prunable(&all, 3, 0, 0);
        let kept: Vec<_> = all.iter().filter(|x| !prune.contains(x)).collect();
        // newest snapshot of the last 3 days
        assert_eq!(kept.len(), 3);
        assert!(kept.contains(&&n(2026, 9, 20, 20)));
        assert!(kept.contains(&&n(2026, 9, 19, 20)));
        assert!(kept.contains(&&n(2026, 9, 18, 20)));
    }

    #[test]
    fn retention_never_deletes_the_newest_even_with_zero_policy() {
        let all = vec![n(2026, 1, 1, 1), n(2026, 1, 2, 1)];
        let prune = select_prunable(&all, 0, 0, 0);
        assert_eq!(prune, vec![n(2026, 1, 1, 1)]);
    }

    #[test]
    fn retention_weekly_and_monthly_reach_back() {
        let all = vec![n(2026, 1, 15, 1), n(2026, 2, 15, 1), n(2026, 3, 15, 1), n(2026, 3, 16, 1)];
        let prune = select_prunable(&all, 1, 0, 3);
        // latest snapshot of each month is kept: Mar 16 (newest), Feb 15, Jan 15 — Mar 15 is redundant
        assert_eq!(prune, vec![n(2026, 3, 15, 1)]);
        let prune2 = select_prunable(&all, 1, 0, 1);
        assert_eq!(prune2.len(), 3);
    }

    #[test]
    fn rsync_args_shape() {
        let cfg = BackupConfig { destination: "/mnt/b".into(), sources: vec!["/home/u/Docs".into()], excludes: vec!["*.tmp".into()], ..BackupConfig::default() };
        let a = rsync_args(&cfg, Some(Path::new("/mnt/b/Blue-Backup/snap-1")), Path::new("/mnt/b/Blue-Backup/.partial-2"));
        assert_eq!(a[0], "-aHR");
        assert!(a.contains(&"--exclude=*.tmp".to_string()));
        assert!(a.contains(&"--link-dest=/mnt/b/Blue-Backup/snap-1".to_string()));
        let dd = a.iter().position(|x| x == "--").unwrap();
        assert_eq!(a[dd + 1], "/home/u/Docs");
        assert_eq!(a.last().unwrap(), "/mnt/b/Blue-Backup/.partial-2/");
    }

    #[test]
    fn progress_lines() {
        assert_eq!(parse_progress("  1,234,567  45%   50.12MB/s    0:00:10 (xfr#1234, to-chk=100/5000)"), Some((1_234_567, 45)));
        assert_eq!(parse_progress("sending incremental file list"), None);
        assert_eq!(parse_transferred("Total transferred file size: 12,345 bytes\n"), 12345);
    }

    #[test]
    fn path_safety() {
        assert!(safe_relative("home/u/a.txt").is_ok());
        assert!(safe_relative("/home/u").is_ok());
        assert!(safe_relative("../x").is_err());
        assert!(safe_relative("a/../../x").is_err());
    }

    #[test]
    fn validation() {
        let ok = BackupConfig { destination: "/mnt/b".into(), sources: vec!["/home/u".into()], ..BackupConfig::default() };
        assert!(validate(&ok).is_ok());
        let inside = BackupConfig { destination: "/home/u/backups".into(), ..ok.clone() };
        assert!(validate(&inside).is_err(), "destination inside a source");
        assert!(validate(&BackupConfig { destination: String::new(), ..ok.clone() }).is_err());
        assert!(validate(&BackupConfig { sources: vec![], ..ok.clone() }).is_err());
        assert!(validate(&BackupConfig { sources: vec!["rel/path".into()], ..ok.clone() }).is_err());
        assert!(validate(&BackupConfig { schedule: "yearly".into(), ..ok }).is_err());
    }

    #[test]
    fn filesystems_and_units() {
        assert!(fs_supports_hardlinks("ext2/ext3"));
        assert!(fs_supports_hardlinks("btrfs"));
        assert!(!fs_supports_hardlinks("msdos"));
        assert!(!fs_supports_hardlinks("exfat"));
        assert!(timer_unit("off").is_none());
        assert!(timer_unit("daily").unwrap().contains("OnCalendar=daily"));
        assert!(service_unit("/usr/bin/blue-environment").contains("ExecStart=/usr/bin/blue-environment --backup-run"));
        assert!(service_unit("/opt/my dir/blue").contains("\"/opt/my dir/blue\""));
    }

    /// End-to-end with the real rsync: two runs, the second must hard-link
    /// unchanged files and restore must work.
    #[test]
    fn real_rsync_incremental_round_trip() {
        if Command::new("rsync").arg("--version").output().is_err() {
            eprintln!("rsync not installed — skipping");
            return;
        }
        let base = std::env::temp_dir().join(format!("blue-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (src, dst) = (base.join("src"), base.join("dst"));
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(src.join("a.txt"), "alpha").unwrap();
        std::fs::write(src.join("sub/b.txt"), "beta").unwrap();
        std::fs::write(src.join("skip.tmp"), "x").unwrap();
        let cfg = BackupConfig { destination: dst.to_string_lossy().to_string(), sources: vec![src.to_string_lossy().to_string()], excludes: vec!["*.tmp".into()], ..BackupConfig::default() };
        let cancel = AtomicBool::new(false);
        let s1 = run_backup(&cfg, &cancel, &mut |_| {}).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100)); // distinct snapshot name
        std::fs::write(src.join("a.txt"), "alpha2").unwrap();
        let s2 = run_backup(&cfg, &cancel, &mut |_| {}).unwrap();
        assert_ne!(s1.name, s2.name);
        let snaps = list_snapshots(&cfg);
        assert_eq!(snaps.len(), 2);
        let rel_src = src.to_string_lossy().trim_start_matches('/').to_string();
        let entries = browse(&cfg, &s2.name, &rel_src).unwrap();
        assert!(entries.iter().any(|e| e.name == "a.txt") && !entries.iter().any(|e| e.name == "skip.tmp"));
        use std::os::unix::fs::MetadataExt;
        let b1 = std::fs::metadata(store_dir(&cfg).join(&s1.name).join(&rel_src).join("sub/b.txt")).unwrap();
        let b2 = std::fs::metadata(store_dir(&cfg).join(&s2.name).join(&rel_src).join("sub/b.txt")).unwrap();
        assert_eq!(b1.ino(), b2.ino(), "unchanged file must be hard-linked");
        let a1 = std::fs::read_to_string(store_dir(&cfg).join(&s1.name).join(&rel_src).join("a.txt")).unwrap();
        assert_eq!(a1, "alpha", "old snapshot keeps the old content");
        // in-place restore of the old version
        std::fs::write(src.join("a.txt"), "changed again").unwrap();
        restore(&cfg, &s1.name, &format!("{rel_src}/a.txt"), true).unwrap();
        assert_eq!(std::fs::read_to_string(src.join("a.txt")).unwrap(), "alpha");
        assert!(src.join("a.txt.pre-restore").exists());
        delete_snapshot(&cfg, &s1.name).unwrap();
        assert_eq!(list_snapshots(&cfg).len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }
}
