use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

fn sessions() -> &'static Mutex<HashMap<String, Child>> {
    static S: OnceLock<Mutex<HashMap<String, Child>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

const SOURCES: &[&str] = &["output", "playback", "mic"];

fn valid_serial(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

// ── pure helpers (unit-tested) ───────────────────────────────────────────────

/// "scrcpy 2.4 <https://…>" → (2, 4)
pub fn parse_scrcpy_version(out: &str) -> Option<(u32, u32)> {
    let first = out.lines().next()?;
    let v = first.strip_prefix("scrcpy ")?.split_whitespace().next()?;
    let mut it = v.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

/// Audio forwarding arrived in scrcpy 2.0.
pub fn version_supports_audio(v: (u32, u32)) -> bool {
    v.0 >= 2
}

/// Finds the PipeWire/Pulse sink-input id that belongs to process `pid`.
pub fn sink_input_for_pid(pactl_out: &str, pid: u32) -> Option<u32> {
    let needle = format!("application.process.id = \"{pid}\"");
    let mut current: Option<u32> = None;
    for line in pactl_out.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Sink Input #") {
            current = rest.trim().parse().ok();
        } else if t == needle {
            return current;
        }
    }
    None
}

/// Arguments for the audio-only scrcpy client.
pub fn scrcpy_audio_args(serial: &str, source: &str) -> Vec<String> {
    vec![
        "--serial".into(), serial.into(),
        "--no-video".into(), "--no-control".into(),
        format!("--audio-source={source}"),
        "--audio-buffer=60".into(), // ms — low enough for video, tolerant to Wi-Fi jitter
        "--no-window".into(),
    ]
}

// ── commands ─────────────────────────────────────────────────────────────────

fn scrcpy_version() -> Option<(u32, u32)> {
    let o = Command::new("scrcpy").arg("--version").stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    parse_scrcpy_version(&String::from_utf8_lossy(&o.stdout))
}

/// Whether audio forwarding is possible here (scrcpy installed and new enough).
#[tauri::command(async)]
pub fn phone_audio_supported() -> bool {
    scrcpy_version().map(version_supports_audio).unwrap_or(false)
}

/// Drops finished sessions and returns the serials that are still streaming audio.
#[tauri::command(async)]
pub fn phone_audio_status() -> Vec<String> {
    let mut map = sessions().lock().unwrap();
    map.retain(|_, c| matches!(c.try_wait(), Ok(None)));
    let mut v: Vec<String> = map.keys().cloned().collect();
    v.sort();
    v
}

#[tauri::command(async)]
pub fn phone_audio_start(serial: String, source: Option<String>) -> Result<(), String> {
    if !valid_serial(&serial) {
        return Err("Invalid device id".into());
    }
    let source = source.unwrap_or_else(|| "output".into());
    if !SOURCES.contains(&source.as_str()) {
        return Err("Invalid audio source".into());
    }
    match scrcpy_version() {
        None => return Err("scrcpy is not installed".into()),
        Some(v) if !version_supports_audio(v) => return Err("scrcpy 2.0 or newer is required for audio".into()),
        _ => {}
    }
    let mut map = sessions().lock().unwrap();
    if let Some(c) = map.get_mut(&serial) {
        if matches!(c.try_wait(), Ok(None)) {
            return Ok(()); // already running
        }
    }
    let mut child = Command::new("scrcpy")
        .args(scrcpy_audio_args(&serial, &source))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    // scrcpy exits within a moment when the device is unauthorised or Android < 11: surface that.
    std::thread::sleep(Duration::from_millis(900));
    if let Ok(Some(status)) = child.try_wait() {
        let mut err = String::new();
        if let Some(mut s) = child.stderr.take() {
            use std::io::Read;
            let _ = s.read_to_string(&mut err);
        }
        let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
        return Err(if last.is_empty() { format!("scrcpy exited ({status})") } else { last });
    }
    child.stderr.take(); // don't keep an unread pipe around
    map.insert(serial, child);
    Ok(())
}

#[tauri::command(async)]
pub fn phone_audio_stop(serial: String) -> Result<(), String> {
    if let Some(mut c) = sessions().lock().unwrap().remove(&serial) {
        let _ = c.kill();
        let _ = c.wait();
    }
    Ok(())
}

/// Stops every audio session (called when the panel closes).
#[tauri::command(async)]
pub fn phone_audio_stop_all() -> Result<(), String> {
    for (_, mut c) in sessions().lock().unwrap().drain() {
        let _ = c.kill();
        let _ = c.wait();
    }
    Ok(())
}

/// Sets the volume (0–150 %) of the phone's playback stream via `pactl` (PipeWire-Pulse / PulseAudio).
#[tauri::command(async)]
pub fn phone_audio_volume(serial: String, percent: u32) -> Result<(), String> {
    let pid = {
        let map = sessions().lock().unwrap();
        map.get(&serial).map(|c| c.id()).ok_or("Audio is not running")?
    };
    let out = Command::new("pactl")
        .args(["list", "sink-inputs"])
        .env("LC_ALL", "C")
        .output()
        .map_err(|_| "pactl is not available".to_string())?;
    let id = sink_input_for_pid(&String::from_utf8_lossy(&out.stdout), pid)
        .ok_or("The phone's audio stream is not visible yet — try again in a second")?;
    let status = Command::new("pactl")
        .args(["set-sink-input-volume", &id.to_string(), &format!("{}%", percent.min(150))])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err("pactl failed to set the volume".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scrcpy_versions() {
        assert_eq!(parse_scrcpy_version("scrcpy 2.4 <https://github.com/Genymobile/scrcpy>\n\n dependencies"), Some((2, 4)));
        assert_eq!(parse_scrcpy_version("scrcpy 1.25 <x>"), Some((1, 25)));
        assert_eq!(parse_scrcpy_version("scrcpy 3 <x>"), Some((3, 0)));
        assert_eq!(parse_scrcpy_version("garbage"), None);
        assert_eq!(parse_scrcpy_version(""), None);
    }

    #[test]
    fn audio_needs_scrcpy_2() {
        assert!(version_supports_audio((2, 0)));
        assert!(version_supports_audio((3, 1)));
        assert!(!version_supports_audio((1, 25)));
    }

    #[test]
    fn finds_sink_input_by_pid() {
        let out = "Sink Input #51\n\tDriver: PipeWire\n\tProperties:\n\t\tapplication.name = \"Firefox\"\n\t\tapplication.process.id = \"1111\"\n\nSink Input #52\n\tDriver: PipeWire\n\tProperties:\n\t\tapplication.name = \"scrcpy\"\n\t\tapplication.process.id = \"4567\"\n";
        assert_eq!(sink_input_for_pid(out, 4567), Some(52));
        assert_eq!(sink_input_for_pid(out, 1111), Some(51));
        assert_eq!(sink_input_for_pid(out, 9), None);
        // a pid that is a prefix of another must not match
        assert_eq!(sink_input_for_pid(out, 456), None);
    }

    #[test]
    fn builds_headless_audio_args() {
        let a = scrcpy_audio_args("emulator-5554", "output");
        assert!(a.contains(&"--no-video".to_string()));
        assert!(a.contains(&"--no-control".to_string()));
        assert!(a.contains(&"--audio-source=output".to_string()));
        assert!(a.contains(&"--no-window".to_string()));
    }

    #[test]
    fn validates_serials() {
        assert!(valid_serial("192.168.1.5:5555"));
        assert!(!valid_serial("a b"));
        assert!(!valid_serial("x;rm"));
        assert!(!valid_serial(""));
    }
}
