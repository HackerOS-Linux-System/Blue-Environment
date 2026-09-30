pub mod engine;
pub mod error;

use engine::InstallConfig;
use error::InstallError;
use serde::Serialize;
use std::process::Command;
use tauri::Emitter;

#[derive(Serialize, Clone)]
pub struct InstallerDisk {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
    pub removable: bool,
}

#[tauri::command]
pub async fn installer_list_disks() -> Result<Vec<InstallerDisk>, String> {
    let output = Command::new("lsblk")
        .args(["-d", "-b", "-o", "NAME,SIZE,MODEL,RM,TYPE", "-J"])
        .output()
        .map_err(|e| format!("lsblk failed: {e}"))?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse lsblk output: {e}"))?;

    let devices = json["blockdevices"].as_array().cloned().unwrap_or_default();
    let disks = devices
        .into_iter()
        .filter(|d| d["type"].as_str() == Some("disk"))
        .map(|d| InstallerDisk {
            path: format!("/dev/{}", d["name"].as_str().unwrap_or("")),
            model: d["model"].as_str().unwrap_or("Unknown disk").trim().to_string(),
            size_bytes: d["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
            removable: matches!(d["rm"].as_str(), Some("1")) || d["rm"].as_bool().unwrap_or(false),
        })
        .collect();

    Ok(disks)
}

/// Entry point of `blue-environment --installer-priv` (runs as root, see
/// `crate::privileged`). Reads one [`InstallConfig`] JSON document from
/// stdin, runs [`engine::run_install`], streaming `BLUE> {"pct":…}` progress
/// lines to stdout exactly like `BlueStore::run_priv_helper` does.
pub fn run_priv_helper() -> i32 {
    let mut input = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).is_err() {
        proto_error(&InstallError::new("internal", "Could not read the install request"));
        return 2;
    }
    let cfg: InstallConfig = match serde_json::from_str(input.trim()) {
        Ok(c) => c,
        Err(e) => {
            proto_error(&InstallError::new("internal", "Malformed install request").detail(e.to_string()));
            return 2;
        }
    };
    let progress = |pct: u8, msg: &str| proto_line(serde_json::json!({"event": "progress", "pct": pct, "message": msg}));
    match engine::run_install(&cfg, &progress) {
        Ok(()) => {
            proto_line(serde_json::json!({"event": "done"}));
            0
        }
        Err(e) => {
            proto_error(&e);
            1
        }
    }
}

fn proto_line(v: serde_json::Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}{}", crate::privileged::LINE_PREFIX, v);
    let _ = out.flush();
}
fn proto_error(e: &InstallError) {
    proto_line(serde_json::json!({"event": "error", "error": e}));
}

/// `blue-environment --installer-priv` is spawned directly by
/// `installer_run` below (not the generic `--store-priv` path) because
/// this needs its own progress event name (`installer-progress`, already
/// wired into `BlueInstallerApp.svelte`/`installState.ts`) and its own
/// argv marker.
#[tauri::command]
pub async fn installer_run(app: tauri::AppHandle, config: InstallConfig) -> Result<(), InstallError> {
    let payload = serde_json::to_string(&config).map_err(|e| InstallError::new("internal", e.to_string()))?;
    let app_for_thread = app.clone();

    // Mirrors BlueStore::run_job's protocol handling: track the actual
    // "done"/"error" event, and only fall back to interpreting the launch
    // itself as having failed (`explain_launch_failure`) when NEITHER
    // arrived — e.g. the install genuinely ran and failed partway through
    // (a real `InstallError`, already shown by ErrorStep.svelte) is a
    // completely different situation from pkexec never having started at
    // all, and must not be misreported as the latter.
    let mut final_result: Option<Result<(), InstallError>> = None;
    let outcome = tokio::task::spawn_blocking(move || {
        crate::privileged::run_privileged(
            &["--installer-priv"],
            &payload,
            |line| {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return };
                match v.get("event").and_then(|e| e.as_str()) {
                    Some("progress") => {
                        let pct = v.get("pct").and_then(|p| p.as_u64()).unwrap_or(0).min(100) as u8;
                        let label = v.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
                        let _ = app_for_thread.emit("installer-progress", serde_json::json!({"pct": pct, "label": label, "line": label}));
                    }
                    Some("done") => {
                        final_result = Some(Ok(()));
                        let _ = app_for_thread.emit("installer-progress", serde_json::json!({"pct": 100, "label": "Installation complete", "line": "Installation complete"}));
                    }
                    Some("error") => {
                        let err = serde_json::from_value::<InstallError>(v.get("error").cloned().unwrap_or_default())
                            .unwrap_or_else(|_| InstallError::new("internal", "The installer reported an unknown error"));
                        let _ = app_for_thread.emit("installer-progress", serde_json::json!({"pct": -1, "label": serde_json::Value::Null, "line": format!("ERROR {}", err.message)}));
                        final_result = Some(Err(err));
                    }
                    _ => {}
                }
            },
            |_log| {},
        )
        .map(|o| (o, final_result))
    })
    .await
    .map_err(|e| InstallError::new("internal", "Install task crashed").detail(e.to_string()))?
    .map_err(|e| {
        let mut err = InstallError::new("privilege", e.message);
        if let Some(h) = e.hint {
            err = err.hint(h);
        }
        if let Some(d) = e.detail {
            err = err.detail(d);
        }
        err
    })?;

    let (proc_outcome, final_result) = outcome;
    if let Some(result) = final_result {
        return result;
    }
    // No protocol event ever arrived — the privileged process itself
    // couldn't be started or authenticated.
    let info = crate::privileged::explain_launch_failure(&proc_outcome);
    let mut err = InstallError::new("privilege", info.message);
    if let Some(h) = info.hint {
        err = err.hint(h);
    }
    if !proc_outcome.stderr_tail.is_empty() {
        err = err.detail(proc_outcome.stderr_tail);
    }
    Err(err)
}
