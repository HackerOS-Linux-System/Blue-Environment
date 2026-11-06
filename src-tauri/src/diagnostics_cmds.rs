use crate::diagnostics::{self as diag, CrashReport, CrashSettings, ProbeInfo};
use std::path::PathBuf;
use std::process::Command;

fn first_line_of(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    let text = String::from_utf8_lossy(if out.stdout.is_empty() { &out.stderr } else { &out.stdout }).to_string();
    text.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

fn os_pretty_name() -> Option<String> {
    std::fs::read_to_string("/etc/os-release").ok()?.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string())
}

fn shell_log_paths() -> Vec<(String, PathBuf)> {
    let mut v = vec![("Shell log".to_string(), PathBuf::from("/var/log/Blue-Environment/blue-environment.log"))];
    if let Some(home) = dirs::home_dir() {
        v.push(("Shell log (user)".to_string(), home.join(".cache/Blue-Environment/shell/logs/blue-environment.log")));
    }
    if let Some(cache) = dirs::cache_dir() {
        v.push(("Compositor launch log (launch.log)".to_string(), cache.join("Blue-Environment/launch.log")));
    }
    v
}

pub async fn probe() -> ProbeInfo {
    let backend = crate::backend::active();
    let compositor_bin = match backend {
        crate::backend::BackendKind::Labwc => Some("labwc"),
        crate::backend::BackendKind::Sway => Some("sway"),
        crate::backend::BackendKind::Wayfire => Some("wayfire"),
        crate::backend::BackendKind::HackerosComp => None,
    };
    let portals = crate::xdg_portal::status().await;
    ProbeInfo {
        shell_version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
        webkit_version: first_line_of("pkg-config", &["--modversion", "webkit2gtk-4.1"]),
        os: os_pretty_name(),
        kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease").ok().map(|s| s.trim().to_string()),
        arch: std::env::consts::ARCH.to_string(),
        session_type: std::env::var("XDG_SESSION_TYPE").ok(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
        backend: backend.as_str().to_string(),
        compositor_version: compositor_bin.and_then(|b| first_line_of(b, &["--version"])),
        pipewire_version: Command::new("pipewire").arg("--version").output().ok().and_then(|o| String::from_utf8_lossy(&o.stdout).lines().nth(1).map(|l| l.trim().to_string())).filter(|l| !l.is_empty()),
        bwrap_version: first_line_of("bwrap", &["--version"]),
        portal_problems: portals.problems,
    }
}

/// Versions and environment (already redacted by `render_report` when exported).
#[tauri::command]
pub async fn diagnostics_info() -> ProbeInfo {
    probe().await
}

/// The whole report as redacted text, for "Copy" and "Save".
#[tauri::command]
pub async fn diagnostics_report(log_lines: Option<usize>) -> String {
    let n = log_lines.unwrap_or(300).clamp(10, 2000);
    let info = probe().await;
    let logs: Vec<(String, Option<String>)> = shell_log_paths().into_iter().map(|(name, p)| (name, diag::tail_lines(&p, n))).collect();
    diag::render_report(&info, &logs, &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Writes the report to `~/Downloads/blue-diagnostics-<time>.txt` and returns the path.
#[tauri::command]
pub async fn diagnostics_save_report() -> Result<String, String> {
    let text = diagnostics_report(Some(500)).await;
    let dir = dirs::download_dir().or_else(dirs::home_dir).ok_or("no Downloads folder")?;
    let path = dir.join(format!("blue-diagnostics-{}.txt", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn crash_reporting_get() -> CrashSettings {
    diag::load_settings()
}

#[tauri::command]
pub fn crash_reporting_set(settings: CrashSettings) -> Result<CrashSettings, String> {
    diag::save_settings(settings)
}

#[tauri::command]
pub fn crash_reports_list() -> Vec<CrashReport> {
    diag::list_crashes()
}

#[tauri::command]
pub fn crash_report_delete(id: String) -> bool {
    diag::delete_crash(&id)
}

/// POSTs one (already redacted) report to the endpoint the person configured.
/// Without an endpoint nothing is ever sent.
pub async fn send_crash(id: &str) -> Result<(), String> {
    let settings = diag::load_settings();
    if settings.endpoint.is_empty() || !diag::valid_endpoint(&settings.endpoint) {
        return Err("No crash-report endpoint is configured".into());
    }
    let report = diag::get_crash(id).ok_or("Report not found")?;
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().map_err(|e| e.to_string())?;
    let resp = client.post(&settings.endpoint).json(&report).send().await.map_err(|e| format!("could not reach the endpoint: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("the endpoint answered {}", resp.status()));
    }
    diag::mark_sent(id);
    Ok(())
}

#[tauri::command]
pub async fn crash_report_send(id: String) -> Result<(), String> {
    send_crash(&id).await
}

/// Called once at start-up: installs the panic hook (records only while enabled)
/// and sends reports automatically only when the person turned auto-send on.
pub fn init() {
    diag::install_panic_hook(env!("CARGO_PKG_VERSION"), |report| {
        let s = diag::load_settings();
        if s.enabled && s.auto_send && !s.endpoint.is_empty() {
            let id = report.id.clone();
            // The process may be about to die: send on a throw-away thread/runtime, best effort.
            let _ = std::thread::Builder::new().name("crash-send".into()).spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                    let _ = rt.block_on(send_crash(&id));
                }
            });
        }
    });
}
