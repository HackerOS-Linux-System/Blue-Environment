pub mod clipboard;
pub mod labwc_config;
pub mod shell_ipc;
pub mod toplevels;

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};

/// Set (to `1`) in the environment of everything started by our own
/// `labwc -s` invocation.
pub const ENV_SESSION: &str = "BLUE_LABWC_SESSION";
/// Force a backend for one run (`hackeros-comp` | `labwc`), ignoring `config.hk`.
pub const ENV_BACKEND_OVERRIDE: &str = "BLUE_BACKEND";
/// Explicit path of the `config.hk` to use.
pub const ENV_CONFIG_HK: &str = "BLUE_CONFIG_HK";
/// CLI flag: internal — "you were started by labwc, run as the shell".
pub const ARG_LABWC_CHILD: &str = "--labwc-child";
/// CLI flag: start labwc even though a display session already exists
/// (nested, mainly for testing).
pub const ARG_START_BACKEND: &str = "--start-backend";
/// CLI flag: never start a compositor, just run the shell.
pub const ARG_NO_BACKEND: &str = "--no-backend";

// ── Events towards the frontend ─────────────────────────────────────────

pub type EventSink = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;
static SINK: OnceLock<EventSink> = OnceLock::new();

/// Registers how backend threads deliver events to the UI (main.rs wires
/// this to `AppHandle::emit`).
pub fn set_event_sink(sink: EventSink) {
    let _ = SINK.set(sink);
}

pub(crate) fn emit(event: &str, payload: serde_json::Value) {
    if let Some(sink) = SINK.get() {
        sink(event, payload);
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum BackendKind {
    #[serde(rename = "hackeros-comp")]
    HackerosComp,
    #[serde(rename = "labwc")]
    Labwc,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendKind::HackerosComp => "hackeros-comp",
            BackendKind::Labwc => "labwc",
        }
    }

    pub fn parse(s: &str) -> Option<BackendKind> {
        match s.trim().trim_matches('"').to_lowercase().replace('_', "-").as_str() {
            "hackeros-comp" | "hackeroscomp" | "hackeros" | "comp" | "classic" => Some(BackendKind::HackerosComp),
            "labwc" => Some(BackendKind::Labwc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub compositor: BackendKind,
    pub labwc_binary: String,
    pub labwc_args: Vec<String>,
    pub labwc_config_dir: Option<PathBuf>,
    pub generate_labwc_config: bool,
    /// The `config.hk` this was read from (or created at).
    pub source: PathBuf,
}

impl BackendConfig {
    fn defaults(source: PathBuf) -> Self {
        BackendConfig {
            compositor: BackendKind::HackerosComp,
            labwc_binary: "labwc".into(),
            labwc_args: Vec::new(),
            labwc_config_dir: None,
            generate_labwc_config: true,
            source,
        }
    }
}

// ── config.hk location + parsing ────────────────────────────────────────

const DEFAULT_CONFIG_HK: &str = "\
! Blue Environment — main configuration
! Format: HackerOS Configuration Format (.hk)  ·  '!' starts a comment
! File:   ~/.config/Blue-Environment/config.hk
!
! [backend] selects the compositor Blue Environment runs on:
!
!   compositor => hackeros-comp   Classic Blue backend (default). Started
!                                 exactly as before — nothing changes.
!   compositor => labwc           Run on labwc (`labwc -s`). If labwc isn't
!                                 installed Blue falls back to hackeros-comp.
!                                 HackerOS-Comp is NOT required in this mode.
!
! Optional (labwc only):
!   labwc_binary           => labwc      ! name or full path of the binary
!   labwc_args             =>            ! extra labwc arguments, space separated
!   labwc_config_dir       =>            ! custom labwc config dir (default ~/.config/labwc)
!   generate_labwc_config  => true       ! create a default labwc config if none exists

[backend]
-> compositor => hackeros-comp
-> labwc_binary => labwc
-> generate_labwc_config => true
";

/// Where `config.hk` lives (see the module docs for the search order).
/// Does not create anything.
pub fn config_hk_path() -> PathBuf {
    if let Ok(p) = std::env::var(ENV_CONFIG_HK) {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(cfg) = dirs::config_dir() {
        candidates.push(cfg.join("Blue-Environment/config.hk"));
    }
    candidates.push(home.join(".config/Blue-Environment/config.hk"));
    candidates.push(PathBuf::from("/etc/xdg/Blue-Environment/config.hk"));
    if let Some(found) = candidates.iter().find(|p| p.is_file()) {
        return found.clone();
    }
    // Nothing yet → the canonical default location.
    home.join(".config/Blue-Environment/config.hk")
}

/// Loads `[backend]` from `config.hk`, creating the file with defaults
/// first if it doesn't exist. Never fails — problems fall back to the
/// defaults (= classic hackeros-comp behaviour).
pub fn load_config() -> BackendConfig {
    let path = config_hk_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, DEFAULT_CONFIG_HK);
    }
    let mut cfg = BackendConfig::defaults(path.clone());
    match hk_parser::load_hk_file(&path) {
        Ok(mut hk) => {
            let _ = hk_parser::resolve_interpolations(&mut hk);
            if let Some(section) = hk.get("backend").and_then(|v| v.as_map().ok()) {
                let get = |key: &str| section.get(key).and_then(|v| v.as_string().ok());
                if let Some(raw) = get("compositor") {
                    match BackendKind::parse(&raw) {
                        Some(kind) => cfg.compositor = kind,
                        None => eprintln!("[blue-backend] unknown compositor '{raw}' in {} — using hackeros-comp", path.display()),
                    }
                }
                if let Some(bin) = get("labwc_binary").filter(|s| !s.trim().is_empty()) {
                    cfg.labwc_binary = bin.trim().to_string();
                }
                if let Some(args) = get("labwc_args") {
                    cfg.labwc_args = args.split_whitespace().map(String::from).collect();
                }
                if let Some(dir) = get("labwc_config_dir").filter(|s| !s.trim().is_empty()) {
                    cfg.labwc_config_dir = Some(expand_home(dir.trim()));
                }
                if let Some(v) = section.get("generate_labwc_config") {
                    if let Ok(b) = v.as_bool() {
                        cfg.generate_labwc_config = b;
                    } else if let Ok(s) = v.as_string() {
                        cfg.generate_labwc_config = !matches!(s.trim().to_lowercase().as_str(), "false" | "no" | "0" | "off");
                    }
                }
            }
        }
        Err(e) => eprintln!("[blue-backend] cannot parse {}: {e} — using defaults", path.display()),
    }
    if let Ok(over) = std::env::var(ENV_BACKEND_OVERRIDE) {
        if let Some(kind) = BackendKind::parse(&over) {
            cfg.compositor = kind;
        }
    }
    cfg
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        return dirs::home_dir().unwrap_or_default().join(rest);
    }
    PathBuf::from(p)
}

/// Sets `compositor` in `config.hk`, preserving comments and all other
/// content (plain text edit — hk-parser's writer would drop comments).
pub fn set_compositor(kind: BackendKind) -> Result<PathBuf, String> {
    let path = config_hk_path();
    if !path.exists() {
        let _ = load_config(); // creates it
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let updated = replace_compositor_line(&text, kind);
    std::fs::write(&path, updated).map_err(|e| e.to_string())?;
    Ok(path)
}

fn replace_compositor_line(text: &str, kind: BackendKind) -> String {
    let new_line = format!("-> compositor => {}", kind.as_str());
    let mut out: Vec<String> = Vec::new();
    let mut in_backend = false;
    let mut replaced = false;
    let mut section_seen = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            if in_backend && !replaced {
                out.push(new_line.clone());
                replaced = true;
            }
            in_backend = t.eq_ignore_ascii_case("[backend]");
            section_seen |= in_backend;
            out.push(line.to_string());
            continue;
        }
        if in_backend && t.starts_with("->") {
            let key = t.trim_start_matches("->").trim().split("=>").next().unwrap_or("").trim();
            if key == "compositor" {
                if !replaced {
                    out.push(new_line.clone());
                    replaced = true;
                }
                continue;
            }
        }
        out.push(line.to_string());
    }
    if !replaced {
        if !section_seen {
            if out.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                out.push(String::new());
            }
            out.push("[backend]".into());
        }
        out.push(new_line);
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

// ── Detection ───────────────────────────────────────────────────────────

pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    if bin.contains('/') {
        let p = PathBuf::from(bin);
        return p.is_file().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(bin)).find(|p| p.is_file())
}

fn hackeros_socket_exists() -> bool {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    Path::new(&runtime).join("hackeros-comp.sock").exists()
}

/// Which compositor is this shell *actually* running under? Cached.
pub fn active() -> BackendKind {
    static ACTIVE: OnceLock<BackendKind> = OnceLock::new();
    *ACTIVE.get_or_init(|| {
        if std::env::var(ENV_SESSION).map(|v| v == "1").unwrap_or(false) {
            return BackendKind::Labwc;
        }
        if std::env::args().any(|a| a == ARG_LABWC_CHILD) {
            return BackendKind::Labwc;
        }
        if let Ok(over) = std::env::var(ENV_BACKEND_OVERRIDE) {
            if let Some(k) = BackendKind::parse(&over) {
                return k;
            }
        }
        if hackeros_socket_exists() {
            return BackendKind::HackerosComp;
        }
        let desktop = format!(
            "{} {}",
            std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default()
        )
        .to_lowercase();
        if desktop.contains("labwc") {
            return BackendKind::Labwc;
        }
        // Unknown compositor: trust the configuration.
        load_config().compositor
    })
}

pub fn is_labwc() -> bool {
    active() == BackendKind::Labwc
}

// ── Start-up ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum StartupAction {
    /// Run the Tauri shell in this process.
    RunShell,
    /// Replace this process by `labwc -s …`.
    LaunchLabwc { binary: PathBuf, config: BackendConfig },
}

fn display_session_present() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

/// Decides what the classic `blue-environment` invocation should do.
pub fn plan_startup(args: &[String]) -> StartupAction {
    if args.iter().any(|a| a == ARG_NO_BACKEND) {
        return StartupAction::RunShell;
    }
    if args.iter().any(|a| a == ARG_LABWC_CHILD) {
        std::env::set_var(ENV_SESSION, "1");
        return StartupAction::RunShell;
    }
    if std::env::var(ENV_SESSION).map(|v| v == "1").unwrap_or(false) {
        return StartupAction::RunShell;
    }

    let cfg = load_config();
    if cfg.compositor != BackendKind::Labwc {
        return StartupAction::RunShell; // hackeros-comp: as usual, unchanged
    }
    let Some(binary) = find_in_path(&cfg.labwc_binary) else {
        eprintln!(
            "[blue-backend] backend is 'labwc' but '{}' was not found in PATH — falling back to the classic hackeros-comp behaviour",
            cfg.labwc_binary
        );
        return StartupAction::RunShell;
    };
    if display_session_present() && !args.iter().any(|a| a == ARG_START_BACKEND) {
        eprintln!("[blue-backend] a display session already exists — not nesting labwc (use {ARG_START_BACKEND} to force)");
        return StartupAction::RunShell;
    }
    StartupAction::LaunchLabwc { binary, config: cfg }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The string handed to `labwc -s`.
pub fn startup_command(exe: &Path, forward_args: &[String]) -> String {
    let mut cmd = format!("{} {}", shell_quote(&exe.to_string_lossy()), ARG_LABWC_CHILD);
    for a in forward_args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(a));
    }
    cmd
}

/// Generates the labwc config if needed and `exec`s labwc. Only returns
/// (with a message) if the exec failed, so the caller can fall back to
/// running the shell.
pub fn exec_labwc(binary: &Path, cfg: &BackendConfig, args: &[String]) -> String {
    if cfg.generate_labwc_config {
        let generated = labwc_config::ensure_default_config(cfg.labwc_config_dir.as_deref());
        if !generated.files.is_empty() {
            eprintln!(
                "[blue-backend] no prepared labwc config found — generated defaults: {}",
                generated.files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
            );
        }
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return format!("cannot determine own path: {e}"),
    };
    let forward: Vec<String> = args
        .iter()
        .filter(|a| !matches!(a.as_str(), ARG_START_BACKEND | ARG_NO_BACKEND | ARG_LABWC_CHILD))
        .cloned()
        .collect();

    let mut cmd = Command::new(binary);
    if let Some(dir) = &cfg.labwc_config_dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(&cfg.labwc_args).arg("-s").arg(startup_command(&exe, &forward));
    cmd.env(ENV_SESSION, "1");
    cmd.env("XDG_SESSION_TYPE", "wayland");
    if std::env::var_os("XDG_CURRENT_DESKTOP").is_none() {
        cmd.env("XDG_CURRENT_DESKTOP", "labwc:Blue");
    }
    // exec() only returns on failure.
    let err = cmd.exec();
    format!("exec {} failed: {err}", binary.display())
}

// ── Native compositor commands (labwc) ──────────────────────────────────

fn parse_id(payload: &serde_json::Value) -> Option<u64> {
    payload.get("id").and_then(|v| v.as_u64())
}

/// Executes the `CompositorBridge` command set (`focus_window`,
/// `close_window`, …) on labwc. Same command names as the hackeros-comp
/// IPC, so callers don't care which backend is underneath.
pub fn labwc_command(cmd_type: &str, payload: &serde_json::Value) -> Result<(), String> {
    use toplevels::Cmd;
    let list = toplevels::list();
    match cmd_type {
        "focus_window" | "restore_window" => {
            let id = parse_id(payload).ok_or("missing id")?;
            toplevels::send(Cmd::Activate(id));
            Ok(())
        }
        "close_window" | "kill_window" => {
            toplevels::send(Cmd::Close(parse_id(payload).ok_or("missing id")?));
            Ok(())
        }
        "minimize_window" => {
            toplevels::send(Cmd::SetMinimized(parse_id(payload).ok_or("missing id")?, true));
            Ok(())
        }
        "toggle_maximize" => {
            let id = parse_id(payload).ok_or("missing id")?;
            let now = list.iter().find(|t| t.id == id).map(|t| t.maximized).unwrap_or(false);
            toplevels::send(Cmd::SetMaximized(id, !now));
            Ok(())
        }
        "set_fullscreen" => {
            let id = parse_id(payload).ok_or("missing id")?;
            let on = payload.get("fullscreen").and_then(|v| v.as_bool()).unwrap_or(true);
            toplevels::send(Cmd::SetFullscreen(id, on));
            Ok(())
        }
        "tile_window" => {
            let id = parse_id(payload).ok_or("missing id")?;
            match payload.get("position").and_then(|v| v.as_str()) {
                Some("full") => {
                    toplevels::send(Cmd::SetMaximized(id, true));
                    Ok(())
                }
                Some("restore") => {
                    toplevels::send(Cmd::SetMaximized(id, false));
                    Ok(())
                }
                _ => Err("half-screen tiling is a labwc keybind (Super+Left/Right), not available over the toplevel protocol".into()),
            }
        }
        "get_window_list" => Ok(()), // list is pushed as `compositor:window-list` on every change
        "reload_config" => run_labwc_flag("-r"),
        "set_workspace_count" => {
            let count = payload.get("count").and_then(|v| v.as_u64()).unwrap_or(4).clamp(1, 16);
            set_labwc_desktop_count(count as u32)
        }
        "lock_screen" => {
            let _ = Command::new("sh")
                .arg("-c")
                .arg("loginctl lock-session || swaylock -f || gtklock")
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        "take_screenshot" => {
            let path = payload.get("path").and_then(|v| v.as_str()).ok_or("missing path")?.to_string();
            let status = Command::new("grim").arg(&path).status().map_err(|e| format!("grim: {e}"))?;
            if status.success() {
                emit("compositor:screenshot-ready", serde_json::json!({ "path": path }));
                Ok(())
            } else {
                Err("grim failed".into())
            }
        }
        "set_keyboard_layout" => {
            let layout = payload.get("layout").and_then(|v| v.as_str()).ok_or("missing layout")?;
            set_environment_var("XKB_DEFAULT_LAYOUT", layout)?;
            if let Some(variant) = payload.get("variant").and_then(|v| v.as_str()) {
                set_environment_var("XKB_DEFAULT_VARIANT", variant)?;
            }
            Ok(()) // labwc reads its `environment` file at start-up: applies at next login
        }
        "set_cursor" => {
            if let Some(theme) = payload.get("theme").and_then(|v| v.as_str()) {
                set_environment_var("XCURSOR_THEME", theme)?;
            }
            if let Some(size) = payload.get("size").and_then(|v| v.as_u64()) {
                set_environment_var("XCURSOR_SIZE", &size.to_string())?;
            }
            Ok(())
        }
        other => Err(format!("'{other}' is not supported by the labwc backend")),
    }
}

fn run_labwc_flag(flag: &str) -> Result<(), String> {
    let bin = find_in_path("labwc").ok_or("labwc not found")?;
    Command::new(bin).arg(flag).status().map_err(|e| e.to_string())?;
    Ok(())
}

/// Ends the labwc session (used for "Log out").
pub fn labwc_exit() {
    let _ = run_labwc_flag("-e");
}

fn labwc_dir() -> PathBuf {
    labwc_config::user_config_dir(load_config().labwc_config_dir.as_deref())
}

fn set_labwc_desktop_count(count: u32) -> Result<(), String> {
    let path = labwc_dir().join("rc.xml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (start, tag) = match text.find("<number>") {
        Some(s) => (s, "<number>"),
        None => return Err("no <number> element in rc.xml".into()),
    };
    let end = text[start..].find("</number>").ok_or("malformed rc.xml")? + start;
    let updated = format!("{}{}{}{}", &text[..start], tag, count, &text[end..]);
    std::fs::write(&path, updated).map_err(|e| e.to_string())?;
    run_labwc_flag("-r")
}

fn set_environment_var(key: &str, value: &str) -> Result<(), String> {
    if value.contains('\n') || value.contains('\r') {
        return Err("invalid value".into());
    }
    let path = labwc_dir().join("environment");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| {
            if l.trim_start().starts_with(&format!("{key}=")) {
                found = true;
                format!("{key}={value}")
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        lines.push(format!("{key}={value}"));
    }
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    std::fs::write(&path, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

/// Gives the shell's own toplevel keyboard focus (labwc). Used when a
/// modal text field opens while a native app still holds keyboard focus.
pub fn focus_shell_window() -> bool {
    if !toplevels::is_running() {
        return false;
    }
    match toplevels::list().iter().find(|t| toplevels::is_shell_window(t)) {
        Some(t) => {
            toplevels::send(toplevels::Cmd::Activate(t.id));
            true
        }
        None => false,
    }
}

#[derive(Debug, serde::Serialize)]
pub struct BackendInfo {
    pub configured: &'static str,
    pub active: &'static str,
    pub config_path: String,
    pub labwc_available: bool,
    pub hackeros_comp_available: bool,
    pub toplevel_tracking: bool,
    pub clipboard_watcher: bool,
}

pub fn info() -> BackendInfo {
    let cfg = load_config();
    BackendInfo {
        configured: cfg.compositor.as_str(),
        active: active().as_str(),
        config_path: cfg.source.display().to_string(),
        labwc_available: find_in_path(&cfg.labwc_binary).is_some(),
        hackeros_comp_available: find_in_path("hackeros-comp").is_some() || hackeros_socket_exists(),
        toplevel_tracking: toplevels::is_running(),
        clipboard_watcher: clipboard::is_available(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_backend_names() {
        assert_eq!(BackendKind::parse("labwc"), Some(BackendKind::Labwc));
        assert_eq!(BackendKind::parse(" LabWC "), Some(BackendKind::Labwc));
        assert_eq!(BackendKind::parse("hackeros-comp"), Some(BackendKind::HackerosComp));
        assert_eq!(BackendKind::parse("HackerOS_Comp"), Some(BackendKind::HackerosComp));
        assert_eq!(BackendKind::parse("sway"), None);
    }

    #[test]
    fn replaces_compositor_preserving_rest() {
        let text = "! hi\n[backend]\n-> compositor => hackeros-comp\n-> labwc_binary => labwc\n\n[other]\n-> compositor => x\n";
        let out = replace_compositor_line(text, BackendKind::Labwc);
        assert!(out.contains("[backend]\n-> compositor => labwc\n-> labwc_binary => labwc"));
        assert!(out.contains("[other]\n-> compositor => x"));
        assert!(out.starts_with("! hi"));
    }

    #[test]
    fn adds_missing_section_or_key() {
        let out = replace_compositor_line("[ui]\n-> a => b\n", BackendKind::Labwc);
        assert!(out.ends_with("[backend]\n-> compositor => labwc\n"));
        let out = replace_compositor_line("[backend]\n-> labwc_binary => x\n[ui]\n", BackendKind::Labwc);
        assert!(out.contains("-> labwc_binary => x\n-> compositor => labwc\n[ui]"));
    }

    #[test]
    fn startup_command_quotes_path() {
        let c = startup_command(Path::new("/opt/it's/blue"), &["--dev".into()]);
        assert_eq!(c, "'/opt/it'\\''s/blue' --labwc-child '--dev'");
    }

    #[test]
    fn default_file_is_valid_hk_and_classic() {
        let dir = std::env::temp_dir().join(format!("blue-hk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.hk");
        std::fs::write(&p, DEFAULT_CONFIG_HK).unwrap();
        let hk = hk_parser::load_hk_file(&p).unwrap();
        let backend = hk.get("backend").unwrap().as_map().unwrap();
        assert_eq!(backend.get("compositor").unwrap().as_string().unwrap(), "hackeros-comp");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
