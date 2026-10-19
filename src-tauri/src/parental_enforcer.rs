use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const CONFIG_DIR: &str = "/etc/blue-environment/parental";
pub const STATE_DIR: &str = "/var/lib/blue-environment/parental";
pub const UNIT_PATH: &str = "/etc/systemd/system/blue-parental.service";
pub const UNIT_NAME: &str = "blue-parental.service";
const TICK: Duration = Duration::from_secs(15);
const MIN_UID: u32 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppRule {
    pub app_id: String,
    /// `exe` (match the program's executable name) or `cmdline` (substring of
    /// the command line — used for Flatpak app ids).
    pub kind: String,
    pub value: String,
    pub blocked: bool,
    pub daily_limit_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemPolicy {
    pub version: u32,
    pub username: String,
    pub uid: u32,
    pub enabled: bool,
    /// Argon2 PHC string of the parental PIN (needed to change / remove the policy).
    pub pin_hash: String,
    pub rules: Vec<AppRule>,
    pub allowed_start: Option<String>,
    pub allowed_end: Option<String>,
    pub total_daily_limit_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub date: String,
    pub total_seconds: u64,
    pub per_app: HashMap<String, u64>,
    #[serde(default)]
    pub warned: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub exe_base: String,
    pub comm: String,
    pub cmdline: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    OutsideHours,
    TotalLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decision {
    pub terminate_session: Option<Reason>,
    /// Indexes into `policy.rules` whose running processes must be killed.
    pub kill_rules: Vec<usize>,
}

// ── Pure logic ──────────────────────────────────────────────────────────

/// `"08:30"` → minutes since midnight.
pub fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    if h < 24 && m < 60 { Some(h * 60 + m) } else { None }
}

/// Is `minute` inside the allowed window? Handles windows that cross midnight
/// (`22:00`–`06:00`). No window (or an unparsable one) means "always allowed".
pub fn in_window(start: &Option<String>, end: &Option<String>, minute: u32) -> bool {
    let (Some(s), Some(e)) = (start.as_deref().and_then(parse_hhmm), end.as_deref().and_then(parse_hhmm)) else { return true };
    if s == e {
        true
    } else if s < e {
        minute >= s && minute < e
    } else {
        minute >= s || minute < e
    }
}

/// Minutes until the account is cut off (allowed-hours end or daily limit),
/// `None` when there is no limit at all.
pub fn minutes_left(p: &SystemPolicy, minute: u32, u: &Usage) -> Option<u32> {
    let mut best: Option<u32> = None;
    let mut consider = |v: u32| best = Some(best.map_or(v, |b| b.min(v)));
    if let (Some(_), Some(e)) = (p.allowed_start.as_deref().and_then(parse_hhmm), p.allowed_end.as_deref().and_then(parse_hhmm)) {
        if in_window(&p.allowed_start, &p.allowed_end, minute) {
            consider((e + 1440 - minute) % 1440);
        }
    }
    if let Some(l) = p.total_daily_limit_minutes {
        consider((l as u64).saturating_sub(u.total_seconds / 60) as u32);
    }
    best
}

pub fn decide(p: &SystemPolicy, minute: u32, u: &Usage, running: &[bool]) -> Decision {
    let mut d = Decision::default();
    if !p.enabled {
        return d;
    }
    if !in_window(&p.allowed_start, &p.allowed_end, minute) {
        d.terminate_session = Some(Reason::OutsideHours);
    } else if p.total_daily_limit_minutes.map(|l| u.total_seconds >= l as u64 * 60).unwrap_or(false) {
        d.terminate_session = Some(Reason::TotalLimit);
    }
    for (i, r) in p.rules.iter().enumerate() {
        if !running.get(i).copied().unwrap_or(false) {
            continue;
        }
        let over = r.daily_limit_minutes.map(|l| u.per_app.get(&r.app_id).copied().unwrap_or(0) >= l as u64 * 60).unwrap_or(false);
        if r.blocked || over {
            d.kill_rules.push(i);
        }
    }
    d
}

/// Adds one tick of usage. Rolls over at midnight (new `date`).
pub fn account_tick(u: &mut Usage, today: &str, tick_secs: u64, session_active: bool, p: &SystemPolicy, running: &[bool]) {
    if u.date != today {
        *u = Usage { date: today.to_string(), ..Usage::default() };
    }
    if session_active {
        u.total_seconds += tick_secs;
    }
    for (i, r) in p.rules.iter().enumerate() {
        if running.get(i).copied().unwrap_or(false) && r.daily_limit_minutes.is_some() {
            *u.per_app.entry(r.app_id.clone()).or_insert(0) += tick_secs;
        }
    }
}

pub fn rule_matches(r: &AppRule, p: &ProcInfo) -> bool {
    if r.value.is_empty() {
        return false;
    }
    match r.kind.as_str() {
        "exe" => {
            p.exe_base == r.value || p.comm == r.value || (r.value.len() > 15 && p.comm == r.value[..15])
        }
        "cmdline" => p.cmdline.contains(&r.value),
        _ => false,
    }
}

/// Turns a desktop entry `Exec=` line into a matching rule.
pub fn exec_to_rule(app_id: &str, exec: &str, blocked: bool, limit: Option<u32>) -> Option<AppRule> {
    let toks: Vec<&str> = exec.split_whitespace().filter(|t| !t.starts_with('%')).collect();
    let mut i = 0;
    // `env FOO=bar prog` / `FOO=bar prog`
    while i < toks.len() && (toks[i] == "env" || (toks[i].contains('=') && !toks[i].starts_with('/') && !toks[i].starts_with('-'))) {
        i += 1;
    }
    let first = *toks.get(i)?;
    let base = first.trim_matches('"').rsplit('/').next().unwrap_or(first);
    let (kind, value) = if base == "flatpak" {
        let run = toks.iter().position(|t| *t == "run")?;
        let id = toks.iter().skip(run + 1).find(|t| !t.starts_with('-') && t.contains('.'))?;
        ("cmdline", id.to_string())
    } else if matches!(base, "sh" | "bash" | "dash" | "zsh") {
        return None; // wrapper script: can't tell what it starts
    } else {
        ("exe", base.to_string())
    };
    Some(AppRule { app_id: app_id.into(), kind: kind.into(), value, blocked, daily_limit_minutes: limit })
}

/// A systemd unit that runs the enforcer.
pub fn unit_text(exe: &str) -> String {
    format!(
        "[Unit]\nDescription=Blue Environment parental-controls enforcement\nAfter=systemd-logind.service\n\n[Service]\nType=simple\nExecStart={exe} --parental-enforce\nRestart=always\nRestartSec=5\n\n[Install]\nWantedBy=multi-user.target\n"
    )
}

/// The enforcer runs as root, so the binary it is started from must not be
/// replaceable by an unprivileged user.
pub fn exe_is_trustworthy(uid: u32, mode: u32) -> bool {
    uid == 0 && mode & 0o022 == 0
}

pub fn valid_target_uid(uid: u32) -> bool {
    (MIN_UID..60000).contains(&uid)
}

// ── Processes, sessions ─────────────────────────────────────────────────

pub fn scan_processes(uid: u32) -> Vec<ProcInfo> {
    let Ok(rd) = std::fs::read_dir("/proc") else { return vec![] };
    rd.filter_map(|e| e.ok())
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_string_lossy().parse().ok()?;
            let meta = std::fs::metadata(e.path()).ok()?;
            if meta.uid() != uid {
                return None;
            }
            let exe = std::fs::read_link(e.path().join("exe")).ok().map(|p| p.to_string_lossy().replace(" (deleted)", "")).unwrap_or_default();
            let comm = std::fs::read_to_string(e.path().join("comm")).unwrap_or_default().trim().to_string();
            let cmdline = std::fs::read(e.path().join("cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ")).unwrap_or_default();
            Some(ProcInfo { pid, exe_base: exe.rsplit('/').next().unwrap_or("").to_string(), comm, cmdline })
        })
        .collect()
}

fn kill_pids(pids: &[u32]) {
    if pids.is_empty() {
        return;
    }
    for p in pids {
        unsafe { libc::kill(*p as i32, libc::SIGTERM) };
    }
    std::thread::sleep(Duration::from_secs(3));
    for p in pids {
        if Path::new(&format!("/proc/{p}")).exists() {
            unsafe { libc::kill(*p as i32, libc::SIGKILL) };
        }
    }
}

fn session_state(uid: u32) -> String {
    Command::new("loginctl")
        .args(["show-user", &uid.to_string(), "-p", "State", "--value"])
        .stdin(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn notify(p: &SystemPolicy, text: &str) {
    let bus = format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{}/bus", p.uid);
    let _ = Command::new("runuser")
        .args(["-u", &p.username, "--", "env", &bus, "notify-send", "-u", "critical", "Blue Environment", text])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

// ── Files ───────────────────────────────────────────────────────────────

fn policy_path(uid: u32) -> PathBuf {
    Path::new(CONFIG_DIR).join(format!("{uid}.json"))
}
fn usage_path(uid: u32) -> PathBuf {
    Path::new(STATE_DIR).join(format!("{uid}.json"))
}

pub fn load_policy(path: &Path) -> Option<SystemPolicy> {
    let p: SystemPolicy = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    // Never act on system accounts, whatever a file claims.
    if valid_target_uid(p.uid) && !p.username.is_empty() { Some(p) } else { None }
}

fn all_policies() -> Vec<SystemPolicy> {
    std::fs::read_dir(CONFIG_DIR)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| load_policy(&e.path())).collect())
        .unwrap_or_default()
}

fn atomic_write(path: &Path, data: &str, mode: u32) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).map_err(|e| format!("Cannot write {}: {e}", tmp.display()))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

// ── The service ─────────────────────────────────────────────────────────

/// `blue-environment --parental-enforce` (root, never returns).
pub fn run_daemon() -> i32 {
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("[blue-parental] must run as root");
        return 1;
    }
    let _ = std::fs::create_dir_all(STATE_DIR);
    eprintln!("[blue-parental] enforcing {} polic(ies)", all_policies().len());
    loop {
        for p in all_policies().iter().filter(|p| p.enabled) {
            if let Err(e) = enforce_once(p) {
                eprintln!("[blue-parental] {}: {e}", p.username);
            }
        }
        std::thread::sleep(TICK);
    }
}

fn enforce_once(p: &SystemPolicy) -> Result<(), String> {
    use chrono::Timelike;
    let now = chrono::Local::now();
    let minute = now.hour() * 60 + now.minute();
    let today = now.format("%Y-%m-%d").to_string();

    let mut usage: Usage = std::fs::read_to_string(usage_path(p.uid)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let procs = scan_processes(p.uid);
    let running: Vec<bool> = p.rules.iter().map(|r| procs.iter().any(|pr| rule_matches(r, pr))).collect();
    let active = session_state(p.uid) == "active";

    let was = usage.date.clone();
    account_tick(&mut usage, &today, TICK.as_secs(), active, p, &running);
    if was != today {
        usage.warned = false;
    }

    // Warn once, 5 minutes before the cut-off.
    if let Some(left) = minutes_left(p, minute, &usage) {
        if left <= 5 && left > 0 && !usage.warned && active {
            usage.warned = true;
            notify(p, &format!("Screen time ends in {left} minute(s). Please save your work."));
        }
    }

    let d = decide(p, minute, &usage, &running);
    let _ = std::fs::create_dir_all(STATE_DIR);
    let _ = atomic_write(&usage_path(p.uid), &serde_json::to_string(&usage).unwrap_or_default(), 0o600);

    for i in &d.kill_rules {
        let pids: Vec<u32> = procs.iter().filter(|pr| rule_matches(&p.rules[*i], pr)).map(|pr| pr.pid).collect();
        kill_pids(&pids);
    }
    if d.terminate_session.is_some() && matches!(session_state(p.uid).as_str(), "active" | "online" | "lingering") {
        let _ = Command::new("loginctl").args(["terminate-user", &p.uid.to_string()]).status();
    }
    Ok(())
}

// ── Privileged helper (`--parental-priv`) ───────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperRequest {
    op: String,
    pin: String,
    #[serde(default)]
    policy: Option<SystemPolicy>,
    /// Only honoured when started directly as root (no pkexec / sudo).
    #[serde(default)]
    uid: Option<u32>,
}

fn out(v: serde_json::Value) {
    use std::io::Write;
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{}{}", crate::privileged::LINE_PREFIX, v);
    let _ = o.flush();
}
fn fail(msg: &str) -> i32 {
    out(serde_json::json!({"event": "error", "message": msg}));
    1
}

fn invoking_uid(req: &HelperRequest) -> Option<u32> {
    for var in ["PKEXEC_UID", "SUDO_UID"] {
        if let Some(u) = std::env::var(var).ok().and_then(|v| v.parse().ok()) {
            return Some(u);
        }
    }
    req.uid
}

fn username_of(uid: u32) -> Option<String> {
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() { None } else { Some(std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().to_string()) }
    }
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl").args(args).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

pub fn run_priv_helper() -> i32 {
    let mut input = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).is_err() {
        return fail("Could not read the request");
    }
    let req: HelperRequest = match serde_json::from_str(input.trim()) {
        Ok(r) => r,
        Err(e) => return fail(&format!("Malformed request: {e}")),
    };
    let Some(uid) = invoking_uid(&req) else { return fail("Cannot tell which account this is for") };
    if !valid_target_uid(uid) {
        return fail("Only regular user accounts can be restricted");
    }
    let Some(username) = username_of(uid) else { return fail("Unknown account") };
    let path = policy_path(uid);

    // An existing enabled policy may only be changed with its PIN.
    if let Some(existing) = load_policy(&path) {
        if existing.enabled && !crate::parental_controls::verify_pin_hash_public(&req.pin, &existing.pin_hash) {
            return fail("Wrong parental PIN");
        }
    }

    match req.op.as_str() {
        "enable" => {
            let Some(mut policy) = req.policy else { return fail("No policy supplied") };
            if !crate::parental_controls::verify_pin_hash_public(&req.pin, &policy.pin_hash) {
                return fail("Wrong parental PIN");
            }
            let exe = match std::env::current_exe() {
                Ok(e) => e,
                Err(e) => return fail(&format!("Cannot locate the program: {e}")),
            };
            let meta = match std::fs::metadata(&exe) {
                Ok(m) => m,
                Err(e) => return fail(&format!("Cannot inspect the program: {e}")),
            };
            if !exe_is_trustworthy(meta.uid(), meta.mode()) {
                return fail("Blue Environment must be installed in a system location (e.g. /usr/bin) owned by root before system-level enforcement can be enabled");
            }
            policy.uid = uid;
            policy.username = username;
            policy.version = 1;
            policy.enabled = true;
            if let Err(e) = std::fs::create_dir_all(CONFIG_DIR).and_then(|_| std::fs::set_permissions(CONFIG_DIR, std::fs::Permissions::from_mode(0o755))) {
                return fail(&format!("Cannot create {CONFIG_DIR}: {e}"));
            }
            if let Err(e) = atomic_write(&path, &serde_json::to_string_pretty(&policy).unwrap_or_default(), 0o644) {
                return fail(&e);
            }
            if let Err(e) = atomic_write(Path::new(UNIT_PATH), &unit_text(&exe.to_string_lossy()), 0o644) {
                return fail(&e);
            }
            systemctl(&["daemon-reload"]);
            if !systemctl(&["enable", "--now", UNIT_NAME]) || !systemctl(&["restart", UNIT_NAME]) {
                return fail("Could not start the enforcement service (systemctl failed)");
            }
        }
        "disable" => {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(usage_path(uid));
            if all_policies().is_empty() {
                systemctl(&["disable", "--now", UNIT_NAME]);
                let _ = std::fs::remove_file(UNIT_PATH);
                systemctl(&["daemon-reload"]);
            }
        }
        _ => return fail("Unknown operation"),
    }
    out(serde_json::json!({"event": "done"}));
    0
}

// ── Commands used by Settings → Parental Controls ───────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    /// The binary is installed where root can trust it.
    pub supported: bool,
    pub enforced: bool,
    pub service_active: bool,
    pub username: String,
    pub rule_count: usize,
}

#[tauri::command(async)]
pub fn parental_system_status() -> SystemStatus {
    let uid = unsafe { libc::getuid() };
    let policy = load_policy(&policy_path(uid));
    let supported = std::env::current_exe()
        .ok()
        .and_then(|e| std::fs::metadata(e).ok())
        .map(|m| exe_is_trustworthy(m.uid(), m.mode()))
        .unwrap_or(false);
    SystemStatus {
        supported,
        enforced: policy.as_ref().map(|p| p.enabled).unwrap_or(false),
        service_active: systemctl(&["is-active", "--quiet", UNIT_NAME]),
        username: username_of(uid).unwrap_or_default(),
        rule_count: policy.map(|p| p.rules.len()).unwrap_or(0),
    }
}

fn run_helper(req: serde_json::Value) -> Result<(), String> {
    let mut result: Option<Result<(), String>> = None;
    let outcome = crate::privileged::run_privileged(
        &["--parental-priv"],
        &req.to_string(),
        |l| {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(l) else { return };
            match v.get("event").and_then(|e| e.as_str()) {
                Some("done") => result = Some(Ok(())),
                Some("error") => result = Some(Err(v.get("message").and_then(|m| m.as_str()).unwrap_or("Failed").to_string())),
                _ => {}
            }
        },
        |_| {},
    )
    .map_err(|e| match e.hint {
        Some(h) => format!("{} — {h}", e.message),
        None => e.message,
    })?;
    match result {
        Some(r) => r,
        None => {
            let info = crate::privileged::explain_launch_failure(&outcome);
            Err(info.message)
        }
    }
}

/// Copies the (PIN-verified) in-shell policy to the system and starts the service.
#[tauri::command(async)]
pub fn parental_system_enable(pin: String) -> Result<(), String> {
    let cfg = crate::parental_controls::verified_config(&pin).ok_or("Wrong PIN, or too many attempts")?;
    let apps = crate::apps::scan_desktop_apps(false);
    let mut rules: Vec<AppRule> = Vec::new();
    let mut ids: Vec<&String> = cfg.blocked_apps.iter().chain(cfg.daily_limits_minutes.keys()).collect();
    ids.sort();
    ids.dedup();
    for id in ids {
        let Some(app) = apps.iter().find(|a| &a.id == id) else { continue };
        let blocked = cfg.blocked_apps.contains(id);
        let limit = cfg.daily_limits_minutes.get(id).copied();
        if let Some(r) = exec_to_rule(id, &app.exec, blocked, limit) {
            rules.push(r);
        }
    }
    let policy = SystemPolicy {
        version: 1,
        username: String::new(),
        uid: 0,
        enabled: cfg.enabled,
        pin_hash: cfg.pin_hash.clone().unwrap_or_default(),
        rules,
        allowed_start: cfg.allowed_hours_start.clone(),
        allowed_end: cfg.allowed_hours_end.clone(),
        total_daily_limit_minutes: cfg.daily_total_limit_minutes,
    };
    run_helper(serde_json::json!({"op": "enable", "pin": pin, "policy": policy}))
}

#[tauri::command(async)]
pub fn parental_system_disable(pin: String) -> Result<(), String> {
    run_helper(serde_json::json!({"op": "disable", "pin": pin}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pol() -> SystemPolicy {
        SystemPolicy {
            version: 1,
            username: "kid".into(),
            uid: 1001,
            enabled: true,
            pin_hash: "x".into(),
            rules: vec![
                AppRule { app_id: "game.desktop".into(), kind: "exe".into(), value: "supertux2".into(), blocked: false, daily_limit_minutes: Some(30) },
                AppRule { app_id: "web.desktop".into(), kind: "exe".into(), value: "firefox".into(), blocked: true, daily_limit_minutes: None },
            ],
            allowed_start: Some("08:00".into()),
            allowed_end: Some("20:00".into()),
            total_daily_limit_minutes: Some(120),
        }
    }

    #[test]
    fn hhmm_and_windows() {
        assert_eq!(parse_hhmm("08:30"), Some(510));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("x"), None);
        let (s, e) = (Some("08:00".to_string()), Some("20:00".to_string()));
        assert!(in_window(&s, &e, 8 * 60) && in_window(&s, &e, 19 * 60 + 59));
        assert!(!in_window(&s, &e, 7 * 60 + 59) && !in_window(&s, &e, 20 * 60));
        // across midnight
        let (s, e) = (Some("22:00".to_string()), Some("06:00".to_string()));
        assert!(in_window(&s, &e, 23 * 60) && in_window(&s, &e, 5 * 60) && !in_window(&s, &e, 12 * 60));
        assert!(in_window(&None, &None, 3), "no window = always allowed");
        assert!(in_window(&Some("bad".into()), &e, 3), "garbage fails open (never locks everyone out)");
    }

    #[test]
    fn decisions() {
        let p = pol();
        let u = Usage::default();
        assert_eq!(decide(&p, 12 * 60, &u, &[false, false]), Decision::default());
        // outside hours → session terminated
        assert_eq!(decide(&p, 21 * 60, &u, &[false, false]).terminate_session, Some(Reason::OutsideHours));
        // blocked app running is killed, limited app below limit is not
        let d = decide(&p, 12 * 60, &u, &[true, true]);
        assert_eq!(d.kill_rules, vec![1]);
        // limit reached
        let mut u2 = Usage::default();
        u2.per_app.insert("game.desktop".into(), 30 * 60);
        assert_eq!(decide(&p, 12 * 60, &u2, &[true, false]).kill_rules, vec![0]);
        // total limit
        let u3 = Usage { total_seconds: 120 * 60, ..Usage::default() };
        assert_eq!(decide(&p, 12 * 60, &u3, &[false, false]).terminate_session, Some(Reason::TotalLimit));
        // disabled policy does nothing
        let mut off = pol();
        off.enabled = false;
        assert_eq!(decide(&off, 23 * 60, &u3, &[true, true]), Decision::default());
    }

    #[test]
    fn ticks_and_midnight_rollover() {
        let p = pol();
        let mut u = Usage { date: "2026-10-15".into(), total_seconds: 999, ..Usage::default() };
        account_tick(&mut u, "2026-10-16", 15, true, &p, &[true, false]);
        assert_eq!(u.date, "2026-10-16");
        assert_eq!(u.total_seconds, 15, "new day starts from zero");
        assert_eq!(u.per_app.get("game.desktop"), Some(&15));
        account_tick(&mut u, "2026-10-16", 15, false, &p, &[false, true]);
        assert_eq!(u.total_seconds, 15, "idle session does not count");
        assert!(!u.per_app.contains_key("web.desktop"), "apps without a limit are not tracked");
    }

    #[test]
    fn time_left() {
        let p = pol();
        assert_eq!(minutes_left(&p, 19 * 60 + 30, &Usage::default()), Some(30));
        let u = Usage { total_seconds: 115 * 60, ..Usage::default() };
        assert_eq!(minutes_left(&p, 10 * 60, &u), Some(5));
        let mut free = pol();
        free.allowed_start = None;
        free.total_daily_limit_minutes = None;
        assert_eq!(minutes_left(&free, 10, &Usage::default()), None);
    }

    #[test]
    fn exec_lines_to_rules() {
        let r = exec_to_rule("a", "/usr/bin/firefox %u", true, None).unwrap();
        assert_eq!((r.kind.as_str(), r.value.as_str()), ("exe", "firefox"));
        assert_eq!(exec_to_rule("a", "env MOZ_ENABLE_WAYLAND=1 firefox", true, None).unwrap().value, "firefox");
        let f = exec_to_rule("a", "flatpak run --branch=stable --arch=x86_64 org.mozilla.firefox @@u %U @@", true, None).unwrap();
        assert_eq!((f.kind.as_str(), f.value.as_str()), ("cmdline", "org.mozilla.firefox"));
        assert!(exec_to_rule("a", "sh -c 'steam'", true, None).is_none());
        assert!(exec_to_rule("a", "", true, None).is_none());
    }

    #[test]
    fn matching_and_safety() {
        let p = ProcInfo { pid: 5, exe_base: "firefox".into(), comm: "firefox".into(), cmdline: "/usr/lib/firefox/firefox".into() };
        let rule = |k: &str, v: &str| AppRule { kind: k.into(), value: v.into(), ..AppRule::default() };
        assert!(rule_matches(&rule("exe", "firefox"), &p));
        assert!(!rule_matches(&rule("exe", "chromium"), &p));
        assert!(rule_matches(&rule("cmdline", "lib/firefox"), &p));
        assert!(!rule_matches(&rule("exe", ""), &p), "empty rule matches nothing");
        // truncated comm (15 chars)
        let long = ProcInfo { pid: 6, exe_base: String::new(), comm: "very-long-progr".into(), cmdline: String::new() };
        assert!(rule_matches(&rule("exe", "very-long-program-name"), &long));
        assert!(!valid_target_uid(0) && !valid_target_uid(999) && valid_target_uid(1000) && !valid_target_uid(65534));
        assert!(exe_is_trustworthy(0, 0o755) && !exe_is_trustworthy(1000, 0o755) && !exe_is_trustworthy(0, 0o775));
        assert!(unit_text("/usr/bin/blue-environment").contains("ExecStart=/usr/bin/blue-environment --parental-enforce"));
    }

    #[test]
    fn scans_own_processes() {
        let me = unsafe { libc::getuid() };
        let procs = scan_processes(me);
        assert!(procs.iter().any(|p| p.pid == std::process::id()), "must see itself");
    }

    #[test]
    fn policy_loading_rejects_system_uids() {
        let dir = std::env::temp_dir().join(format!("blue-parental-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = pol();
        let f = dir.join("a.json");
        std::fs::write(&f, serde_json::to_string(&p).unwrap()).unwrap();
        assert!(load_policy(&f).is_some());
        p.uid = 0;
        std::fs::write(&f, serde_json::to_string(&p).unwrap()).unwrap();
        assert!(load_policy(&f).is_none(), "root must never be restricted");
        std::fs::write(&f, "garbage").unwrap();
        assert!(load_policy(&f).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
