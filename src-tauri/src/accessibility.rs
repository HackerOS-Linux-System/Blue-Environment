use serde::Serialize;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScreenReaderStatus {
    pub installed: bool,
    pub running: bool,
    /// `spd-say` (speech-dispatcher) exists — used for the shell's own announcements.
    pub speech_available: bool,
}

fn exists(prog: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(prog).is_file())).unwrap_or(false)
}

fn running(prog: &str) -> bool {
    Command::new("pgrep").args(["-x", prog]).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

#[tauri::command(async)]
pub fn a11y_screen_reader_status() -> ScreenReaderStatus {
    ScreenReaderStatus { installed: exists("orca"), running: running("orca"), speech_available: exists("spd-say") }
}

/// Starts or stops Orca. Starting also turns on GNOME's `toolkit-accessibility`
/// key (harmless elsewhere) so AT-SPI is enabled for the webview.
#[tauri::command(async)]
pub fn a11y_screen_reader_set(enabled: bool) -> Result<(), String> {
    if enabled {
        if !exists("orca") {
            return Err("Orca is not installed — install the `orca` package.".into());
        }
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.desktop.interface", "toolkit-accessibility", "true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if running("orca") {
            return Ok(());
        }
        Command::new("orca")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not start Orca: {e}"))
    } else {
        if exists("orca") && running("orca") {
            let quit = Command::new("orca").arg("--quit").stdout(Stdio::null()).stderr(Stdio::null()).status();
            if !matches!(quit, Ok(s) if s.success()) {
                let _ = Command::new("pkill").args(["-x", "orca"]).status();
            }
        }
        Ok(())
    }
}

/// Speaks a short message with speech-dispatcher (used for the shell's own
/// announcements when Orca is not running). Text is passed as an argument,
/// never through a shell; capped at 400 characters.
#[tauri::command(async)]
pub fn a11y_speak(text: String) -> Result<(), String> {
    if !exists("spd-say") {
        return Err("speech-dispatcher (spd-say) is not installed".into());
    }
    let t: String = text.chars().filter(|c| !c.is_control()).take(400).collect();
    if t.trim().is_empty() {
        return Ok(());
    }
    Command::new("spd-say")
        .args(["--wait=false", "--", &t])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
