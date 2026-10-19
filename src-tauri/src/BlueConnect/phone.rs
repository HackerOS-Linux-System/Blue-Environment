use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// ───────────────────────────── types ──────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdbStatus {
    pub installed: bool,
    pub version: String,
    pub ffmpeg: bool,
    pub scrcpy: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhoneDevice {
    pub serial: String,
    /// `device` | `unauthorized` | `offline` | …
    pub state: String,
    pub model: String,
    pub product: String,
    pub is_wifi: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhoneInfo {
    pub manufacturer: String,
    pub model: String,
    pub android: String,
    pub sdk: u32,
    pub screen_w: u32,
    pub screen_h: u32,
    pub battery_level: Option<u32>,
    pub charging: bool,
    pub battery_temp_c: Option<f32>,
    pub storage_total_kb: u64,
    pub storage_used_kb: u64,
    pub wifi_ip: String,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhoneApp {
    pub package: String,
    /// Human-ish label derived from the package name (real labels need aapt).
    pub label: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhoneNotification {
    pub package: String,
    pub title: String,
    pub text: String,
}

// ───────────────────────── validation & quoting ─────────────────────────

pub fn valid_serial(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

/// `com.example.app` — at least two dot-separated identifiers.
pub fn valid_package(p: &str) -> bool {
    if p.len() > 200 || !p.contains('.') {
        return false;
    }
    p.split('.').all(|seg| {
        let mut ch = seg.chars();
        matches!(ch.next(), Some(c) if c.is_ascii_alphabetic()) && ch.all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// `192.168.1.20:5555` / `[fe80::1]:37000` / `phone.local:5555`.
pub fn valid_host_port(s: &str) -> bool {
    let Some((host, port)) = s.rsplit_once(':') else { return false };
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'));
    matches!(port.parse::<u16>(), Ok(p) if p > 0) && host_ok
}

pub fn valid_pair_code(c: &str) -> bool {
    c.len() == 6 && c.chars().all(|ch| ch.is_ascii_digit())
}

/// Phone numbers as typed by people: digits, `+*#()-` and spaces.
pub fn valid_phone_number(n: &str) -> bool {
    let t = n.trim();
    !t.is_empty() && t.len() <= 32 && t.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '*' | '#' | '(' | ')' | '-' | ' ')) && t.chars().any(|c| c.is_ascii_digit())
}

/// Absolute device path without NUL / newline / `..` segments.
pub fn valid_remote_path(p: &str) -> bool {
    p.starts_with('/') && p.len() <= 1024 && !p.contains(['\0', '\n', '\r']) && !p.split('/').any(|s| s == "..")
}

/// Single-quotes `s` for the *device's* `sh`: `it's` → `'it'\''s'`.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Escapes text for `adb shell input text`: spaces become `%s`; shell
/// metacharacters are backslash-escaped. Non-ASCII is rejected because
/// `input text` cannot type it.
pub fn escape_input_text(text: &str) -> Result<String, String> {
    if text.is_empty() {
        return Err("Nothing to type".into());
    }
    if text.len() > 500 {
        return Err("Text is too long (500 characters max)".into());
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            ' ' => out.push_str("%s"),
            '%' => out.push_str("\\%"),
            c if c.is_ascii_alphanumeric() => out.push(c),
            '.' | ',' | '-' | '_' | '@' | '+' | ':' | '=' | '/' | '?' | '!' => out.push(c),
            '\n' | '\r' | '\t' => return Err("Line breaks are not supported — send Enter as a key".into()),
            c if c.is_ascii() && !c.is_control() => {
                out.push('\\');
                out.push(c);
            }
            _ => return Err("The phone's shell can only type ASCII text".into()),
        }
    }
    Ok(out)
}

/// Android KEYCODE_* values for the keys the UI offers.
pub fn keycode(name: &str) -> Option<u32> {
    Some(match name {
        "back" => 4,
        "home" => 3,
        "recents" => 187,
        "power" => 26,
        "volume_up" => 24,
        "volume_down" => 25,
        "mute" => 164,
        "enter" => 66,
        "delete" => 67,
        "forward_delete" => 112,
        "tab" => 61,
        "escape" => 111,
        "menu" => 82,
        "wake" => 224,
        "sleep" => 223,
        "call" => 5,
        "end_call" => 6,
        "dpad_up" => 19,
        "dpad_down" => 20,
        "dpad_left" => 21,
        "dpad_right" => 22,
        "dpad_center" => 23,
        "page_up" => 92,
        "page_down" => 93,
        "move_home" => 122,
        "move_end" => 123,
        "play_pause" => 85,
        "next" => 87,
        "previous" => 88,
        "camera" => 27,
        "search" => 84,
        _ => return None,
    })
}

// ───────────────────────────── parsers ──────────────────────────────

pub fn parse_devices(out: &str) -> Vec<PhoneDevice> {
    out.lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let serial = it.next()?.to_string();
            let state = it.next()?.to_string();
            if !valid_serial(&serial) {
                return None;
            }
            let mut model = String::new();
            let mut product = String::new();
            for kv in it {
                if let Some(v) = kv.strip_prefix("model:") {
                    model = v.replace('_', " ");
                } else if let Some(v) = kv.strip_prefix("product:") {
                    product = v.to_string();
                }
            }
            Some(PhoneDevice { is_wifi: serial.contains(':') || serial.contains("._adb-tls-"), serial, state, model, product })
        })
        .collect()
}

/// Physical size, or the override (`wm size 720x1600`) when one is set.
pub fn parse_wm_size(out: &str) -> Option<(u32, u32)> {
    let find = |key: &str| {
        out.lines().find_map(|l| {
            let rest = l.trim().strip_prefix(key)?;
            let (w, h) = rest.trim().split_once('x')?;
            Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
    };
    find("Override size:").or_else(|| find("Physical size:"))
}

pub fn parse_battery(out: &str) -> (Option<u32>, bool, Option<f32>) {
    let (mut level, mut charging, mut temp) = (None, false, None);
    for l in out.lines() {
        let l = l.trim();
        if let Some(v) = l.strip_prefix("level:") {
            level = v.trim().parse().ok();
        } else if let Some(v) = l.strip_prefix("temperature:") {
            temp = v.trim().parse::<f32>().ok().map(|t| t / 10.0);
        } else if matches!(l, "AC powered: true" | "USB powered: true" | "Wireless powered: true") {
            charging = true;
        }
    }
    (level, charging, temp)
}

/// Total and used KiB of the first data line of `df`.
pub fn parse_df(out: &str) -> (u64, u64) {
    out.lines()
        .skip(1)
        .find_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() >= 4 {
                Some((f[1].parse().ok()?, f[2].parse().ok()?))
            } else {
                None
            }
        })
        .unwrap_or((0, 0))
}

pub fn parse_ip(out: &str) -> String {
    out.lines()
        .find_map(|l| {
            let rest = l.trim().strip_prefix("inet ")?;
            Some(rest.split('/').next()?.trim().to_string())
        })
        .unwrap_or_default()
}

/// `package:com.x` lines → package names.
pub fn parse_packages(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out.lines().filter_map(|l| l.trim().strip_prefix("package:")).map(|p| p.trim().to_string()).filter(|p| valid_package(p)).collect();
    v.sort();
    v.dedup();
    v
}

/// Packages that own a launcher activity, from `cmd package query-activities --brief …`
/// (component lines look like `com.x/.MainActivity`).
pub fn parse_launcher_packages(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.contains(' ') || !l.contains('/') {
                return None;
            }
            Some(l.split('/').next()?.to_string())
        })
        .filter(|p| valid_package(p))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Best-effort label from a package name: the last meaningful segment, title-cased.
pub fn label_from_package(p: &str) -> String {
    const SKIP: &[&str] = &["android", "app", "apps", "mobile", "main", "free", "lite", "pro", "client", "official"];
    let seg = p.rsplit('.').find(|s| !SKIP.contains(&s.to_ascii_lowercase().as_str()) && s.len() > 1).unwrap_or_else(|| p.rsplit('.').next().unwrap_or(p));
    let spaced = seg.replace('_', " ");
    let mut c = spaced.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => p.to_string(),
    }
}

/// `ls -la` from toybox/mksh: `drwxrwx--x 3 u0_a1 ext_data_rw 4096 2026-10-16 09:12 name`.
pub fn parse_ls(out: &str) -> Vec<RemoteEntry> {
    let mut v: Vec<RemoteEntry> = out
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let kind = f.first()?.chars().next()?;
            if !matches!(kind, 'd' | '-' | 'l') || f.len() < 8 {
                return None;
            }
            // name may contain spaces → everything after the time column
            let date_idx = f.iter().position(|x| x.len() == 10 && x.as_bytes().get(4) == Some(&b'-') && x.as_bytes().get(7) == Some(&b'-'))?;
            let name_parts = &f[date_idx + 2..];
            if name_parts.is_empty() {
                return None;
            }
            let joined = name_parts.join(" ");
            let name = joined.split(" -> ").next()?.to_string();
            if name == "." || name == ".." {
                return None;
            }
            let size = f[date_idx - 1].parse().unwrap_or(0);
            Some(RemoteEntry { name, is_dir: kind == 'd' || (kind == 'l' && joined.ends_with('/')), size, modified: format!("{} {}", f[date_idx], f[date_idx + 1]) })
        })
        .collect();
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    v
}

fn clean_extra(v: &str) -> String {
    let mut s = v.trim().to_string();
    for suffix in [" (java.lang.String)", " (java.lang.CharSequence)", " (android.text.SpannableString)", " (android.text.SpannableStringBuilder)"] {
        if let Some(x) = s.strip_suffix(suffix) {
            s = x.to_string();
        }
    }
    for prefix in ["String (", "SpannableString (", "SpannableStringBuilder (", "CharSequence ("] {
        if let Some(x) = s.strip_prefix(prefix) {
            s = x.trim_end_matches(')').to_string();
        }
    }
    s
}

/// Best-effort parse of `dumpsys notification --noredact`. The text format
/// differs between Android versions, so unknown shapes yield fewer results
/// rather than errors.
pub fn parse_notifications(out: &str) -> Vec<PhoneNotification> {
    let mut v: Vec<PhoneNotification> = Vec::new();
    let mut cur: Option<PhoneNotification> = None;
    for l in out.lines() {
        let t = l.trim();
        if t.starts_with("NotificationRecord(") {
            if let Some(n) = cur.take() {
                if !n.title.is_empty() || !n.text.is_empty() {
                    v.push(n);
                }
            }
            let pkg = t.split_whitespace().find_map(|w| w.strip_prefix("pkg=")).unwrap_or("").to_string();
            cur = Some(PhoneNotification { package: pkg, title: String::new(), text: String::new() });
        } else if let Some(n) = cur.as_mut() {
            // extras may be on their own line or comma-joined inside `extras={…}`
            let t = t.trim_start_matches("extras={").trim_end_matches('}');
            for part in t.split(", android.") {
                let part = part.trim_start_matches("android.");
                if let Some(x) = part.strip_prefix("title=") {
                    if n.title.is_empty() {
                        n.title = clean_extra(x);
                    }
                } else if let Some(x) = part.strip_prefix("text=") {
                    if n.text.is_empty() {
                        n.text = clean_extra(x);
                    }
                }
            }
        }
    }
    if let Some(n) = cur {
        if !n.title.is_empty() || !n.text.is_empty() {
            v.push(n);
        }
    }
    v
}

// ───────────────────────── MJPEG frame splitting ─────────────────────────

/// Splits a concatenated JPEG byte stream (what `ffmpeg -f image2pipe -c:v mjpeg`
/// writes) into individual frames, tolerant of arbitrary chunk boundaries.
#[derive(Default)]
pub struct JpegSplitter {
    buf: Vec<u8>,
}

impl JpegSplitter {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(chunk);
        let mut frames = Vec::new();
        loop {
            let Some(start) = find_pair(&self.buf, 0, 0xFF, 0xD8) else {
                // keep a trailing 0xFF: it may be the first half of an SOI
                let keep = if self.buf.last() == Some(&0xFF) { 1 } else { 0 };
                let n = self.buf.len() - keep;
                self.buf.drain(..n);
                break;
            };
            let Some(end) = find_pair(&self.buf, start + 2, 0xFF, 0xD9) else {
                self.buf.drain(..start);
                break;
            };
            frames.push(self.buf[start..end + 2].to_vec());
            self.buf.drain(..end + 2);
        }
        // never let a corrupt stream grow without bound
        if self.buf.len() > 16 * 1024 * 1024 {
            self.buf.clear();
        }
        frames
    }
}

fn find_pair(buf: &[u8], from: usize, a: u8, b: u8) -> Option<usize> {
    if buf.len() < 2 || from >= buf.len() {
        return None;
    }
    (from..buf.len() - 1).find(|&i| buf[i] == a && buf[i + 1] == b)
}

// ───────────────────────── process plumbing ─────────────────────────

#[derive(Debug)]
pub struct Out {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub ok: bool,
}

/// Runs a command with a hard timeout (adb can hang on an offline device).
pub fn run_timeout(prog: &str, args: &[&str], timeout: Duration) -> Result<Out, String> {
    let mut child = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { format!("`{prog}` is not installed") } else { e.to_string() })?;
    let mut so = child.stdout.take().ok_or("no stdout")?;
    let mut se = child.stderr.take().ok_or("no stderr")?;
    let t1 = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = so.read_to_end(&mut v);
        v
    });
    let t2 = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = se.read_to_end(&mut v);
        v
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("`{prog}` timed out after {}s", timeout.as_secs()));
            }
            None => std::thread::sleep(Duration::from_millis(15)),
        }
    };
    Ok(Out { stdout: t1.join().unwrap_or_default(), stderr: String::from_utf8_lossy(&t2.join().unwrap_or_default()).to_string(), ok: status.success() })
}

const T: Duration = Duration::from_secs(15);

fn adb(serial: &str, args: &[&str]) -> Result<Out, String> {
    if !valid_serial(serial) {
        return Err("Invalid device id".into());
    }
    let mut full = vec!["-s", serial];
    full.extend_from_slice(args);
    run_timeout("adb", &full, T)
}

/// `adb -s S shell <line>` → stdout text; errors carry adb's own message.
fn shell(serial: &str, line: &str) -> Result<String, String> {
    let o = adb(serial, &["shell", line])?;
    if !o.ok && o.stdout.is_empty() {
        return Err(first_line(&o.stderr, "adb shell failed"));
    }
    Ok(String::from_utf8_lossy(&o.stdout).to_string())
}

fn first_line(s: &str, fallback: &str) -> String {
    s.lines().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or(fallback).to_string()
}

fn which(prog: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(prog).is_file())).unwrap_or(false)
}

// ───────────────────────────── commands ─────────────────────────────

#[tauri::command(async)]
pub fn phone_adb_status() -> AdbStatus {
    let v = run_timeout("adb", &["version"], Duration::from_secs(5)).ok();
    AdbStatus {
        installed: v.as_ref().map(|o| o.ok).unwrap_or(false),
        version: v.map(|o| first_line(&String::from_utf8_lossy(&o.stdout), "")).unwrap_or_default(),
        ffmpeg: which("ffmpeg"),
        scrcpy: which("scrcpy"),
    }
}

#[tauri::command(async)]
pub fn phone_devices() -> Result<Vec<PhoneDevice>, String> {
    let o = run_timeout("adb", &["devices", "-l"], Duration::from_secs(10))?;
    if !o.ok {
        return Err(first_line(&o.stderr, "adb failed"));
    }
    Ok(parse_devices(&String::from_utf8_lossy(&o.stdout)))
}

/// Android 11+ "Wireless debugging → Pair device with pairing code".
#[tauri::command(async)]
pub fn phone_pair(host_port: String, code: String) -> Result<String, String> {
    if !valid_host_port(&host_port) {
        return Err("Enter the pairing address as host:port, e.g. 192.168.1.20:37123".into());
    }
    if !valid_pair_code(&code) {
        return Err("The pairing code has 6 digits".into());
    }
    let o = run_timeout("adb", &["pair", &host_port, &code], Duration::from_secs(20))?;
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &o.stderr;
    if text.to_lowercase().contains("successfully paired") {
        Ok(first_line(&text, "Paired"))
    } else {
        Err(first_line(&text, "Pairing failed"))
    }
}

#[tauri::command(async)]
pub fn phone_connect(host_port: String) -> Result<String, String> {
    if !valid_host_port(&host_port) {
        return Err("Enter the address as host:port, e.g. 192.168.1.20:5555".into());
    }
    let o = run_timeout("adb", &["connect", &host_port], Duration::from_secs(15))?;
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &o.stderr;
    if text.contains("connected to") || text.contains("already connected") {
        Ok(first_line(&text, "Connected"))
    } else {
        Err(first_line(&text, "Could not connect"))
    }
}

#[tauri::command(async)]
pub fn phone_disconnect(serial: String) -> Result<(), String> {
    if !valid_serial(&serial) {
        return Err("Invalid device id".into());
    }
    phone_stream_stop(serial.clone());
    run_timeout("adb", &["disconnect", &serial], Duration::from_secs(8)).map(|_| ())
}

/// Switches a USB-connected phone to TCP/IP mode and connects to it over Wi-Fi.
#[tauri::command(async)]
pub fn phone_enable_wifi(serial: String) -> Result<String, String> {
    let ip = parse_ip(&shell(&serial, "ip -f inet addr show wlan0")?);
    if ip.is_empty() {
        return Err("The phone has no Wi-Fi address — connect it to the same network first".into());
    }
    let o = adb(&serial, &["tcpip", "5555"])?;
    if !o.ok {
        return Err(first_line(&o.stderr, "Could not enable TCP/IP mode"));
    }
    std::thread::sleep(Duration::from_millis(1200));
    phone_connect(format!("{ip}:5555"))
}

#[tauri::command(async)]
pub fn phone_info(serial: String) -> Result<PhoneInfo, String> {
    let props = shell(&serial, "getprop ro.product.manufacturer; getprop ro.product.model; getprop ro.build.version.release; getprop ro.build.version.sdk")?;
    let mut p = props.lines().map(|l| l.trim().to_string());
    let mut info = PhoneInfo {
        manufacturer: p.next().unwrap_or_default(),
        model: p.next().unwrap_or_default(),
        android: p.next().unwrap_or_default(),
        sdk: p.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        ..Default::default()
    };
    if let Ok(w) = shell(&serial, "wm size") {
        if let Some((a, b)) = parse_wm_size(&w) {
            info.screen_w = a;
            info.screen_h = b;
        }
    }
    if let Ok(b) = shell(&serial, "dumpsys battery") {
        let (l, c, t) = parse_battery(&b);
        info.battery_level = l;
        info.charging = c;
        info.battery_temp_c = t;
    }
    if let Ok(d) = shell(&serial, "df /data") {
        let (t, u) = parse_df(&d);
        info.storage_total_kb = t;
        info.storage_used_kb = u;
    }
    if let Ok(i) = shell(&serial, "ip -f inet addr show wlan0") {
        info.wifi_ip = parse_ip(&i);
    }
    if let Ok(w) = shell(&serial, "dumpsys window") {
        info.locked = w.contains("mDreamingLockscreen=true") || w.contains("isStatusBarKeyguard=true") || w.contains("mShowingLockscreen=true") || w.contains("showing=true");
    }
    Ok(info)
}

#[tauri::command(async)]
pub fn phone_apps(serial: String, include_system: bool) -> Result<Vec<PhoneApp>, String> {
    let launchers = shell(&serial, "cmd package query-activities --brief -a android.intent.action.MAIN -c android.intent.category.LAUNCHER").map(|o| parse_launcher_packages(&o)).unwrap_or_default();
    let mut pkgs = if include_system && !launchers.is_empty() {
        launchers
    } else {
        let third = parse_packages(&shell(&serial, "pm list packages -3")?);
        if launchers.is_empty() { third } else { third.into_iter().filter(|p| launchers.contains(p)).collect() }
    };
    pkgs.sort();
    Ok(pkgs.into_iter().map(|p| PhoneApp { label: label_from_package(&p), package: p }).collect())
}

#[tauri::command(async)]
pub fn phone_launch_app(serial: String, package: String) -> Result<(), String> {
    if !valid_package(&package) {
        return Err("Invalid package name".into());
    }
    let out = shell(&serial, &format!("monkey -p {} -c android.intent.category.LAUNCHER 1", shell_quote(&package)))?;
    if out.contains("No activities found") || out.contains("monkey aborted") {
        Err("This app has no launcher screen".into())
    } else {
        Ok(())
    }
}

#[tauri::command(async)]
pub fn phone_stop_app(serial: String, package: String) -> Result<(), String> {
    if !valid_package(&package) {
        return Err("Invalid package name".into());
    }
    shell(&serial, &format!("am force-stop {}", shell_quote(&package))).map(|_| ())
}

#[tauri::command(async)]
pub fn phone_uninstall_app(serial: String, package: String) -> Result<(), String> {
    if !valid_package(&package) {
        return Err("Invalid package name".into());
    }
    let out = shell(&serial, &format!("pm uninstall {}", shell_quote(&package)))?;
    if out.to_lowercase().contains("success") { Ok(()) } else { Err(first_line(&out, "Could not uninstall")) }
}

#[tauri::command(async)]
pub fn phone_install_apk(serial: String, path: String) -> Result<(), String> {
    if !path.to_lowercase().ends_with(".apk") || !std::path::Path::new(&path).is_file() {
        return Err("Choose an .apk file".into());
    }
    let o = run_timeout("adb", &["-s", &serial, "install", "-r", &path], Duration::from_secs(180))?;
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &o.stderr;
    if text.contains("Success") { Ok(()) } else { Err(first_line(&text, "Install failed")) }
}

// ── input ──

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PhoneInput {
    Tap { x: u32, y: u32 },
    LongPress { x: u32, y: u32 },
    Swipe { x1: u32, y1: u32, x2: u32, y2: u32, #[serde(default)] ms: Option<u32> },
    Key { name: String },
    Text { text: String },
}

/// The device-side shell line for an input event (pure → unit-tested).
pub fn input_line(i: &PhoneInput) -> Result<String, String> {
    const MAX: u32 = 10_000;
    Ok(match i {
        PhoneInput::Tap { x, y } if *x < MAX && *y < MAX => format!("input tap {x} {y}"),
        PhoneInput::LongPress { x, y } if *x < MAX && *y < MAX => format!("input swipe {x} {y} {x} {y} 800"),
        PhoneInput::Swipe { x1, y1, x2, y2, ms } if [x1, y1, x2, y2].iter().all(|v| **v < MAX) => {
            format!("input swipe {x1} {y1} {x2} {y2} {}", ms.unwrap_or(250).clamp(50, 3000))
        }
        PhoneInput::Key { name } => format!("input keyevent {}", keycode(name).ok_or_else(|| format!("Unknown key \"{name}\""))?),
        PhoneInput::Text { text } => format!("input text {}", escape_input_text(text)?),
        _ => return Err("Coordinates out of range".into()),
    })
}

#[tauri::command(async)]
pub fn phone_input(serial: String, input: PhoneInput) -> Result<(), String> {
    shell(&serial, &input_line(&input)?).map(|_| ())
}

/// Wakes the phone, dismisses the lock screen and (optionally) enters a PIN.
/// The PIN goes straight to `input text` and is never stored or logged.
#[tauri::command(async)]
pub fn phone_unlock(serial: String, pin: Option<String>, screen_w: u32, screen_h: u32) -> Result<(), String> {
    if let Some(p) = pin.as_deref() {
        if !(1..=16).contains(&p.len()) || !p.chars().all(|c| c.is_ascii_digit()) {
            return Err("The PIN may contain digits only".into());
        }
    }
    shell(&serial, "input keyevent 224")?;
    std::thread::sleep(Duration::from_millis(350));
    let _ = shell(&serial, "wm dismiss-keyguard");
    let (w, h) = (screen_w.clamp(100, 9000), screen_h.clamp(100, 9000));
    shell(&serial, &format!("input swipe {} {} {} {} 250", w / 2, h * 85 / 100, w / 2, h * 20 / 100))?;
    if let Some(p) = pin {
        std::thread::sleep(Duration::from_millis(500));
        shell(&serial, &format!("input text {p}"))?;
        shell(&serial, "input keyevent 66")?;
    }
    Ok(())
}

// ── calls, SMS, quick toggles ──

#[tauri::command(async)]
pub fn phone_dial(serial: String, number: String, place_call: bool) -> Result<(), String> {
    if !valid_phone_number(&number) {
        return Err("That does not look like a phone number".into());
    }
    let uri: String = number.trim().chars().filter(|c| *c != ' ').map(|c| if c == '#' { "%23".to_string() } else { c.to_string() }).collect();
    let action = if place_call { "android.intent.action.CALL" } else { "android.intent.action.DIAL" };
    shell(&serial, &format!("am start -a {action} -d {}", shell_quote(&format!("tel:{uri}")))).map(|_| ())
}

#[tauri::command(async)]
pub fn phone_call_control(serial: String, action: String) -> Result<(), String> {
    let key = match action.as_str() {
        "answer" => 5,
        "end" => 6,
        _ => return Err("Unknown call action".into()),
    };
    shell(&serial, &format!("input keyevent {key}")).map(|_| ())
}

/// Opens the SMS composer with the text pre-filled (the person taps Send on the phone).
#[tauri::command(async)]
pub fn phone_compose_sms(serial: String, number: String, body: String) -> Result<(), String> {
    if !valid_phone_number(&number) {
        return Err("That does not look like a phone number".into());
    }
    if body.len() > 1000 || body.contains('\0') {
        return Err("Message is too long".into());
    }
    let n: String = number.trim().chars().filter(|c| *c != ' ').collect();
    shell(&serial, &format!("am start -a android.intent.action.SENDTO -d {} --es sms_body {}", shell_quote(&format!("smsto:{n}")), shell_quote(&body))).map(|_| ())
}

#[tauri::command(async)]
pub fn phone_toggle(serial: String, what: String, enabled: bool) -> Result<(), String> {
    let on = if enabled { "enable" } else { "disable" };
    let line = match what.as_str() {
        "wifi" => format!("svc wifi {on}"),
        "data" => format!("svc data {on}"),
        "stay_awake" => format!("svc power stayon {}", if enabled { "true" } else { "false" }),
        "auto_rotate" => format!("settings put system accelerometer_rotation {}", if enabled { 1 } else { 0 }),
        _ => return Err("Unknown setting".into()),
    };
    shell(&serial, &line).map(|_| ())
}

#[tauri::command(async)]
pub fn phone_notifications(serial: String) -> Result<Vec<PhoneNotification>, String> {
    let out = shell(&serial, "dumpsys notification --noredact")?;
    let mut list = parse_notifications(&out);
    list.truncate(100);
    Ok(list)
}

// ── files ──

#[tauri::command(async)]
pub fn phone_ls(serial: String, path: String) -> Result<Vec<RemoteEntry>, String> {
    if !valid_remote_path(&path) {
        return Err("Invalid path".into());
    }
    let dir = if path.ends_with('/') { path } else { format!("{path}/") };
    let out = shell(&serial, &format!("ls -la {}", shell_quote(&dir)))?;
    if out.contains("Permission denied") && parse_ls(&out).is_empty() {
        return Err("Permission denied".into());
    }
    Ok(parse_ls(&out))
}

#[tauri::command(async)]
pub fn phone_pull(serial: String, remote: String, local_dir: String) -> Result<String, String> {
    if !valid_remote_path(&remote) || !std::path::Path::new(&local_dir).is_dir() {
        return Err("Invalid path".into());
    }
    let o = run_timeout("adb", &["-s", &serial, "pull", &remote, &local_dir], Duration::from_secs(600))?;
    if o.ok { Ok(first_line(&String::from_utf8_lossy(&o.stdout), "Done")) } else { Err(first_line(&o.stderr, "Pull failed")) }
}

#[tauri::command(async)]
pub fn phone_push(serial: String, local: String, remote_dir: String) -> Result<String, String> {
    if !valid_remote_path(&remote_dir) || !std::path::Path::new(&local).exists() {
        return Err("Invalid path".into());
    }
    let o = run_timeout("adb", &["-s", &serial, "push", &local, &remote_dir], Duration::from_secs(600))?;
    if o.ok { Ok(first_line(&String::from_utf8_lossy(&o.stdout), "Done")) } else { Err(first_line(&o.stderr, "Push failed")) }
}

/// Saves a PNG screenshot into `dest_dir`; returns the file path.
#[tauri::command(async)]
pub fn phone_screenshot(serial: String, dest_dir: String) -> Result<String, String> {
    if !valid_serial(&serial) || !std::path::Path::new(&dest_dir).is_dir() {
        return Err("Invalid destination".into());
    }
    let o = run_timeout("adb", &["-s", &serial, "exec-out", "screencap", "-p"], Duration::from_secs(20))?;
    if !o.ok || o.stdout.len() < 8 || &o.stdout[1..4] != b"PNG" {
        return Err("The phone did not return a screenshot (is the screen on?)".into());
    }
    let name = format!("Phone-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let path = std::path::Path::new(&dest_dir).join(name);
    std::fs::write(&path, &o.stdout).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Opens scrcpy (separate window, with audio) when it is installed.
#[tauri::command(async)]
pub fn phone_open_scrcpy(serial: String, turn_screen_off: bool, stay_awake: bool) -> Result<(), String> {
    if !valid_serial(&serial) {
        return Err("Invalid device id".into());
    }
    let mut args = vec!["--serial".to_string(), serial];
    if turn_screen_off {
        args.push("--turn-screen-off".into());
    }
    if stay_awake {
        args.push("--stay-awake".into());
    }
    Command::new("scrcpy")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { "scrcpy is not installed".to_string() } else { e.to_string() })
}

// ───────────────────────────── screen stream ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FramePayload {
    serial: String,
    /// `data:image/jpeg;base64,…` (or png in the fallback mode)
    src: String,
    fps_hint: u32,
}

fn streams() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static S: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn emit_frame(app: &AppHandle, serial: &str, mime: &str, bytes: &[u8], fps: u32) {
    let src = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
    let _ = app.emit("blue-phone://frame", FramePayload { serial: serial.to_string(), src, fps_hint: fps });
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// ffmpeg pipeline: screenrecord(H.264) → MJPEG. Runs until `stop` is set;
/// `screenrecord` ends itself every ~3 minutes, so the pipeline is restarted.
fn stream_h264(app: &AppHandle, serial: &str, size: &str, bitrate: &str, fps: u32, stop: &AtomicBool) -> Result<(), String> {
    while !stop.load(Ordering::SeqCst) {
        let mut rec = Command::new("adb")
            .args(["-s", serial, "exec-out", "screenrecord", "--output-format=h264", "--size", size, "--bit-rate", bitrate, "--time-limit", "170", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Cannot start adb: {e}"))?;
        let rec_out = rec.stdout.take().ok_or("no adb stdout")?;
        let fps_s = fps.to_string();
        let mut ff = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-fflags", "nobuffer", "-flags", "low_delay", "-f", "h264", "-i", "pipe:0", "-vf", &format!("fps={fps_s}"), "-q:v", "6", "-f", "image2pipe", "-c:v", "mjpeg", "pipe:1"])
            .stdin(Stdio::from(rec_out))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| { kill(&mut rec); format!("Cannot start ffmpeg: {e}") })?;
        let mut out = ff.stdout.take().ok_or("no ffmpeg stdout")?;
        let mut splitter = JpegSplitter::default();
        let mut buf = vec![0u8; 64 * 1024];
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let min_gap = Duration::from_millis((1000 / fps.max(1)).saturating_sub(5) as u64);
        let mut got_any = false;
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    // keep only the newest complete frame of this chunk (drop the rest if the UI lags)
                    if let Some(frame) = splitter.push(&buf[..n]).pop() {
                        got_any = true;
                        if last_emit.elapsed() >= min_gap {
                            emit_frame(app, serial, "image/jpeg", &frame, fps);
                            last_emit = Instant::now();
                        }
                    }
                }
            }
        }
        kill(&mut ff);
        kill(&mut rec);
        if !got_any && !stop.load(Ordering::SeqCst) {
            return Err("No picture from the phone — is the screen unlocked and USB debugging authorised?".into());
        }
    }
    Ok(())
}

/// Fallback without ffmpeg: poll `screencap` (PNG), ≈2–4 fps.
fn stream_screencap(app: &AppHandle, serial: &str, stop: &AtomicBool) -> Result<(), String> {
    let mut failures = 0;
    while !stop.load(Ordering::SeqCst) {
        match run_timeout("adb", &["-s", serial, "exec-out", "screencap", "-p"], Duration::from_secs(8)) {
            Ok(o) if o.ok && o.stdout.len() > 8 => {
                failures = 0;
                emit_frame(app, serial, "image/png", &o.stdout, 3);
            }
            Ok(_) | Err(_) => {
                failures += 1;
                if failures >= 5 {
                    return Err("The phone stopped answering".into());
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
    Ok(())
}

/// Starts streaming `serial`'s screen as `blue-phone://frame` events.
/// `max_side` limits the longer edge (e.g. 1280); `quality` is `low|medium|high`.
#[tauri::command(async)]
pub fn phone_stream_start(app: AppHandle, serial: String, max_side: u32, quality: String) -> Result<(), String> {
    if !valid_serial(&serial) {
        return Err("Invalid device id".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut map = streams().lock().map_err(|_| "poisoned")?;
        if let Some(old) = map.insert(serial.clone(), stop.clone()) {
            old.store(true, Ordering::SeqCst);
        }
    }
    let info = phone_info(serial.clone()).ok();
    let (w, h) = info.as_ref().map(|i| (i.screen_w.max(1), i.screen_h.max(1))).unwrap_or((1080, 1920));
    let scale = (max_side.clamp(360, 2400) as f64 / w.max(h) as f64).min(1.0);
    // H.264 encoders need even dimensions
    let (sw, sh) = (((w as f64 * scale) as u32 / 2) * 2, ((h as f64 * scale) as u32 / 2) * 2);
    let size = format!("{sw}x{sh}");
    let (bitrate, fps) = match quality.as_str() {
        "low" => ("2M", 15),
        "high" => ("12M", 30),
        _ => ("6M", 24),
    };
    let use_ffmpeg = which("ffmpeg");
    let id = serial.clone();
    std::thread::spawn(move || {
        let res = if use_ffmpeg { stream_h264(&app, &id, &size, bitrate, fps, &stop) } else { stream_screencap(&app, &id, &stop) };
        if let Err(e) = res {
            let _ = app.emit("blue-phone://stream-error", serde_json::json!({"serial": id, "message": e}));
        }
        let _ = app.emit("blue-phone://stream-ended", serde_json::json!({"serial": id}));
        if let Ok(mut m) = streams().lock() {
            if m.get(&id).map(|s| Arc::ptr_eq(s, &stop)).unwrap_or(false) {
                m.remove(&id);
            }
        }
    });
    Ok(())
}

#[tauri::command(async)]
pub fn phone_stream_stop(serial: String) {
    if let Ok(mut m) = streams().lock() {
        if let Some(s) = m.remove(&serial) {
            s.store(true, Ordering::SeqCst);
        }
    }
}

// ───────────────────────────── tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validators() {
        assert!(valid_serial("emulator-5554") && valid_serial("192.168.1.5:5555") && valid_serial("adb-XYZ-abc._adb-tls-connect._tcp"));
        assert!(!valid_serial("a b") && !valid_serial("a;rm") && !valid_serial("") && !valid_serial("$(x)"));
        assert!(valid_package("com.android.chrome") && valid_package("org.mozilla.firefox_beta"));
        assert!(!valid_package("chrome") && !valid_package("com.1bad") && !valid_package("com.x;id") && !valid_package("com.x y"));
        assert!(valid_host_port("192.168.1.20:5555") && valid_host_port("phone.local:37000") && valid_host_port("[fe80::1]:5555"));
        assert!(!valid_host_port("nope") && !valid_host_port("h:0") && !valid_host_port("h:99999") && !valid_host_port("a b:1") && !valid_host_port(":5555"));
        assert!(valid_pair_code("123456") && !valid_pair_code("12345") && !valid_pair_code("12345a"));
        assert!(valid_phone_number("+48 600 700 800") && valid_phone_number("*#06#") && valid_phone_number("(22) 555-1234"));
        assert!(!valid_phone_number("") && !valid_phone_number("abc") && !valid_phone_number("123;ls") && !valid_phone_number("+++"));
        assert!(valid_remote_path("/sdcard/DCIM") && !valid_remote_path("sdcard") && !valid_remote_path("/sdcard/../data") && !valid_remote_path("/a\nb"));
    }

    #[test]
    fn quoting_blocks_injection() {
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        let q = shell_quote("x'; rm -rf /; echo '");
        assert!(q.starts_with('\'') && q.ends_with('\''));
        // a quoted string, parsed back by a POSIX shell, is one word — emulate by checking no unquoted `;`
        let mut in_q = false;
        let mut unquoted_semicolon = false;
        let mut it = q.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                '\'' => in_q = !in_q,
                '\\' if !in_q => { it.next(); }
                ';' if !in_q => unquoted_semicolon = true,
                _ => {}
            }
        }
        assert!(!unquoted_semicolon);
    }

    #[test]
    fn input_text_escaping() {
        assert_eq!(escape_input_text("hello world").unwrap(), "hello%sworld");
        assert_eq!(escape_input_text("a&b").unwrap(), "a\\&b");
        assert_eq!(escape_input_text("50%").unwrap(), "50\\%");
        assert_eq!(escape_input_text("x;y").unwrap(), "x\\;y");
        assert_eq!(escape_input_text("a@b.com").unwrap(), "a@b.com");
        assert!(escape_input_text("zażółć").is_err());
        assert!(escape_input_text("a\nb").is_err());
        assert!(escape_input_text("").is_err());
        assert!(escape_input_text(&"a".repeat(501)).is_err());
        // no raw shell metacharacter may survive unescaped
        for bad in ["$(id)", "`id`", "a|b", "a>b", "a<b", "(a)", "a\"b", "a'b", "a*b", "a\\b", "a#b", "a~b"] {
            let e = escape_input_text(bad).unwrap();
            let mut chars = e.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\' { chars.next(); continue; }
                assert!(!"$`|><()\"'*#~&;{}[]\\".contains(c), "unescaped {c:?} in {e:?} (from {bad:?})");
            }
        }
    }

    #[test]
    fn input_lines() {
        assert_eq!(input_line(&PhoneInput::Tap { x: 10, y: 20 }).unwrap(), "input tap 10 20");
        assert_eq!(input_line(&PhoneInput::Swipe { x1: 1, y1: 2, x2: 3, y2: 4, ms: None }).unwrap(), "input swipe 1 2 3 4 250");
        assert_eq!(input_line(&PhoneInput::Swipe { x1: 1, y1: 2, x2: 3, y2: 4, ms: Some(99999) }).unwrap(), "input swipe 1 2 3 4 3000");
        assert_eq!(input_line(&PhoneInput::LongPress { x: 5, y: 6 }).unwrap(), "input swipe 5 6 5 6 800");
        assert_eq!(input_line(&PhoneInput::Key { name: "home".into() }).unwrap(), "input keyevent 3");
        assert!(input_line(&PhoneInput::Key { name: "nope".into() }).is_err());
        assert!(input_line(&PhoneInput::Tap { x: 99999, y: 1 }).is_err());
        assert_eq!(input_line(&PhoneInput::Text { text: "a b".into() }).unwrap(), "input text a%sb");
    }

    #[test]
    fn parses_devices() {
        let out = "List of devices attached\nR58M123ABC           device usb:1-2 product:beyond1 model:SM_G973F device:beyond1 transport_id:1\n192.168.1.7:5555       device product:sunfish model:Pixel_4a transport_id:2\nZX1G22         unauthorized usb:1-3 transport_id:3\n\n";
        let d = parse_devices(out);
        assert_eq!(d.len(), 3);
        assert_eq!((d[0].model.as_str(), d[0].is_wifi, d[0].state.as_str()), ("SM G973F", false, "device"));
        assert!(d[1].is_wifi);
        assert_eq!(d[2].state, "unauthorized");
        assert!(parse_devices("List of devices attached\n\n").is_empty());
        // daemon start-up noise before the header is ignored
        assert_eq!(parse_devices("* daemon not running; starting now\n* daemon started successfully\nList of devices attached\nabc device\n").len(), 1);
    }

    #[test]
    fn parses_device_facts() {
        assert_eq!(parse_wm_size("Physical size: 1080x2400\n"), Some((1080, 2400)));
        assert_eq!(parse_wm_size("Physical size: 1080x2400\nOverride size: 720x1600\n"), Some((720, 1600)));
        assert_eq!(parse_wm_size("garbage"), None);
        let b = "Current Battery Service state:\n  AC powered: false\n  USB powered: true\n  status: 2\n  level: 87\n  temperature: 312\n";
        assert_eq!(parse_battery(b), (Some(87), true, Some(31.2)));
        assert_eq!(parse_battery("level: 5\nAC powered: false\nUSB powered: false\n"), (Some(5), false, None));
        let df = "Filesystem       1K-blocks     Used Available Use% Mounted on\n/dev/block/dm-9  113000000 45000000  68000000  40% /data\n";
        assert_eq!(parse_df(df), (113000000, 45000000));
        assert_eq!(parse_ip("5: wlan0: <BROADCAST>\n    inet 192.168.1.42/24 brd 192.168.1.255 scope global wlan0\n"), "192.168.1.42");
        assert_eq!(parse_ip("nothing"), "");
    }

    #[test]
    fn parses_apps() {
        let pk = "package:com.spotify.music\npackage:org.telegram.messenger\npackage:com.spotify.music\npackage:bad name\n";
        assert_eq!(parse_packages(pk), vec!["com.spotify.music", "org.telegram.messenger"]);
        let q = "2 activities found:\n  priority=0 preferredOrder=0 match=0x108000 specificIndex=-1 isDefault=false\n  com.android.chrome/com.google.android.apps.chrome.Main\n  com.spotify.music/.MainActivity\n";
        assert_eq!(parse_launcher_packages(q), vec!["com.android.chrome", "com.spotify.music"]);
        assert_eq!(label_from_package("com.spotify.music"), "Music");
        assert_eq!(label_from_package("org.telegram.messenger"), "Messenger");
        assert_eq!(label_from_package("com.whatsapp"), "Whatsapp");
        assert_eq!(label_from_package("com.my_cool.app"), "My cool");
    }

    #[test]
    fn parses_ls() {
        let out = "total 40\ndrwxrwx--x 5 root sdcard_rw 4096 2026-10-16 09:12 DCIM\n-rw-rw---- 1 root sdcard_rw 123456 2026-10-15 18:30 My Photo.jpg\nlrwxrwxrwx 1 root root 12 2026-01-01 00:00 sdcard -> /storage/self/primary\ndrwxr-xr-x 2 root root 4096 2026-01-01 00:00 .\ndrwxr-xr-x 2 root root 4096 2026-01-01 00:00 ..\n";
        let e = parse_ls(out);
        assert_eq!(e.len(), 3);
        assert_eq!((e[0].name.as_str(), e[0].is_dir), ("DCIM", true));
        assert!(e.iter().any(|x| x.name == "My Photo.jpg" && x.size == 123456 && !x.is_dir));
        assert!(e.iter().any(|x| x.name == "sdcard"));
        assert!(parse_ls("ls: /x: Permission denied\n").is_empty());
    }

    #[test]
    fn parses_notifications_in_several_formats() {
        let old = "  NotificationRecord(0x1: pkg=com.whatsapp user=UserHandle{0} id=1)\n    extras={\n      android.title=Anna\n      android.text=Hi there\n    }\n  NotificationRecord(0x2: pkg=com.x.mail user=UserHandle{0} id=2)\n      android.title=Inbox (java.lang.String)\n";
        let n = parse_notifications(old);
        assert_eq!(n.len(), 2);
        assert_eq!((n[0].package.as_str(), n[0].title.as_str(), n[0].text.as_str()), ("com.whatsapp", "Anna", "Hi there"));
        assert_eq!(n[1].title, "Inbox");
        let new = "NotificationRecord(0x3: pkg=com.a.b user=UserHandle{0} id=3)\n  extras={android.title=String (Bob), android.text=String (Call me)}\n";
        let m = parse_notifications(new);
        assert_eq!((m[0].title.as_str(), m[0].text.as_str()), ("Bob", "Call me"));
        assert!(parse_notifications("nothing useful here").is_empty());
    }

    #[test]
    fn jpeg_splitter_handles_chunking() {
        let f1 = [0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9];
        let f2 = [0xFF, 0xD8, 9, 9, 0xFF, 0xD9];
        let mut stream = Vec::new();
        stream.extend_from_slice(&f1);
        stream.extend_from_slice(&f2);
        // all at once
        let mut s = JpegSplitter::default();
        assert_eq!(s.push(&stream), vec![f1.to_vec(), f2.to_vec()]);
        // byte by byte
        let mut s = JpegSplitter::default();
        let mut got = Vec::new();
        for b in &stream {
            got.extend(s.push(&[*b]));
        }
        assert_eq!(got, vec![f1.to_vec(), f2.to_vec()]);
        // garbage before the first SOI and a split marker
        let mut s = JpegSplitter::default();
        assert!(s.push(&[0, 0, 7, 0xFF]).is_empty());
        let out = s.push(&[0xD8, 5, 0xFF, 0xD9]);
        assert_eq!(out, vec![vec![0xFF, 0xD8, 5, 0xFF, 0xD9]]);
    }

    #[test]
    fn timeout_kills_hung_commands() {
        let t = Instant::now();
        let r = run_timeout("sleep", &["5"], Duration::from_millis(200));
        assert!(r.is_err());
        assert!(t.elapsed() < Duration::from_secs(2));
        let ok = run_timeout("echo", &["hi"], Duration::from_secs(2)).unwrap();
        assert!(ok.ok && String::from_utf8_lossy(&ok.stdout).trim() == "hi");
        assert!(run_timeout("definitely-not-a-program", &[], Duration::from_secs(1)).unwrap_err().contains("not installed"));
    }

    #[test]
    fn keycodes() {
        assert_eq!(keycode("back"), Some(4));
        assert_eq!(keycode("recents"), Some(187));
        assert_eq!(keycode("zzz"), None);
    }
}
