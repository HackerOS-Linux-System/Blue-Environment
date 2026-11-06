#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const MAX_CRASH_REPORTS: usize = 20;
pub const MAX_LOG_TAIL_BYTES: u64 = 512 * 1024;

// ── redaction ──────────────────────────────────────────────────────────────

struct Rule {
    re: Regex,
    to: &'static str,
}

fn rules() -> &'static Vec<Rule> {
    static R: OnceLock<Vec<Rule>> = OnceLock::new();
    R.get_or_init(|| {
        const SPEC: &[(&str, &str)] = &[
            // credentials inside URLs: scheme://user:pass@host
            (r"(?i)([a-z][a-z0-9+.-]*://)[^/\s:@]+:[^/\s@]+@", "${1}<redacted>@"),
            // bearer / basic authorisation
            (r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]{8,}", "${1} <redacted>"),
            // key=value / key: value secrets
            (r#"(?i)\b(password|passwd|pwd|token|secret|api[_-]?key|access[_-]?key|authorization|cookie|session[_-]?id|private[_-]?key)(["']?\s*[:=]\s*["']?)[^\s"',;&]+"#, "${1}${2}<redacted>"),
            // e-mail
            (r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+", "<email>"),
            // MAC
            (r"\b(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}\b", "<mac>"),
            // user names inside home paths
            (r"(/home/|/Users/|/var/home/)[^/\s:'\x22]+", "${1}<user>"),
            // long random-looking strings (keys, tokens, digests); no '/' so file paths survive
            (r"\b[A-Za-z0-9_+=-]{40,}\b", "<redacted-long-string>"),
        ];
        SPEC.iter().filter_map(|(p, t)| Regex::new(p).ok().map(|re| Rule { re, to: t })).collect()
    })
}

fn ipv4() -> Option<&'static Regex> {
    static R: OnceLock<Option<Regex>> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\b").ok()).as_ref()
}

/// Removes personal and secret data from free text.
pub fn redact(input: &str) -> String {
    redact_with(input, hostname().as_deref())
}

fn hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()).filter(|h| h.len() >= 3)
}

pub fn redact_with(input: &str, host: Option<&str>) -> String {
    let mut s = input.to_string();
    for r in rules() {
        s = r.re.replace_all(&s, r.to).into_owned();
    }
    if let Some(re) = ipv4() {
        s = re
            .replace_all(&s, |c: &regex::Captures| {
                let octets: Vec<u32> = (1..=4).filter_map(|i| c[i].parse().ok()).collect();
                let valid = octets.len() == 4 && octets.iter().all(|&o| o <= 255);
                let keep = matches!(c[0].as_ref(), "127.0.0.1" | "0.0.0.0" | "255.255.255.255");
                if valid && !keep { "<ip>".to_string() } else { c[0].to_string() }
            })
            .into_owned();
    }
    if let Some(h) = host {
        if let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(h))) {
            s = re.replace_all(&s, "<host>").into_owned();
        }
    }
    if let Some(home) = dirs::home_dir() {
        let h = home.to_string_lossy();
        if h.len() > 1 {
            s = s.replace(h.as_ref(), "~");
        }
    }
    s
}

// ── log tails ──────────────────────────────────────────────────────────────

/// Last `lines` lines of a file, reading at most [`MAX_LOG_TAIL_BYTES`] from its end.
pub fn tail_lines(path: &Path, lines: usize) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(MAX_LOG_TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let mut all: Vec<&str> = text.lines().collect();
    if start > 0 && !all.is_empty() {
        all.remove(0); // the first line is probably cut in half
    }
    let skip = all.len().saturating_sub(lines);
    Some(all[skip..].join("\n"))
}

// ── report ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeInfo {
    pub shell_version: String,
    pub tauri_version: String,
    pub webkit_version: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub arch: String,
    pub session_type: Option<String>,
    pub desktop: Option<String>,
    pub backend: String,
    pub compositor_version: Option<String>,
    pub pipewire_version: Option<String>,
    pub bwrap_version: Option<String>,
    pub portal_problems: Vec<String>,
}

pub fn render_report(info: &ProbeInfo, logs: &[(String, Option<String>)], now: &str) -> String {
    let opt = |o: &Option<String>| o.clone().unwrap_or_else(|| "not found".into());
    let mut r = String::new();
    r.push_str(&format!("# Blue Environment diagnostics\nGenerated: {now}\n\n## Versions\n"));
    r.push_str(&format!("- Shell: {}\n- Tauri: {}\n- WebKitGTK: {}\n", info.shell_version, info.tauri_version, opt(&info.webkit_version)));
    r.push_str(&format!("- OS: {}\n- Kernel: {}\n- Architecture: {}\n", opt(&info.os), opt(&info.kernel), info.arch));
    r.push_str(&format!("- Session: {} / {}\n", opt(&info.session_type), opt(&info.desktop)));
    r.push_str(&format!("- Compositor backend: {} ({})\n", info.backend, opt(&info.compositor_version)));
    r.push_str(&format!("- PipeWire: {}\n- bubblewrap: {}\n", opt(&info.pipewire_version), opt(&info.bwrap_version)));
    r.push_str("\n## Portals (file pickers, screenshots, screen sharing)\n");
    if info.portal_problems.is_empty() {
        r.push_str("- everything in place\n");
    } else {
        for p in &info.portal_problems {
            r.push_str(&format!("- {p}\n"));
        }
    }
    for (name, text) in logs {
        r.push_str(&format!("\n## {name}\n"));
        match text {
            Some(t) if !t.trim().is_empty() => r.push_str(&format!("```\n{}\n```\n", t.trim_end())),
            _ => r.push_str("(empty or not found)\n"),
        }
    }
    redact(&r)
}

// ── crash reporting ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrashSettings {
    #[serde(default)]
    pub enabled: bool,
    /// Where "Send" posts reports. Empty = nowhere (reports stay on this computer).
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub auto_send: bool,
}

impl Default for CrashSettings {
    fn default() -> Self {
        Self { enabled: false, endpoint: String::new(), auto_send: false }
    }
}

/// `https://…`, or plain http only to this computer (self-hosted dev collector).
pub fn valid_endpoint(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() || u.len() > 300 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    u.starts_with("https://") && u.len() > 8 || u.starts_with("http://127.0.0.1") || u.starts_with("http://localhost")
}

pub fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("BLUE_DIAG_DIR") {
        return PathBuf::from(d);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Blue-Environment")
}

pub fn crash_dir() -> PathBuf {
    if let Ok(d) = std::env::var("BLUE_DIAG_DIR") {
        return PathBuf::from(d).join("crashes");
    }
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Blue-Environment/crashes")
}

fn settings_path() -> PathBuf {
    config_dir().join("diagnostics.json")
}

pub fn load_settings() -> CrashSettings {
    std::fs::read_to_string(settings_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_settings(mut s: CrashSettings) -> Result<CrashSettings, String> {
    s.endpoint = s.endpoint.trim().to_string();
    if !s.endpoint.is_empty() && !valid_endpoint(&s.endpoint) {
        return Err("The endpoint must be an https:// address".into());
    }
    if s.auto_send && s.endpoint.is_empty() {
        s.auto_send = false; // nothing to send to
    }
    std::fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(), serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(s)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub id: String,
    pub time: String,
    pub version: String,
    pub thread: String,
    pub message: String,
    pub location: String,
    pub backtrace: String,
    #[serde(default)]
    pub sent: bool,
}

/// Builds a redacted report from a panic. Pure.
pub fn build_crash(version: &str, thread: &str, message: &str, location: &str, backtrace: &str, now: chrono::DateTime<chrono::Utc>) -> CrashReport {
    CrashReport {
        id: format!("crash-{}", now.format("%Y%m%d-%H%M%S-%3f")),
        time: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        version: version.to_string(),
        thread: redact(thread),
        message: redact(message),
        location: redact(location),
        backtrace: redact(backtrace),
        sent: false,
    }
}

fn id_ok(id: &str) -> bool {
    id.starts_with("crash-") && id.len() < 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub fn store_crash(report: &CrashReport) -> Result<(), String> {
    let dir = crash_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{}.json", report.id)), serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    // keep only the newest MAX_CRASH_REPORTS
    let mut ids = list_ids();
    ids.sort();
    while ids.len() > MAX_CRASH_REPORTS {
        let old = ids.remove(0);
        let _ = std::fs::remove_file(dir.join(format!("{old}.json")));
    }
    Ok(())
}

fn list_ids() -> Vec<String> {
    std::fs::read_dir(crash_dir())
        .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(String::from)).filter(|id| id_ok(id)).collect())
        .unwrap_or_default()
}

pub fn list_crashes() -> Vec<CrashReport> {
    let mut v: Vec<CrashReport> = list_ids().iter().filter_map(|id| get_crash(id)).collect();
    v.sort_by(|a, b| b.id.cmp(&a.id));
    v
}

pub fn get_crash(id: &str) -> Option<CrashReport> {
    if !id_ok(id) {
        return None;
    }
    std::fs::read_to_string(crash_dir().join(format!("{id}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok())
}

pub fn delete_crash(id: &str) -> bool {
    id_ok(id) && std::fs::remove_file(crash_dir().join(format!("{id}.json"))).is_ok()
}

pub fn mark_sent(id: &str) {
    if let Some(mut r) = get_crash(id) {
        r.sent = true;
        let _ = store_crash(&r);
    }
}

/// Installs a panic hook that records a (redacted) crash report **only while
/// crash reporting is enabled**; the previous hook (logging, abort message) still runs.
/// Returns the report when one was written so the caller can auto-send it.
pub fn install_panic_hook(version: &'static str, on_report: impl Fn(&CrashReport) + Send + Sync + 'static) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        if !load_settings().enabled {
            return;
        }
        let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into());
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        let report = build_crash(version, &thread, &message, &location, &bt, chrono::Utc::now());
        if store_crash(&report).is_ok() {
            on_report(&report);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    static GUARD: Mutex<()> = Mutex::new(());
    fn env() -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
        let g = GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let d = std::env::temp_dir().join(format!("blue-diag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::env::set_var("BLUE_DIAG_DIR", &d);
        (g, d)
    }

    #[test]
    fn personal_data_is_removed() {
        let raw = "user alice@example.com opened /home/alice/Documents/tax.pdf from 192.168.1.23 (mac aa:bb:cc:dd:ee:ff) on 10.0.0.5:8080";
        let out = redact_with(raw, None);
        for gone in ["alice@example.com", "/home/alice", "192.168.1.23", "aa:bb:cc:dd:ee:ff", "10.0.0.5"] {
            assert!(!out.contains(gone), "{gone} leaked: {out}");
        }
        assert!(out.contains("/home/<user>/Documents/tax.pdf") && out.contains("<email>") && out.contains("<ip>") && out.contains("<mac>"));
    }

    #[test]
    fn secrets_in_all_common_shapes_are_removed() {
        let raw = r#"password=hunter2 token: abc123def "api_key":"sk-live-123" Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig https://bob:s3cr3t@git.example.com/x.git?access_key=ZZZ999&a=1 session_id=xyz"#;
        let out = redact_with(raw, None);
        for gone in ["hunter2", "abc123def", "sk-live-123", "eyJhbGciOiJIUzI1NiJ9", "s3cr3t", "ZZZ999", "xyz"] {
            assert!(!out.contains(gone), "{gone} leaked: {out}");
        }
        assert!(out.contains("password=<redacted>"));
        assert!(out.contains("a=1"), "unrelated query parameters survive");
    }

    #[test]
    fn long_random_strings_go_but_paths_and_versions_stay() {
        let out = redact_with("key AbCdEfGhIjKlMnOpQrStUvWxYz0123456789AbCdEf at /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/injected-bundle/libwebkit2gtkinjectedbundle.so v2.52.6", None);
        assert!(!out.contains("AbCdEfGhIjKlMnOpQrStUvWxYz0123456789AbCdEf"));
        assert!(out.contains("/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/injected-bundle/libwebkit2gtkinjectedbundle.so") && out.contains("v2.52.6"));
    }

    #[test]
    fn host_name_and_localhost_handling() {
        let out = redact_with("connect to sunny-laptop failed; 127.0.0.1 ok; 999.1.1.1 is not an ip", Some("sunny-laptop"));
        assert!(out.contains("<host>") && !out.contains("sunny-laptop"));
        assert!(out.contains("127.0.0.1"));
        assert!(out.contains("999.1.1.1"), "not a valid address: left alone");
    }

    #[test]
    fn tail_returns_the_last_lines_and_survives_missing_files() {
        let (_g, d) = env();
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.log");
        std::fs::write(&f, (1..=1000).map(|i| format!("line {i}\n")).collect::<String>()).unwrap();
        let t = tail_lines(&f, 3).unwrap();
        assert_eq!(t, "line 998\nline 999\nline 1000");
        assert!(tail_lines(&d.join("nope.log"), 3).is_none());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn report_contains_sections_and_is_redacted() {
        let info = ProbeInfo { shell_version: "0.9".into(), tauri_version: "2".into(), backend: "labwc".into(), arch: "x86_64".into(), portal_problems: vec!["Screen sharing needs PipeWire".into()], ..Default::default() };
        let logs = vec![("launch.log".to_string(), Some("started by alice@example.com token=abc".to_string())), ("shell log".to_string(), None)];
        let r = render_report(&info, &logs, "2026-11-01T12:00:00Z");
        assert!(r.contains("Compositor backend: labwc") && r.contains("Screen sharing needs PipeWire") && r.contains("(empty or not found)"));
        assert!(!r.contains("alice@example.com") && !r.contains("token=abc"));
    }

    #[test]
    fn crash_reporting_is_off_by_default_and_endpoint_rules_hold() {
        let (_g, d) = env();
        assert_eq!(load_settings(), CrashSettings { enabled: false, endpoint: String::new(), auto_send: false });
        assert!(save_settings(CrashSettings { enabled: true, endpoint: "http://evil.example/x".into(), auto_send: false }).is_err(), "plain http to a remote host is refused");
        assert!(save_settings(CrashSettings { enabled: true, endpoint: "https://crash.example.org/in".into(), auto_send: true }).unwrap().auto_send);
        let s = save_settings(CrashSettings { enabled: true, endpoint: String::new(), auto_send: true }).unwrap();
        assert!(!s.auto_send, "auto-send without a destination is switched off");
        assert!(valid_endpoint("http://127.0.0.1:9000/in") && !valid_endpoint("https://") && !valid_endpoint("ftp://x") && !valid_endpoint("https://a b"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn crash_reports_are_redacted_stored_rotated_and_deletable() {
        let (_g, d) = env();
        let t0 = chrono::Utc::now();
        let r = build_crash("0.9", "main", "failed to open /home/bob/secret.txt for bob@x.org", "src/a.rs:1:2", "frame /home/bob/.cargo/x", t0);
        assert!(!r.message.contains("bob") && !r.backtrace.contains("/home/bob"));
        for i in 0..(MAX_CRASH_REPORTS + 5) {
            let mut r = r.clone();
            r.id = format!("crash-20260101-0000{:02}-000", i);
            store_crash(&r).unwrap();
        }
        let all = list_crashes();
        assert_eq!(all.len(), MAX_CRASH_REPORTS, "oldest reports are dropped");
        assert_eq!(all[0].id, format!("crash-20260101-0000{:02}-000", MAX_CRASH_REPORTS + 4), "newest first");
        assert!(delete_crash(&all[0].id) && get_crash(&all[0].id).is_none());
        assert!(get_crash("../../etc/passwd").is_none() && !delete_crash("../x"), "ids cannot escape the crash folder");
        mark_sent(&all[1].id);
        assert!(get_crash(&all[1].id).unwrap().sent);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn panic_hook_writes_nothing_while_disabled_and_a_report_when_enabled() {
        let (_g, d) = env();
        install_panic_hook("test", |_| {});
        let _ = std::panic::catch_unwind(|| panic!("boom while disabled"));
        assert!(list_crashes().is_empty(), "opt-in: nothing may be written by default");
        save_settings(CrashSettings { enabled: true, ..Default::default() }).unwrap();
        let _ = std::panic::catch_unwind(|| panic!("boom for alice@example.com"));
        let all = list_crashes();
        assert_eq!(all.len(), 1);
        assert!(all[0].message.contains("boom") && !all[0].message.contains("alice@example.com"));
        let _ = std::panic::take_hook();
        let _ = std::fs::remove_dir_all(d);
    }
}
