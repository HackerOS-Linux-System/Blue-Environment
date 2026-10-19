use serde::Serialize;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Printer {
    pub name: String,
    /// `idle` | `printing` | `disabled`
    pub state: String,
    pub reason: String,
    pub uri: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrintersState {
    pub cups_available: bool,
    pub printers: Vec<Printer>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredPrinter {
    pub uri: String,
    /// `network` | `direct` (usb) | …
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrintJob {
    pub id: u32,
    pub printer: String,
    pub user: String,
    pub size_bytes: u64,
    pub submitted: String,
}

// ── Pure parsers / validators ───────────────────────────────────────────

/// Queue names: what CUPS itself accepts without quoting trouble.
pub fn is_valid_queue_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 127 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Device URIs we are willing to hand to `lpadmin -v`.
pub fn is_valid_device_uri(u: &str) -> bool {
    if u.len() > 512 || u.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let Some((scheme, rest)) = u.split_once("://") else { return false };
    const OK: &[&str] = &["ipp", "ipps", "http", "https", "socket", "lpd", "usb", "dnssd", "hp", "hpfax", "smb", "ippusb"];
    OK.contains(&scheme) && !rest.is_empty()
}

pub fn parse_default(lpstat_d: &str) -> Option<String> {
    lpstat_d.lines().find_map(|l| l.trim().strip_prefix("system default destination:").map(|x| x.trim().to_string()))
}

/// `lpstat -v`: `device for NAME: uri`
pub fn parse_devices(lpstat_v: &str) -> Vec<(String, String)> {
    lpstat_v
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("device for ")?;
            let (name, uri) = rest.split_once(": ")?;
            Some((name.trim().to_string(), uri.trim().to_string()))
        })
        .collect()
}

/// `lpstat -p` (state lines start with `printer NAME`, the reason of a
/// disabled queue is on the following indented line).
pub fn parse_printer_states(lpstat_p: &str) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for l in lpstat_p.lines() {
        if let Some(rest) = l.strip_prefix("printer ") {
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            let low = rest.to_lowercase();
            let state = if low.contains("disabled") {
                "disabled"
            } else if low.contains("now printing") || low.contains("printing ") {
                "printing"
            } else {
                "idle"
            };
            out.push((name, state.to_string(), String::new()));
        } else if l.starts_with(char::is_whitespace) {
            if let Some(last) = out.last_mut() {
                if last.2.is_empty() {
                    last.2 = l.trim().to_string();
                }
            }
        }
    }
    out
}

/// `lpinfo -v`: `network ipp://host/…` / `direct usb://…`. Generic backend
/// entries without a host (`network http`, `network socket`) are dropped.
pub fn parse_lpinfo(out: &str) -> Vec<DiscoveredPrinter> {
    let mut seen = std::collections::HashSet::new();
    out.lines()
        .filter_map(|l| {
            let (kind, uri) = l.trim().split_once(' ')?;
            let uri = uri.trim();
            if !uri.contains("://") || !is_valid_device_uri(uri) {
                return None;
            }
            if !seen.insert(uri.to_string()) {
                return None;
            }
            let label = uri
                .split("://")
                .nth(1)
                .unwrap_or(uri)
                .trim_end_matches('/')
                .replace("%20", " ")
                .replace('+', " ");
            Some(DiscoveredPrinter { uri: uri.to_string(), kind: kind.to_string(), label })
        })
        .collect()
}

/// `lpstat -o`: `NAME-12   user   1024   Mon 01 Jan 2026 10:00:00`
pub fn parse_jobs(out: &str) -> Vec<PrintJob> {
    out.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let jobname = it.next()?;
            let user = it.next()?.to_string();
            let size: u64 = it.next()?.parse().unwrap_or(0);
            let submitted = it.collect::<Vec<_>>().join(" ");
            let (printer, id) = jobname.rsplit_once('-')?;
            Some(PrintJob { id: id.parse().ok()?, printer: printer.to_string(), user, size_bytes: size, submitted })
        })
        .collect()
}

pub fn parse_request_id(lp_out: &str) -> Option<String> {
    lp_out.lines().find_map(|l| l.trim().strip_prefix("request id is ").map(|r| r.split_whitespace().next().unwrap_or("").to_string()))
}

// ── Process helpers ─────────────────────────────────────────────────────

fn run(prog: &str, args: &[&str]) -> Result<(String, String, bool), String> {
    let o = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Cannot run `{prog}` — is CUPS installed? ({e})"))?;
    Ok((String::from_utf8_lossy(&o.stdout).to_string(), String::from_utf8_lossy(&o.stderr).to_string(), o.status.success()))
}

fn needs_auth(stderr: &str) -> bool {
    let l = stderr.to_lowercase();
    l.contains("forbidden") || l.contains("not authorized") || l.contains("unauthorized") || l.contains("permission denied") || l.contains("authentication")
}

/// Runs an administrative CUPS tool, escalating through pkexec once if CUPS
/// refuses the plain attempt.
fn run_admin(prog: &str, args: &[&str]) -> Result<String, String> {
    let (out, err, ok) = run(prog, args)?;
    if ok {
        return Ok(out);
    }
    if needs_auth(&err) && !crate::privileged::is_root() {
        let mut full = vec![prog];
        full.extend_from_slice(args);
        let (out2, err2, ok2) = run("pkexec", &full)?;
        if ok2 {
            return Ok(out2);
        }
        return Err(first_line(&err2, "Administrator authentication failed"));
    }
    Err(first_line(&err, &format!("`{prog}` failed")))
}

fn first_line(s: &str, fallback: &str) -> String {
    s.lines().map(|l| l.trim()).find(|l| !l.is_empty()).map(|l| l.to_string()).unwrap_or_else(|| fallback.to_string())
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command(async)]
pub fn printers_list() -> PrintersState {
    // `lpstat -r` answers "scheduler is running" / "scheduler is not running".
    let running = run("lpstat", &["-r"]).map(|r| r.0.contains("scheduler is running")).unwrap_or(false);
    if !running {
        return PrintersState { cups_available: false, printers: vec![] };
    }
    let p = run("lpstat", &["-p"]).map(|r| r.0).unwrap_or_default();
    let devices = run("lpstat", &["-v"]).map(|r| parse_devices(&r.0)).unwrap_or_default();
    let default = run("lpstat", &["-d"]).ok().and_then(|r| parse_default(&r.0));
    let printers = parse_printer_states(&p)
        .into_iter()
        .map(|(name, state, reason)| Printer {
            uri: devices.iter().find(|(n, _)| *n == name).map(|(_, u)| u.clone()).unwrap_or_default(),
            is_default: default.as_deref() == Some(name.as_str()),
            name,
            state,
            reason,
        })
        .collect::<Vec<_>>();
    // `lpstat -p` exits non-zero with "No destinations added." when there are none — not an error.
    PrintersState { cups_available: true, printers }
}

#[tauri::command(async)]
pub fn printers_discover() -> Result<Vec<DiscoveredPrinter>, String> {
    // `lpinfo` needs administrator rights on most systems; try plain first.
    let out = run_admin("lpinfo", &["-v"])?;
    let mut list = parse_lpinfo(&out);
    // Offer driverless-capable network printers first.
    list.sort_by_key(|d| match d.kind.as_str() {
        "network" => 0,
        "direct" => 1,
        _ => 2,
    });
    Ok(list)
}

/// Adds a queue. `driver` defaults to `everywhere` (IPP Everywhere /
/// driverless); pass `raw` for a raw queue or a PPD name from `lpinfo -m`.
#[tauri::command(async)]
pub fn printers_add(name: String, uri: String, driver: Option<String>, make_default: bool) -> Result<(), String> {
    if !is_valid_queue_name(&name) {
        return Err("The name may only contain letters, digits, '-' and '_'".into());
    }
    if !is_valid_device_uri(&uri) {
        return Err("That address is not a valid printer URI".into());
    }
    let driver = driver.unwrap_or_else(|| "everywhere".into());
    if driver.len() > 200 || driver.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("Invalid driver name".into());
    }
    run_admin("lpadmin", &["-p", &name, "-E", "-v", &uri, "-m", &driver])?;
    if make_default {
        run_admin("lpadmin", &["-d", &name])?;
    }
    Ok(())
}

#[tauri::command(async)]
pub fn printers_remove(name: String) -> Result<(), String> {
    if !is_valid_queue_name(&name) {
        return Err("Invalid printer name".into());
    }
    run_admin("lpadmin", &["-x", &name]).map(|_| ())
}

#[tauri::command(async)]
pub fn printers_set_default(name: String) -> Result<(), String> {
    if !is_valid_queue_name(&name) {
        return Err("Invalid printer name".into());
    }
    // Per-user default first (needs no rights); fall back to the system default.
    match run("lpoptions", &["-d", &name]) {
        Ok((_, _, true)) => Ok(()),
        _ => run_admin("lpadmin", &["-d", &name]).map(|_| ()),
    }
}

#[tauri::command(async)]
pub fn printers_set_enabled(name: String, enabled: bool) -> Result<(), String> {
    if !is_valid_queue_name(&name) {
        return Err("Invalid printer name".into());
    }
    let tool = if enabled { "cupsenable" } else { "cupsdisable" };
    run_admin(tool, &[&name]).map(|_| ())
}

#[tauri::command(async)]
pub fn printers_jobs() -> Vec<PrintJob> {
    run("lpstat", &["-o"]).map(|r| parse_jobs(&r.0)).unwrap_or_default()
}

#[tauri::command(async)]
pub fn printers_cancel_job(id: u32) -> Result<(), String> {
    run_admin("cancel", &[&id.to_string()]).map(|_| ())
}

#[tauri::command(async)]
pub fn printers_cancel_all(name: String) -> Result<(), String> {
    if !is_valid_queue_name(&name) {
        return Err("Invalid printer name".into());
    }
    run_admin("cancel", &["-a", &name]).map(|_| ())
}

/// Sends a file to a queue. `duplex` maps to the standard `sides` option.
#[tauri::command(async)]
pub fn printers_print_file(name: String, path: String, copies: Option<u32>, duplex: Option<bool>) -> Result<String, String> {
    if !is_valid_queue_name(&name) {
        return Err("Invalid printer name".into());
    }
    if !std::path::Path::new(&path).is_file() {
        return Err(format!("{path} is not a file"));
    }
    let copies = copies.unwrap_or(1).clamp(1, 99).to_string();
    let mut args: Vec<&str> = vec!["-d", &name, "-n", &copies];
    if duplex.unwrap_or(false) {
        args.extend(["-o", "sides=two-sided-long-edge"]);
    }
    args.push("--");
    args.push(&path);
    let (out, err, ok) = run("lp", &args)?;
    if ok {
        Ok(parse_request_id(&out).unwrap_or_default())
    } else {
        Err(first_line(&err, "Printing failed"))
    }
}

#[tauri::command(async)]
pub fn printers_test_page(name: String) -> Result<String, String> {
    if !is_valid_queue_name(&name) {
        return Err("Invalid printer name".into());
    }
    let cups_page = std::path::Path::new("/usr/share/cups/data/testprint");
    let page = if cups_page.is_file() {
        cups_page.to_path_buf()
    } else {
        let p = std::env::temp_dir().join(format!("blue-test-page-{}.txt", std::process::id()));
        std::fs::write(
            &p,
            format!("Blue Environment test page\n\nPrinter: {name}\nIf you can read this, printing works.\n"),
        )
        .map_err(|e| e.to_string())?;
        p
    };
    printers_print_file(name, page.to_string_lossy().to_string(), Some(1), Some(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_uris() {
        assert!(is_valid_queue_name("HP_LaserJet-1"));
        assert!(!is_valid_queue_name("bad name"));
        assert!(!is_valid_queue_name("a;b"));
        assert!(!is_valid_queue_name(""));
        assert!(is_valid_device_uri("ipp://printer.local:631/ipp/print"));
        assert!(is_valid_device_uri("usb://HP/LaserJet?serial=1"));
        assert!(!is_valid_device_uri("file:///etc/passwd"));
        assert!(!is_valid_device_uri("ipp://x y"));
        assert!(!is_valid_device_uri("notauri"));
    }

    #[test]
    fn parses_lpstat() {
        let p = "printer Office is idle.  enabled since Mon 01 Jan 2026\nprinter Lab disabled since Mon 01 Jan 2026 -\n\tPaused\nprinter Hall now printing Hall-7.  enabled since x\n";
        let s = parse_printer_states(p);
        assert_eq!(s[0], ("Office".into(), "idle".into(), String::new()));
        assert_eq!(s[1], ("Lab".into(), "disabled".into(), "Paused".into()));
        assert_eq!(s[2].1, "printing");
        let v = "device for Office: ipp://10.0.0.5/ipp/print\ndevice for Lab: usb://HP/x\n";
        assert_eq!(parse_devices(v)[1], ("Lab".into(), "usb://HP/x".into()));
        assert_eq!(parse_default("system default destination: Office\n"), Some("Office".into()));
        assert_eq!(parse_default("no system default destination\n"), None);
    }

    #[test]
    fn parses_lpinfo_and_jobs() {
        let out = "network socket\nnetwork http\nnetwork ipp://Brother%20HL._ipp._tcp.local/\ndirect usb://HP/LaserJet?serial=1\nnetwork ipp://Brother%20HL._ipp._tcp.local/\n";
        let d = parse_lpinfo(out);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].kind, "network");
        assert!(d[0].label.contains("Brother HL"));
        let jobs = parse_jobs("Office-12   alice   1024   Mon 01 Jan 2026 10:00:00\nLab-7 bob 0 Tue\n");
        assert_eq!(jobs.len(), 2);
        assert_eq!((jobs[0].id, jobs[0].printer.as_str(), jobs[0].user.as_str(), jobs[0].size_bytes), (12, "Office", "alice", 1024));
        assert_eq!(parse_request_id("request id is Office-13 (1 file(s))\n"), Some("Office-13".into()));
    }

    #[test]
    fn auth_detection() {
        assert!(needs_auth("lpadmin: Forbidden"));
        assert!(!needs_auth("lpadmin: Bad printer"));
    }
}
