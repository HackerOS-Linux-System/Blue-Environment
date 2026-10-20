use rand::{rngs::OsRng, Rng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, Semaphore};

pub const REMOTE_PORT: u16 = 1718;
const REMOTE_PAGE: &str = include_str!("remote.html");
const MAX_HEADER: usize = 8192;
const MAX_BODY: usize = 4096;
const MAX_PAIRING_MINUTES: u64 = 10;
const COOKIE: &str = "blue_remote";

// ───────────────────────────── HTTP parsing ─────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum ParseErr {
    Incomplete,
    TooLarge,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

fn find_subslice(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// Parses one HTTP/1.x request from the start of `buf`. Returns the request
/// and the number of bytes consumed. Strict on purpose: only GET/POST, no
/// chunked bodies, bounded header and body sizes.
pub fn parse_request(buf: &[u8]) -> Result<(Req, usize), ParseErr> {
    let Some(pos) = find_subslice(buf, b"\r\n\r\n") else {
        return Err(if buf.len() > MAX_HEADER { ParseErr::TooLarge } else { ParseErr::Incomplete });
    };
    if pos > MAX_HEADER {
        return Err(ParseErr::TooLarge);
    }
    let head = std::str::from_utf8(&buf[..pos]).map_err(|_| ParseErr::Bad)?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().ok_or(ParseErr::Bad)?.split(' ');
    let (method, path, version) = (first.next(), first.next(), first.next());
    let (Some(method), Some(path), Some(version)) = (method, path, version) else { return Err(ParseErr::Bad) };
    if first.next().is_some() || !version.starts_with("HTTP/1.") || !matches!(method, "GET" | "POST") || !path.starts_with('/') || path.len() > 512 {
        return Err(ParseErr::Bad);
    }
    let mut headers = HashMap::new();
    for l in lines {
        if headers.len() >= 64 {
            return Err(ParseErr::TooLarge);
        }
        let (k, v) = l.split_once(':').ok_or(ParseErr::Bad)?;
        headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
    }
    if headers.contains_key("transfer-encoding") {
        return Err(ParseErr::Bad);
    }
    let len: usize = match headers.get("content-length") {
        Some(v) => v.parse().map_err(|_| ParseErr::Bad)?,
        None => 0,
    };
    if len > MAX_BODY {
        return Err(ParseErr::TooLarge);
    }
    let total = pos + 4 + len;
    if buf.len() < total {
        return Err(ParseErr::Incomplete);
    }
    let path = path.split('?').next().unwrap_or("/").to_string();
    Ok((Req { method: method.to_string(), path, headers, body: buf[pos + 4..total].to_vec() }, total))
}

pub fn http_response(status: u16, content_type: &str, body: &[u8], extra: &[(&str, String)]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let mut h = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nConnection: keep-alive\r\n",
        body.len()
    );
    for (k, v) in extra {
        h.push_str(&format!("{k}: {v}\r\n"));
    }
    h.push_str("\r\n");
    let mut out = h.into_bytes();
    out.extend_from_slice(body);
    out
}

fn json_response(status: u16, v: serde_json::Value, extra: &[(&str, String)]) -> Vec<u8> {
    http_response(status, "application/json", v.to_string().as_bytes(), extra)
}

/// Bearer token, else the `blue_remote` cookie.
pub fn token_from(req: &Req) -> Option<String> {
    if let Some(a) = req.headers.get("authorization") {
        if let Some(t) = a.strip_prefix("Bearer ") {
            return Some(t.trim().to_string());
        }
    }
    req.headers.get("cookie")?.split(';').find_map(|c| c.trim().strip_prefix(&format!("{COOKIE}=")).map(|v| v.to_string()))
}

/// Cross-site request check: when `Origin` is present its host must equal `Host`.
pub fn origin_ok(req: &Req) -> bool {
    match (req.headers.get("origin"), req.headers.get("host")) {
        (None, _) => true,
        (Some(o), Some(h)) => o.split("://").nth(1).map(|x| x == h).unwrap_or(false),
        (Some(_), None) => false,
    }
}

// ───────────────────────────── input model ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Input {
    Key { name: String },
    Text { text: String },
    Move { dx: i32, dy: i32 },
    Click { button: String },
    Scroll { dy: i32 },
    Volume { delta: i32 },
    Mute,
    Media { action: String },
}

const KEYS: &[&str] = &["up", "down", "left", "right", "select", "back", "home", "menu", "enter", "backspace", "tab", "space"];
const MEDIA: &[&str] = &["play_pause", "next", "prev", "stop"];

/// Validates and clamps. Unknown keys / actions are errors, numbers are clamped.
pub fn validate(i: &Input) -> Result<Input, String> {
    Ok(match i {
        Input::Key { name } if KEYS.contains(&name.as_str()) => i.clone(),
        Input::Key { .. } => return Err("unknown key".into()),
        Input::Text { text } => {
            let t: String = text.chars().filter(|c| !c.is_control()).take(200).collect();
            if t.is_empty() {
                return Err("empty text".into());
            }
            Input::Text { text: t }
        }
        Input::Move { dx, dy } => Input::Move { dx: (*dx).clamp(-300, 300), dy: (*dy).clamp(-300, 300) },
        Input::Click { button } if matches!(button.as_str(), "left" | "right" | "middle") => i.clone(),
        Input::Click { .. } => return Err("unknown button".into()),
        Input::Scroll { dy } => Input::Scroll { dy: (*dy).clamp(-10, 10) },
        Input::Volume { delta } => Input::Volume { delta: (*delta).clamp(-5, 5) },
        Input::Mute => Input::Mute,
        Input::Media { action } if MEDIA.contains(&action.as_str()) => i.clone(),
        Input::Media { .. } => return Err("unknown media action".into()),
    })
}

/// Linux `input-event-codes.h` for `ydotool key`.
pub fn evdev_code(name: &str) -> Option<u32> {
    Some(match name {
        "up" => 103,
        "down" => 108,
        "left" => 105,
        "right" => 106,
        "select" | "enter" => 28,
        "back" => 1,
        "home" => 102,
        "menu" => 139,
        "backspace" => 14,
        "tab" => 15,
        "space" => 57,
        _ => return None,
    })
}

/// `ydotool` argument vector for an input (system-wide injection, optional).
pub fn ydotool_args(i: &Input) -> Option<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    Some(match i {
        Input::Key { name } => {
            let c = evdev_code(name)?;
            vec!["key".into(), format!("{c}:1"), format!("{c}:0")]
        }
        Input::Text { text } => vec!["type".into(), "--".into(), text.clone()],
        Input::Move { dx, dy } => vec!["mousemove".into(), "-x".into(), dx.to_string(), "-y".into(), dy.to_string()],
        Input::Click { button } => s(&["click", match button.as_str() { "right" => "0xC1", "middle" => "0xC2", _ => "0xC0" }]),
        Input::Scroll { dy } => vec!["mousemove".into(), "-w".into(), "-x".into(), "0".into(), "-y".into(), (-dy).to_string()],
        _ => return None,
    })
}

// ───────────────────────────── pairing & state ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRecord {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub created: u64,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSettings {
    pub enabled: bool,
    /// Also inject keys/mouse system-wide through `ydotool` (for native apps).
    pub system_input: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pairing {
    pub code: String,
    pub expires_at: u64,
}

#[derive(Debug, Default)]
pub struct Guard {
    fails: u32,
    locked_until: u64,
}

impl Guard {
    pub fn locked(&self, now: u64) -> bool {
        now < self.locked_until
    }
    pub fn fail(&mut self, now: u64) {
        self.fails += 1;
        if self.fails >= 5 {
            self.locked_until = now + 60;
            self.fails = 0;
        }
    }
    pub fn success(&mut self) {
        self.fails = 0;
    }
}

#[derive(Debug, Default)]
pub struct State {
    pub settings: RemoteSettings,
    pub records: Vec<RemoteRecord>,
    pub pairing: Option<Pairing>,
    pub guard: Guard,
    pub running: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PairError {
    Locked,
    Closed,
    Wrong,
}

pub fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Constant-time equality of two strings (lengths are public).
pub fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn new_code() -> String {
    format!("{:06}", OsRng.gen_range(0..1_000_000u32))
}

pub fn new_token() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl State {
    pub fn open_pairing(&mut self, minutes: u64, now: u64) -> Pairing {
        let p = Pairing { code: new_code(), expires_at: now + minutes.clamp(1, MAX_PAIRING_MINUTES) * 60 };
        self.pairing = Some(p.clone());
        p
    }

    pub fn pairing_open(&self, now: u64) -> bool {
        self.pairing.as_ref().map(|p| now < p.expires_at).unwrap_or(false)
    }

    /// Returns the new record and the plain token (shown to the remote once).
    pub fn try_pair(&mut self, code: &str, name: &str, now: u64) -> Result<(RemoteRecord, String), PairError> {
        if self.guard.locked(now) {
            return Err(PairError::Locked);
        }
        let Some(p) = self.pairing.clone().filter(|p| now < p.expires_at) else { return Err(PairError::Closed) };
        if !ct_eq(&p.code, code.trim()) {
            self.guard.fail(now);
            return Err(PairError::Wrong);
        }
        self.guard.success();
        self.pairing = None; // one code, one device
        let token = new_token();
        let clean: String = name.chars().filter(|c| !c.is_control()).take(40).collect();
        let rec = RemoteRecord {
            id: new_token()[..16].to_string(),
            name: if clean.trim().is_empty() { "Remote".into() } else { clean.trim().to_string() },
            token_hash: sha256_hex(&token),
            created: now,
            last_seen: now,
        };
        self.records.push(rec.clone());
        Ok((rec, token))
    }

    pub fn authenticate(&mut self, token: &str, now: u64) -> Option<String> {
        let h = sha256_hex(token);
        let rec = self.records.iter_mut().find(|r| ct_eq(&r.token_hash, &h))?;
        rec.last_seen = now;
        Some(rec.id.clone())
    }

    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.records.len();
        self.records.retain(|r| r.id != id);
        self.records.len() != before
    }
}

// ───────────────────────────── persistence ─────────────────────────────

fn dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join(".config/Blue-Environment/blue-connect")
}

fn write_private(path: &std::path::Path, data: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, data).map_err(|e| e.to_string())?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())
}

fn load_json<T: for<'de> Deserialize<'de> + Default>(name: &str) -> T {
    std::fs::read_to_string(dir().join(name)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_state(st: &State) {
    let _ = write_private(&dir().join("remotes.json"), &serde_json::to_string_pretty(&st.records).unwrap_or_default());
    let _ = write_private(&dir().join("remote-settings.json"), &serde_json::to_string_pretty(&st.settings).unwrap_or_default());
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(State { settings: load_json("remote-settings.json"), records: load_json("remotes.json"), ..State::default() })
    })
}

fn stop_signal() -> &'static Arc<Notify> {
    static N: OnceLock<Arc<Notify>> = OnceLock::new();
    N.get_or_init(|| Arc::new(Notify::new()))
}

// ───────────────────────────── applying input ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteEvent {
    input: Input,
    /// ydotool already delivered it system-wide — the shell must not act on it again.
    handled_by_system: bool,
}

fn which(prog: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(prog).is_file())).unwrap_or(false)
}

fn spawn_quiet(prog: &str, args: &[String]) -> bool {
    Command::new(prog).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn volume(delta: i32) {
    let step = format!("{}%{}", delta.abs() * 5, if delta >= 0 { "+" } else { "-" });
    if which("wpctl") {
        spawn_quiet("wpctl", &["set-volume".into(), "-l".into(), "1.0".into(), "@DEFAULT_AUDIO_SINK@".into(), step]);
    } else if which("pactl") {
        spawn_quiet("pactl", &["set-sink-volume".into(), "@DEFAULT_SINK@".into(), format!("{}{}%", if delta >= 0 { "+" } else { "-" }, delta.abs() * 5)]);
    }
}

fn mute() {
    if which("wpctl") {
        spawn_quiet("wpctl", &["set-mute".into(), "@DEFAULT_AUDIO_SINK@".into(), "toggle".into()]);
    } else if which("pactl") {
        spawn_quiet("pactl", &["set-sink-mute".into(), "@DEFAULT_SINK@".into(), "toggle".into()]);
    }
}

fn media(action: &str) {
    let a = match action {
        "play_pause" => "play-pause",
        "next" => "next",
        "prev" => "previous",
        _ => "stop",
    };
    if which("playerctl") {
        spawn_quiet("playerctl", &[a.to_string()]);
    }
}

fn apply(app: &AppHandle, input: Input, system_input: bool) {
    match &input {
        Input::Volume { delta } => volume(*delta),
        Input::Mute => mute(),
        Input::Media { action } => media(action),
        _ => {}
    }
    let mut by_system = false;
    if system_input {
        if let Some(args) = ydotool_args(&input) {
            by_system = which("ydotool") && spawn_quiet("ydotool", &args);
        }
    }
    let _ = app.emit("blue-remote://input", RemoteEvent { input, handled_by_system: by_system });
}

// ───────────────────────────── routing ─────────────────────────────

fn hostname() -> String {
    hostname::get().ok().and_then(|h| h.into_string().ok()).unwrap_or_else(|| "Blue Environment".into())
}

pub fn route(app: &AppHandle, req: &Req) -> Vec<u8> {
    let now = now_secs();
    if !origin_ok(req) {
        return json_response(403, serde_json::json!({"error": "cross-site request"}), &[]);
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/remote") => http_response(
            200,
            "text/html; charset=utf-8",
            REMOTE_PAGE.as_bytes(),
            &[("Content-Security-Policy", "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; frame-ancestors 'none'".into())],
        ),
        ("GET", "/api/info") => {
            let mut st = state().lock().unwrap();
            let authorized = token_from(req).map(|t| st.authenticate(&t, now).is_some()).unwrap_or(false);
            json_response(200, serde_json::json!({"name": hostname(), "pairingOpen": st.pairing_open(now), "authorized": authorized}), &[])
        }
        ("POST", "/api/pair") => {
            #[derive(Deserialize)]
            struct P { code: String, #[serde(default)] name: String }
            let Ok(p) = serde_json::from_slice::<P>(&req.body) else { return json_response(400, serde_json::json!({"error": "bad request"}), &[]) };
            let mut st = state().lock().unwrap();
            match st.try_pair(&p.code, &p.name, now) {
                Ok((rec, token)) => {
                    save_state(&st);
                    drop(st);
                    let _ = app.emit("blue-remote://paired", serde_json::json!({"id": rec.id, "name": rec.name}));
                    let cookie = format!("{COOKIE}={token}; Path=/; Max-Age=31536000; SameSite=Strict; HttpOnly");
                    json_response(200, serde_json::json!({"token": token, "id": rec.id, "name": hostname()}), &[("Set-Cookie", cookie)])
                }
                Err(PairError::Locked) => json_response(429, serde_json::json!({"error": "locked"}), &[]),
                Err(PairError::Closed) => json_response(403, serde_json::json!({"error": "pairing closed"}), &[]),
                Err(PairError::Wrong) => json_response(401, serde_json::json!({"error": "wrong code"}), &[]),
            }
        }
        ("POST", "/api/input") => {
            let (ok, sys) = {
                let mut st = state().lock().unwrap();
                let ok = token_from(req).and_then(|t| st.authenticate(&t, now)).is_some();
                (ok, st.settings.system_input)
            };
            if !ok {
                return json_response(401, serde_json::json!({"error": "not paired"}), &[]);
            }
            let Ok(input) = serde_json::from_slice::<Input>(&req.body) else { return json_response(400, serde_json::json!({"error": "bad input"}), &[]) };
            match validate(&input) {
                Ok(v) => {
                    apply(app, v, sys);
                    json_response(200, serde_json::json!({"ok": true}), &[])
                }
                Err(e) => json_response(400, serde_json::json!({"error": e}), &[]),
            }
        }
        ("POST", "/api/unpair") => {
            let mut st = state().lock().unwrap();
            if let Some(t) = token_from(req) {
                let h = sha256_hex(&t);
                if let Some(id) = st.records.iter().find(|r| ct_eq(&r.token_hash, &h)).map(|r| r.id.clone()) {
                    st.revoke(&id);
                    save_state(&st);
                }
            }
            json_response(200, serde_json::json!({"ok": true}), &[("Set-Cookie", format!("{COOKIE}=; Path=/; Max-Age=0"))])
        }
        _ => json_response(404, serde_json::json!({"error": "not found"}), &[]),
    }
}

async fn handle_conn(mut sock: TcpStream, app: AppHandle) {
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut tmp = [0u8; 2048];
    loop {
        match parse_request(&buf) {
            Ok((req, used)) => {
                let close = req.headers.get("connection").map(|c| c.eq_ignore_ascii_case("close")).unwrap_or(false);
                let app2 = app.clone();
                let resp = tokio::task::spawn_blocking(move || route(&app2, &req)).await.unwrap_or_default();
                if sock.write_all(&resp).await.is_err() || close {
                    return;
                }
                buf.drain(..used);
                continue;
            }
            Err(ParseErr::Incomplete) => {}
            Err(e) => {
                let code = if e == ParseErr::TooLarge { 413 } else { 400 };
                let _ = sock.write_all(&json_response(code, serde_json::json!({"error": "bad request"}), &[])).await;
                return;
            }
        }
        match tokio::time::timeout(Duration::from_secs(20), sock.read(&mut tmp)).await {
            Ok(Ok(n)) if n > 0 => buf.extend_from_slice(&tmp[..n]),
            _ => return,
        }
        if buf.len() > MAX_HEADER + MAX_BODY + 16 {
            return;
        }
    }
}

async fn run_server(app: AppHandle) {
    let listener = match TcpListener::bind(("0.0.0.0", REMOTE_PORT)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[blue-remote] cannot listen on {REMOTE_PORT}: {e}");
            state().lock().unwrap().running = false;
            return;
        }
    };
    let permits = Arc::new(Semaphore::new(32));
    loop {
        tokio::select! {
            r = listener.accept() => {
                let Ok((sock, _)) = r else { continue };
                let Ok(permit) = permits.clone().try_acquire_owned() else { continue };
                let app = app.clone();
                tokio::spawn(async move { handle_conn(sock, app).await; drop(permit); });
            }
            _ = stop_signal().notified() => break,
        }
    }
    state().lock().unwrap().running = false;
}

fn ensure_running(app: &AppHandle) {
    let mut st = state().lock().unwrap();
    if st.running {
        return;
    }
    st.running = true;
    let app = app.clone();
    tauri::async_runtime::spawn(run_server(app));
}

/// Called once at start-up: restarts the server when it was enabled.
pub fn autostart(app: AppHandle) {
    if state().lock().unwrap().settings.enabled {
        ensure_running(&app);
    }
}

// ───────────────────────────── commands: receiver ─────────────────────────────

/// IPv4 addresses from `ip -4 -o addr show scope global`.
pub fn parse_ip_addrs(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            while let Some(w) = it.next() {
                if w == "inet" {
                    return it.next().map(|a| a.split('/').next().unwrap_or(a).to_string());
                }
            }
            None
        })
        .filter(|a| !a.starts_with("127."))
        .collect()
}

fn local_ips() -> Vec<String> {
    Command::new("ip").args(["-4", "-o", "addr", "show", "scope", "global"]).output().map(|o| parse_ip_addrs(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    pub enabled: bool,
    pub running: bool,
    pub system_input: bool,
    pub ydotool: bool,
    pub port: u16,
    pub urls: Vec<String>,
    pub pairing_open: bool,
    pub pairing_code: Option<String>,
    pub pairing_expires_at: Option<u64>,
    pub remotes: Vec<RemoteInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub last_seen: u64,
}

fn status_now() -> RemoteStatus {
    let st = state().lock().unwrap();
    let now = now_secs();
    let open = st.pairing_open(now);
    RemoteStatus {
        enabled: st.settings.enabled,
        running: st.running,
        system_input: st.settings.system_input,
        ydotool: which("ydotool"),
        port: REMOTE_PORT,
        urls: local_ips().into_iter().map(|ip| format!("http://{ip}:{REMOTE_PORT}")).collect(),
        pairing_open: open,
        pairing_code: if open { st.pairing.as_ref().map(|p| p.code.clone()) } else { None },
        pairing_expires_at: if open { st.pairing.as_ref().map(|p| p.expires_at) } else { None },
        remotes: st.records.iter().map(|r| RemoteInfo { id: r.id.clone(), name: r.name.clone(), created: r.created, last_seen: r.last_seen }).collect(),
    }
}

#[tauri::command(async)]
pub fn remote_server_status() -> RemoteStatus {
    status_now()
}

#[tauri::command(async)]
pub fn remote_set_enabled(app: AppHandle, enabled: bool, system_input: bool) -> RemoteStatus {
    {
        let mut st = state().lock().unwrap();
        st.settings = RemoteSettings { enabled, system_input };
        if !enabled {
            st.pairing = None;
        }
        save_state(&st);
    }
    if enabled {
        ensure_running(&app);
    } else {
        stop_signal().notify_waiters();
    }
    std::thread::sleep(Duration::from_millis(120));
    status_now()
}

/// Opens a pairing window (enables the receiver if needed) and returns the code to display.
#[tauri::command(async)]
pub fn remote_pairing_open(app: AppHandle, minutes: Option<u64>) -> RemoteStatus {
    {
        let mut st = state().lock().unwrap();
        st.settings.enabled = true;
        st.open_pairing(minutes.unwrap_or(5), now_secs());
        save_state(&st);
    }
    ensure_running(&app);
    std::thread::sleep(Duration::from_millis(120));
    status_now()
}

#[tauri::command(async)]
pub fn remote_pairing_close() -> RemoteStatus {
    state().lock().unwrap().pairing = None;
    status_now()
}

#[tauri::command(async)]
pub fn remote_revoke(id: String) -> RemoteStatus {
    {
        let mut st = state().lock().unwrap();
        st.revoke(&id);
        save_state(&st);
    }
    status_now()
}

// ───────────────────────────── controller side ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
}

pub fn valid_host(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
}

pub fn base_url(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') { format!("http://[{host}]:{port}") } else { format!("http://{host}:{port}") }
}

fn targets() -> Vec<Target> {
    load_json::<Vec<Target>>("remote-targets.json")
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(Duration::from_secs(4)).redirect(reqwest::redirect::Policy::none()).no_proxy().build().map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub name: String,
    pub pairing_open: bool,
}

/// Is a Blue remote receiver listening on `host`?
#[tauri::command]
pub async fn remote_probe(host: String, port: Option<u16>) -> Result<ProbeResult, String> {
    if !valid_host(&host) {
        return Err("Invalid address".into());
    }
    let url = format!("{}/api/info", base_url(&host, port.unwrap_or(REMOTE_PORT)));
    let r = client()?.get(url).send().await.map_err(|_| "No remote receiver found at that address".to_string())?;
    let v: serde_json::Value = r.json().await.map_err(|_| "Not a Blue remote receiver".to_string())?;
    Ok(ProbeResult { name: v.get("name").and_then(|n| n.as_str()).unwrap_or("TV").to_string(), pairing_open: v.get("pairingOpen").and_then(|b| b.as_bool()).unwrap_or(false) })
}

/// Pairs this device with a TV using the code shown on the TV and remembers the token.
#[tauri::command]
pub async fn remote_pair_with(host: String, port: Option<u16>, code: String) -> Result<Target, String> {
    if !valid_host(&host) {
        return Err("Invalid address".into());
    }
    let port = port.unwrap_or(REMOTE_PORT);
    let body = serde_json::json!({"code": code.trim(), "name": format!("Blue Connect on {}", hostname())});
    let r = client()?.post(format!("{}/api/pair", base_url(&host, port))).json(&body).send().await.map_err(|_| "Cannot reach the TV".to_string())?;
    match r.status().as_u16() {
        200 => {}
        401 => return Err("Wrong code".into()),
        403 => return Err("Pairing is not open on the TV — open Blue Connect → Remote → Pair a remote".into()),
        429 => return Err("Too many attempts — wait a minute".into()),
        s => return Err(format!("The TV answered HTTP {s}")),
    }
    let v: serde_json::Value = r.json().await.map_err(|e| e.to_string())?;
    let token = v.get("token").and_then(|t| t.as_str()).ok_or("The TV sent no token")?.to_string();
    let name = v.get("name").and_then(|t| t.as_str()).unwrap_or("TV").to_string();
    let target = Target { id: format!("{host}:{port}"), name, host, port, token };
    let mut all = targets();
    all.retain(|t| t.id != target.id);
    all.push(target.clone());
    write_private(&dir().join("remote-targets.json"), &serde_json::to_string_pretty(&all).unwrap_or_default())?;
    Ok(Target { token: String::new(), ..target })
}

#[tauri::command(async)]
pub fn remote_targets() -> Vec<Target> {
    targets().into_iter().map(|t| Target { token: String::new(), ..t }).collect()
}

#[tauri::command(async)]
pub fn remote_forget_target(id: String) -> Result<(), String> {
    let mut all = targets();
    all.retain(|t| t.id != id);
    write_private(&dir().join("remote-targets.json"), &serde_json::to_string_pretty(&all).unwrap_or_default())
}

/// Sends one input to a paired TV.
#[tauri::command]
pub async fn remote_send(target: String, input: Input) -> Result<(), String> {
    let t = targets().into_iter().find(|t| t.id == target).ok_or("Unknown TV — pair it first")?;
    let input = validate(&input)?;
    let r = client()?
        .post(format!("{}/api/input", base_url(&t.host, t.port)))
        .bearer_auth(&t.token)
        .json(&input)
        .send()
        .await
        .map_err(|_| "Cannot reach the TV".to_string())?;
    match r.status().as_u16() {
        200 => Ok(()),
        401 => Err("The TV no longer trusts this device — pair again".into()),
        s => Err(format!("The TV answered HTTP {s}")),
    }
}

// ───────────────────────────── tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn req(raw: &str) -> Result<(Req, usize), ParseErr> {
        parse_request(raw.as_bytes())
    }

    #[test]
    fn parses_get_and_post() {
        let raw = "GET /api/info?x=1 HTTP/1.1\r\nHost: tv:1718\r\nCookie: a=b; blue_remote=tok123\r\n\r\n";
        let (r, n) = req(raw).unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str()), ("GET", "/api/info"));
        assert_eq!(token_from(&r).as_deref(), Some("tok123"));
        assert_eq!(n, raw.len());
        let body = r#"{"type":"mute"}"#;
        let raw = format!("POST /api/input HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nAuthorization: Bearer abc\r\n\r\n{body}", body.len());
        let (r, n) = req(&raw).unwrap();
        assert_eq!(r.body, body.as_bytes());
        assert_eq!(n, raw.len());
        assert_eq!(token_from(&r).as_deref(), Some("abc"));
    }

    #[test]
    fn pipelined_requests_report_consumed_length() {
        let a = "GET / HTTP/1.1\r\nHost: x\r\n\r\n";
        let both = format!("{a}{a}");
        let (_, used) = parse_request(both.as_bytes()).unwrap();
        assert_eq!(used, a.len());
    }

    #[test]
    fn handles_partial_and_hostile_input() {
        assert_eq!(req("GET / HTTP/1.1\r\nHo").unwrap_err(), ParseErr::Incomplete);
        assert_eq!(req("POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc").unwrap_err(), ParseErr::Incomplete);
        assert_eq!(req("POST / HTTP/1.1\r\nContent-Length: 99999\r\n\r\n").unwrap_err(), ParseErr::TooLarge);
        assert_eq!(req("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap_err(), ParseErr::Bad);
        assert_eq!(req("DELETE / HTTP/1.1\r\n\r\n").unwrap_err(), ParseErr::Bad);
        assert_eq!(req("GET nopath HTTP/1.1\r\n\r\n").unwrap_err(), ParseErr::Bad);
        assert_eq!(req("GET / SPDY/3\r\n\r\n").unwrap_err(), ParseErr::Bad);
        assert_eq!(req("GET / HTTP/1.1 extra\r\n\r\n").unwrap_err(), ParseErr::Bad);
        assert_eq!(req("GET / HTTP/1.1\r\nbadheader\r\n\r\n").unwrap_err(), ParseErr::Bad);
        assert_eq!(req("POST / HTTP/1.1\r\nContent-Length: -1\r\n\r\n").unwrap_err(), ParseErr::Bad);
        let huge = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(9000));
        assert_eq!(req(&huge).unwrap_err(), ParseErr::TooLarge);
        assert_eq!(parse_request(&vec![b'a'; 9000]).unwrap_err(), ParseErr::TooLarge);
        assert_eq!(parse_request(&[0xff, 0xfe, b'\r', b'\n', b'\r', b'\n']).unwrap_err(), ParseErr::Bad);
    }

    #[test]
    fn origin_check() {
        let mk = |o: Option<&str>| {
            let mut h = HashMap::new();
            h.insert("host".to_string(), "192.168.1.5:1718".to_string());
            if let Some(o) = o { h.insert("origin".to_string(), o.to_string()); }
            Req { method: "POST".into(), path: "/".into(), headers: h, body: vec![] }
        };
        assert!(origin_ok(&mk(None)));
        assert!(origin_ok(&mk(Some("http://192.168.1.5:1718"))));
        assert!(!origin_ok(&mk(Some("http://evil.example"))));
        assert!(!origin_ok(&mk(Some("null"))));
    }

    #[test]
    fn response_is_well_formed() {
        let r = String::from_utf8(http_response(200, "text/plain", b"hi", &[("X-A", "b".into())])).unwrap();
        assert!(r.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(r.contains("Content-Length: 2\r\n") && r.contains("X-A: b\r\n") && r.contains("nosniff"));
        assert!(r.ends_with("\r\n\r\nhi"));
    }

    #[test]
    fn pairing_flow() {
        let mut st = State::default();
        let now = 1_000;
        // closed by default
        assert_eq!(st.try_pair("000000", "x", now).unwrap_err(), PairError::Closed);
        let p = st.open_pairing(5, now);
        assert_eq!(p.code.len(), 6);
        assert!(st.pairing_open(now) && !st.pairing_open(now + 301));
        // expired
        assert_eq!(st.try_pair(&p.code, "x", now + 301).unwrap_err(), PairError::Closed);
        // right code works once and consumes the pairing
        let (rec, token) = st.try_pair(&p.code, "Pixel \u{7} 7", now + 10).unwrap();
        assert_eq!(token.len(), 64);
        assert_eq!(rec.name, "Pixel  7");
        assert_ne!(rec.token_hash, token, "only the hash is stored");
        assert_eq!(rec.token_hash, sha256_hex(&token));
        assert!(!st.pairing_open(now + 11));
        assert_eq!(st.try_pair(&p.code, "y", now + 11).unwrap_err(), PairError::Closed);
        // the token authenticates; a wrong one doesn't
        assert_eq!(st.authenticate(&token, now + 20).as_deref(), Some(rec.id.as_str()));
        assert!(st.authenticate("nope", now).is_none());
        assert_eq!(st.records[0].last_seen, now + 20);
        // revoke
        assert!(st.revoke(&rec.id));
        assert!(st.authenticate(&token, now).is_none());
        assert!(!st.revoke("zzz"));
    }

    #[test]
    fn brute_force_is_locked_out() {
        let mut st = State::default();
        let now = 5_000;
        let p = st.open_pairing(10, now);
        let wrong = if p.code == "000000" { "111111" } else { "000000" };
        for _ in 0..5 {
            assert_eq!(st.try_pair(wrong, "x", now).unwrap_err(), PairError::Wrong);
        }
        // locked: even the RIGHT code is refused
        assert_eq!(st.try_pair(&p.code, "x", now + 1).unwrap_err(), PairError::Locked);
        assert_eq!(st.try_pair(&p.code, "x", now + 59).unwrap_err(), PairError::Locked);
        assert!(st.try_pair(&p.code, "x", now + 61).is_ok());
    }

    #[test]
    fn pairing_window_is_capped() {
        let mut st = State::default();
        let p = st.open_pairing(9999, 0);
        assert_eq!(p.expires_at, MAX_PAIRING_MINUTES * 60);
        assert_eq!(st.open_pairing(0, 0).expires_at, 60);
    }

    #[test]
    fn codes_and_tokens_are_random_and_formatted() {
        let c = new_code();
        assert_eq!(c.len(), 6);
        assert!(c.chars().all(|x| x.is_ascii_digit()));
        assert_ne!(new_token(), new_token());
        assert!(ct_eq("abc", "abc") && !ct_eq("abc", "abd") && !ct_eq("abc", "ab"));
    }

    #[test]
    fn input_validation() {
        let ok = |i: Input| validate(&i).unwrap();
        assert_eq!(ok(Input::Key { name: "up".into() }), Input::Key { name: "up".into() });
        assert!(validate(&Input::Key { name: "rm -rf".into() }).is_err());
        assert_eq!(ok(Input::Move { dx: 9999, dy: -9999 }), Input::Move { dx: 300, dy: -300 });
        assert_eq!(ok(Input::Scroll { dy: 99 }), Input::Scroll { dy: 10 });
        assert_eq!(ok(Input::Volume { delta: -50 }), Input::Volume { delta: -5 });
        assert_eq!(ok(Input::Text { text: "a\nb\u{0}c".into() }), Input::Text { text: "abc".into() });
        assert!(validate(&Input::Text { text: "\n\n".into() }).is_err());
        assert_eq!(ok(Input::Text { text: "x".repeat(500) }), Input::Text { text: "x".repeat(200) });
        assert!(validate(&Input::Click { button: "double".into() }).is_err());
        assert!(validate(&Input::Media { action: "format".into() }).is_err());
        assert!(validate(&Input::Media { action: "next".into() }).is_ok());
    }

    #[test]
    fn json_wire_format() {
        let i: Input = serde_json::from_str(r#"{"type":"key","name":"select"}"#).unwrap();
        assert_eq!(i, Input::Key { name: "select".into() });
        let m: Input = serde_json::from_str(r#"{"type":"move","dx":5,"dy":-3}"#).unwrap();
        assert_eq!(m, Input::Move { dx: 5, dy: -3 });
        assert_eq!(serde_json::from_str::<Input>(r#"{"type":"mute"}"#).unwrap(), Input::Mute);
        assert!(serde_json::from_str::<Input>(r#"{"type":"shell","cmd":"id"}"#).is_err());
    }

    #[test]
    fn ydotool_commands_are_argument_vectors() {
        assert_eq!(ydotool_args(&Input::Key { name: "select".into() }).unwrap(), vec!["key", "28:1", "28:0"]);
        assert_eq!(ydotool_args(&Input::Move { dx: 5, dy: -2 }).unwrap(), vec!["mousemove", "-x", "5", "-y", "-2"]);
        assert_eq!(ydotool_args(&Input::Click { button: "right".into() }).unwrap(), vec!["click", "0xC1"]);
        // text is passed after `--`, never interpreted as an option
        assert_eq!(ydotool_args(&Input::Text { text: "--help".into() }).unwrap(), vec!["type", "--", "--help"]);
        assert!(ydotool_args(&Input::Mute).is_none());
        assert_eq!(evdev_code("home"), Some(102));
        assert_eq!(evdev_code("zzz"), None);
    }

    #[test]
    fn local_addresses() {
        let out = "2: wlan0    inet 192.168.1.5/24 brd 192.168.1.255 scope global dynamic wlan0\n3: eth0    inet 10.0.0.7/8 scope global eth0\n1: lo    inet 127.0.0.1/8 scope host lo\n";
        assert_eq!(parse_ip_addrs(out), vec!["192.168.1.5", "10.0.0.7"]);
        assert!(parse_ip_addrs("").is_empty());
    }

    #[test]
    fn controller_helpers() {
        assert!(valid_host("192.168.1.5") && valid_host("tv.local") && valid_host("[fe80::1]"));
        assert!(!valid_host("a b") && !valid_host("x;y") && !valid_host("") && !valid_host("a/b"));
        assert_eq!(base_url("192.168.1.5", 1718), "http://192.168.1.5:1718");
        assert_eq!(base_url("fe80::1", 1718), "http://[fe80::1]:1718");
    }
}
