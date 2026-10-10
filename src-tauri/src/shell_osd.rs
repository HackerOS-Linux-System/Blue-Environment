use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

/// Event names the frontend listens to (`App.svelte`).
pub const EVENT_VOLUME: &str = "osd:volume";
pub const EVENT_BRIGHTNESS: &str = "osd:brightness";

/// The default sink's level. Values above 100 % are possible (PulseAudio and
/// PipeWire allow amplification up to 150 %).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VolumeState {
    pub volume: i32,
    pub muted: bool,
}

/// `Volume: front-left: 42598 /  65% / -11.56 dB, front-right: …` → `65`.
/// Takes the first percentage on the line, which is the left/mono channel.
pub fn parse_volume_percent(text: &str) -> Option<i32> {
    let idx = text.find('%')?;
    let digits: String = text[..idx]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

/// `Mute: yes` / `Mute: no` (pactl is always run with `LC_ALL=C`).
pub fn parse_mute(text: &str) -> Option<bool> {
    let v = text.split(':').nth(1)?.trim().to_ascii_lowercase();
    match v.as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// `amixer get Master` → `[65%] [on]`.
fn parse_amixer(text: &str) -> Option<VolumeState> {
    let line = text.lines().rev().find(|l| l.contains('%'))?;
    let volume = {
        let open = line.find('[')?;
        parse_volume_percent(&line[open..])?
    };
    Some(VolumeState { volume, muted: line.contains("[off]") })
}

/// Only events about a sink or the server (default sink switch) can change what
/// the OSD shows; `sink-input`/`source`/`card` lines are noise.
pub fn is_relevant_event(line: &str) -> bool {
    line.contains(" on sink #") || line.contains(" on server")
}

/// New level after a relative step. Stepping up never crosses 100 % (like KDE
/// without "raise maximum volume") but a level that is already amplified may
/// still be lowered; the result is never negative.
pub fn stepped_volume(current: i32, delta: i32) -> i32 {
    let upper = current.max(100);
    (current + delta).clamp(0, upper)
}

fn pactl(args: &[&str]) -> Option<String> {
    let out = Command::new("pactl")
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn amixer(args: &[&str]) -> Option<String> {
    let out = Command::new("amixer")
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Current level of the default output (PulseAudio/PipeWire, ALSA as fallback).
pub fn read_volume() -> Option<VolumeState> {
    if let Some(v) = pactl(&["get-sink-volume", "@DEFAULT_SINK@"]).and_then(|t| parse_volume_percent(&t)) {
        let muted = pactl(&["get-sink-mute", "@DEFAULT_SINK@"]).and_then(|t| parse_mute(&t)).unwrap_or(false);
        return Some(VolumeState { volume: v, muted });
    }
    amixer(&["get", "Master"]).and_then(|t| parse_amixer(&t))
}

fn write_volume(percent: i32) -> Result<(), String> {
    let pct = format!("{}%", percent.clamp(0, 150));
    if Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &pct])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    if Command::new("amixer")
        .args(["set", "Master", &pct])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    Err("no audio backend found (pactl/amixer)".to_string())
}

fn write_mute(muted: bool) -> Result<(), String> {
    let ok = Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", if muted { "1" } else { "0" }])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
        || Command::new("amixer")
            .args(["set", "Master", if muted { "mute" } else { "unmute" }])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    if ok { Ok(()) } else { Err("no audio backend found (pactl/amixer)".to_string()) }
}

/// Relative volume step (media keys). Raising the volume also unmutes, like
/// every desktop does. Returns the state after the change.
pub fn adjust_volume(delta: i32) -> Result<VolumeState, String> {
    let cur = read_volume().ok_or_else(|| "cannot read the current volume".to_string())?;
    let target = stepped_volume(cur.volume, delta);
    if target != cur.volume {
        write_volume(target)?;
    }
    if delta > 0 && cur.muted {
        write_mute(false)?;
    }
    Ok(read_volume().unwrap_or(VolumeState { volume: target, muted: cur.muted && delta <= 0 }))
}

/// Mute toggle (media key). Returns the state after the change.
pub fn toggle_mute() -> Result<VolumeState, String> {
    let cur = read_volume().ok_or_else(|| "cannot read the current volume".to_string())?;
    write_mute(!cur.muted)?;
    Ok(read_volume().unwrap_or(VolumeState { volume: cur.volume, muted: !cur.muted }))
}

/// Backlight percentage from a `/sys/class/backlight`-style directory. When a
/// machine exposes several devices the one with the finest range wins (that is
/// the real panel driver, not a coarse ACPI shim).
pub fn read_brightness_percent(dir: &Path) -> Option<i32> {
    let mut best: Option<(u64, i32)> = None; // (max, percent)
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        let cur = std::fs::read_to_string(p.join("brightness")).ok().and_then(|t| t.trim().parse::<f64>().ok());
        let max = std::fs::read_to_string(p.join("max_brightness")).ok().and_then(|t| t.trim().parse::<f64>().ok());
        if let (Some(c), Some(m)) = (cur, max) {
            if m <= 0.0 {
                continue;
            }
            let pct = ((c / m) * 100.0).round() as i32;
            if best.map_or(true, |(bm, _)| (m as u64) > bm) {
                best = Some((m as u64, pct.clamp(0, 100)));
            }
        }
    }
    best.map(|(_, pct)| pct)
}

/// Brightness of the machine's panel, `None` on a desktop without a backlight.
pub fn read_brightness() -> Option<i32> {
    read_brightness_percent(Path::new("/sys/class/backlight"))
}

/// New brightness after a relative step: never below 1 % (a black screen with
/// no way back) and never above 100 %.
pub fn stepped_brightness(current: i32, delta: i32) -> i32 {
    (current + delta).clamp(1, 100)
}

type Emit = Arc<dyn Fn(&str, Value) + Send + Sync>;

fn emit_volume(emit: &Emit, s: VolumeState) {
    emit(EVENT_VOLUME, json!({ "volume": s.volume, "muted": s.muted }));
}

/// Starts both watcher threads. Never blocks, never panics the caller.
pub fn start_watchers<E>(emit: E)
where
    E: Fn(&str, Value) + Send + Sync + 'static,
{
    let emit: Emit = Arc::new(emit);
    let e1 = emit.clone();
    let _ = thread::Builder::new().name("osd-volume".into()).spawn(move || volume_watcher(e1));
    let _ = thread::Builder::new().name("osd-brightness".into()).spawn(move || brightness_watcher(emit));
}

fn volume_watcher(emit: Emit) {
    let mut last = read_volume();
    let mut backoff = Duration::from_secs(2);
    loop {
        let spawned = Command::new("pactl")
            .arg("subscribe")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        match spawned {
            Ok(mut child) => {
                backoff = Duration::from_secs(2);
                if let Some(stdout) = child.stdout.take() {
                    let (tx, rx) = mpsc::channel::<()>();
                    let reader = thread::spawn(move || {
                        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                            if is_relevant_event(&line) && tx.send(()).is_err() {
                                break;
                            }
                        }
                    });
                    // Block for the first event, then swallow the burst a single
                    // key press produces before reading the state once.
                    while rx.recv().is_ok() {
                        while rx.recv_timeout(Duration::from_millis(30)).is_ok() {}
                        let now = read_volume();
                        if let (Some(s), true) = (now, now != last) {
                            emit_volume(&emit, s);
                        }
                        if now.is_some() {
                            last = now;
                        }
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                } else {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
            // `pactl` is not installed (pure-ALSA system): look again rarely.
            Err(_) => backoff = (backoff * 2).min(Duration::from_secs(60)),
        }
        thread::sleep(backoff);
    }
}

fn brightness_watcher(emit: Emit) {
    let mut last = read_brightness();
    loop {
        // No backlight (desktop PC): nothing to watch, check again rarely.
        thread::sleep(Duration::from_millis(if last.is_some() { 300 } else { 10_000 }));
        let now = read_brightness();
        if let (Some(prev), Some(cur)) = (last, now) {
            if prev != cur {
                emit(EVENT_BRIGHTNESS, json!({ "percent": cur }));
            }
        }
        last = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pactl_volume_lines() {
        let stereo = "Volume: front-left: 42598 /  65% / -11.56 dB,   front-right: 42598 /  65% / -11.56 dB\n        balance 0.00\n";
        assert_eq!(parse_volume_percent(stereo), Some(65));
        assert_eq!(parse_volume_percent("Volume: mono: 65536 / 100% / 0.00 dB\n"), Some(100));
        assert_eq!(parse_volume_percent("Volume: front-left: 98304 / 150% / 10.57 dB"), Some(150));
        assert_eq!(parse_volume_percent("Volume: front-left: 0 /   0% / -inf dB"), Some(0));
        assert_eq!(parse_volume_percent("no percent here"), None);
    }

    #[test]
    fn parses_mute() {
        assert_eq!(parse_mute("Mute: yes\n"), Some(true));
        assert_eq!(parse_mute("Mute: no\n"), Some(false));
        assert_eq!(parse_mute("garbage"), None);
    }

    #[test]
    fn parses_amixer_fallback() {
        let t = "Simple mixer control 'Master',0\n  Front Left: Playback 42 [65%] [-12.00dB] [on]\n  Front Right: Playback 42 [65%] [-12.00dB] [on]\n";
        assert_eq!(parse_amixer(t), Some(VolumeState { volume: 65, muted: false }));
        let off = "  Mono: Playback 30 [40%] [off]\n";
        assert_eq!(parse_amixer(off), Some(VolumeState { volume: 40, muted: true }));
    }

    #[test]
    fn only_default_sink_events_are_relevant() {
        assert!(is_relevant_event("Event 'change' on sink #52"));
        assert!(is_relevant_event("Event 'change' on server"));
        assert!(!is_relevant_event("Event 'change' on sink-input #8"));
        assert!(!is_relevant_event("Event 'new' on source #3"));
        assert!(!is_relevant_event("Event 'change' on card #1"));
    }

    #[test]
    fn volume_steps_stop_at_100_but_amplified_levels_can_come_down() {
        assert_eq!(stepped_volume(50, 5), 55);
        assert_eq!(stepped_volume(98, 5), 100);
        assert_eq!(stepped_volume(100, 5), 100);
        assert_eq!(stepped_volume(3, -5), 0);
        // already amplified: no further increase, but decreasing works
        assert_eq!(stepped_volume(120, 5), 120);
        assert_eq!(stepped_volume(120, -5), 115);
    }

    #[test]
    fn brightness_steps_never_black_out() {
        assert_eq!(stepped_brightness(50, 5), 55);
        assert_eq!(stepped_brightness(3, -5), 1);
        assert_eq!(stepped_brightness(98, 5), 100);
    }

    fn fake_backlight(devs: &[(&str, &str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("blue-osd-test-{}-{:?}", std::process::id(), thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, cur, max) in devs {
            let d = dir.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("brightness"), format!("{cur}\n")).unwrap();
            std::fs::write(d.join("max_brightness"), format!("{max}\n")).unwrap();
        }
        dir
    }

    #[test]
    fn reads_backlight_percentage() {
        let dir = fake_backlight(&[("intel_backlight", "48000", "96000")]);
        assert_eq!(read_brightness_percent(&dir), Some(50));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prefers_the_device_with_the_finest_range() {
        let dir = fake_backlight(&[("acpi_video0", "5", "10"), ("amdgpu_bl1", "255", "255")]);
        assert_eq!(read_brightness_percent(&dir), Some(100));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_backlight_means_none() {
        let dir = std::env::temp_dir().join("blue-osd-test-missing-dir");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(read_brightness_percent(&dir), None);
        let empty = fake_backlight(&[]);
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(read_brightness_percent(&empty), None);
        let _ = std::fs::remove_dir_all(&empty);
    }
}
