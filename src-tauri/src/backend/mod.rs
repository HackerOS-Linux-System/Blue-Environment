pub mod clipboard;
pub mod labwc_config;
pub mod launcher;
pub mod shell_ipc;
pub mod sway_config;
pub mod toplevels;
pub mod wayfire_config;

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};

/// Set, in the environment of everything started by our own compositor
/// invocation, to which native backend is running (`labwc` | `sway` |
/// `wayfire`). Read back by [`active`] in the shell process. labwc also
/// happens to set `LABWC_PID` itself; the other two don't have an
/// equivalent, so Blue sets this one uniformly for all three instead of
/// relying on backend-specific behaviour.
pub const ENV_NATIVE_BACKEND: &str = "BLUE_NATIVE_BACKEND";
/// The compositor's own PID, set by Blue right before it execs into the
/// compositor. Needed because getppid() is not reliable across backends:
/// labwc execs the startup command directly (no intermediate shell), but
/// sway's `exec` and wayfire's `autostart` both run commands through `sh
/// -c`, so a shell process — not the compositor — would be our parent.
/// An inherited environment variable survives that indirection; a PPID
/// lookup does not.
pub const ENV_COMPOSITOR_PID: &str = "BLUE_COMPOSITOR_PID";
/// Force a backend for one run (`hackeros-comp` | `labwc` | `sway` |
/// `wayfire`), ignoring `config.hk`.
pub const ENV_BACKEND_OVERRIDE: &str = "BLUE_BACKEND";
/// Explicit path of the `config.hk` to use.
pub const ENV_CONFIG_HK: &str = "BLUE_CONFIG_HK";
/// CLI flag: internal — "you were started by labwc, run as the shell".
pub const ARG_LABWC_CHILD: &str = "--labwc-child";
/// CLI flag: internal — "you were started by sway, run as the shell".
pub const ARG_SWAY_CHILD: &str = "--sway-child";
/// CLI flag: internal — "you were started by wayfire, run as the shell".
pub const ARG_WAYFIRE_CHILD: &str = "--wayfire-child";
/// CLI flag: start the native backend even though a display session
/// already exists (nested, mainly for testing).
pub const ARG_START_BACKEND: &str = "--start-backend";
/// CLI flag: never start a compositor, just run the shell.
pub const ARG_NO_BACKEND: &str = "--no-backend";

/// The three native backends, in a fixed order used wherever code needs to
/// enumerate them (checking child flags, etc.).
const NATIVE_KINDS: [BackendKind; 3] = [BackendKind::Labwc, BackendKind::Sway, BackendKind::Wayfire];

/// The internal CLI flag that tells this process "you were started by
/// `kind` as its session client, run as the shell" — the single source of
/// truth [`active`] and [`plan_startup`] both build on.
fn child_flag(kind: BackendKind) -> Option<&'static str> {
    match kind {
        BackendKind::Labwc => Some(ARG_LABWC_CHILD),
        BackendKind::Sway => Some(ARG_SWAY_CHILD),
        BackendKind::Wayfire => Some(ARG_WAYFIRE_CHILD),
        BackendKind::HackerosComp => None,
    }
}

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
    #[serde(rename = "sway")]
    Sway,
    #[serde(rename = "wayfire")]
    Wayfire,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendKind::HackerosComp => "hackeros-comp",
            BackendKind::Labwc => "labwc",
            BackendKind::Sway => "sway",
            BackendKind::Wayfire => "wayfire",
        }
    }

    /// Whether this backend is a native wlroots compositor Blue drives
    /// itself (as opposed to `hackeros-comp`, which speaks its own IPC).
    /// labwc, sway and wayfire are all wlroots-based, so this is exactly
    /// the set of backends for which `toplevels`/`clipboard`/`shell_ipc`
    /// apply.
    pub fn is_native(&self) -> bool {
        !matches!(self, BackendKind::HackerosComp)
    }

    pub fn parse(s: &str) -> Option<BackendKind> {
        match s.trim().trim_matches('"').to_lowercase().replace(['_', '-'], "").as_str() {
            "hackeroscomp" | "hackeros" | "comp" | "classic" => Some(BackendKind::HackerosComp),
            "labwc" => Some(BackendKind::Labwc),
            "sway" | "swaywm" => Some(BackendKind::Sway),
            "wayfire" | "wayfirewm" => Some(BackendKind::Wayfire),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub compositor: BackendKind,
    /// Binary name/path, extra args and config dir for whichever
    /// `compositor` above is active. Looked up with the matching prefix
    /// (`labwc_*` / `sway_*` / `wayfire_*`) so a `config.hk` can carry
    /// settings for more than one backend at once (only the active one is
    /// used); labwc's keys are unprefixed-compatible with the very first
    /// shipped version of this feature, see [`load_config`].
    pub binary: String,
    pub args: Vec<String>,
    /// Directory holding the backend's config: `rc.xml` for labwc,
    /// `config` for sway, `wayfire.ini` for wayfire. `None` means "that
    /// backend's own default" (`~/.config/<name>/`).
    pub config_dir: Option<PathBuf>,
    pub generate_config: bool,
    /// The `config.hk` this was read from (or created at).
    pub source: PathBuf,
}

impl BackendConfig {
    fn defaults(source: PathBuf) -> Self {
        BackendConfig {
            compositor: BackendKind::HackerosComp,
            binary: String::new(),
            args: Vec::new(),
            config_dir: None,
            generate_config: true,
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
!   compositor => labwc           Run on labwc (`labwc -s`).
!   compositor => sway            Run on sway.
!   compositor => wayfire         Run on wayfire.
!
!                                 For labwc/sway/wayfire, HackerOS-Comp is
!                                 NOT required — Blue drives the compositor
!                                 directly over standard wlroots protocols.
!                                 Falls back to hackeros-comp if the chosen
!                                 compositor isn't installed.
!
! Optional, one set per backend (only the active one's settings are read;
! <name> is labwc, sway or wayfire):
!   <name>_binary          => <name>   ! name or full path of the binary
!   <name>_args            =>          ! extra arguments, space separated
!   <name>_config_dir      =>          ! custom config dir (default: that
!                                      ! backend's own, e.g. ~/.config/sway)
!   generate_<name>_config => true     ! create a default config if none exists

[backend]
-> compositor => hackeros-comp
-> labwc_binary => labwc
-> generate_labwc_config => true
-> sway_binary => sway
-> generate_sway_config => true
-> wayfire_binary => wayfire
-> generate_wayfire_config => true
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
                        None => eprintln!(
                            "[blue-backend] unknown compositor '{raw}' in {} — using hackeros-comp",
                            path.display()
                        ),
                    }
                }
                if cfg.compositor.is_native() {
                    let prefix = cfg.compositor.as_str();
                    cfg.binary = get(&format!("{prefix}_binary"))
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| prefix.to_string());
                    cfg.args = get(&format!("{prefix}_args"))
                        .map(|a| a.split_whitespace().map(String::from).collect())
                        .unwrap_or_default();
                    if let Some(dir) = get(&format!("{prefix}_config_dir")).filter(|s| !s.trim().is_empty()) {
                        cfg.config_dir = Some(expand_home(dir.trim()));
                    }
                    cfg.generate_config = section
                        .get(&format!("generate_{prefix}_config"))
                        .map(|v| {
                            v.as_bool().unwrap_or_else(|_| {
                                v.as_string()
                                    .map(|s| !matches!(s.trim().to_lowercase().as_str(), "false" | "no" | "0" | "off"))
                                    .unwrap_or(true)
                            })
                        })
                        .unwrap_or(true);
                }
            }
        }
        Err(e) => eprintln!("[blue-backend] cannot parse {}: {e} — using defaults", path.display()),
    }
    if let Ok(over) = std::env::var(ENV_BACKEND_OVERRIDE) {
        if let Some(kind) = BackendKind::parse(&over) {
            cfg.compositor = kind;
            if cfg.binary.is_empty() && kind.is_native() {
                cfg.binary = kind.as_str().to_string();
            }
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
        let args: Vec<String> = std::env::args().collect();
        for kind in NATIVE_KINDS {
            if let Some(flag) = child_flag(kind) {
                if args.iter().any(|a| a == flag) {
                    return kind;
                }
            }
        }
        if let Ok(marker) = std::env::var(ENV_NATIVE_BACKEND) {
            if let Some(k) = BackendKind::parse(&marker) {
                return k;
            }
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
        for (needle, kind) in [("labwc", BackendKind::Labwc), ("sway", BackendKind::Sway), ("wayfire", BackendKind::Wayfire)] {
            if desktop.contains(needle) {
                return kind;
            }
        }
        // Unknown compositor: trust the configuration.
        load_config().compositor
    })
}

pub fn is_native() -> bool {
    active().is_native()
}

/// The running compositor's real PID — for labwc, that's the `LABWC_PID`
/// labwc sets in its own environment (and which is therefore already
/// inherited by us); for sway and wayfire, the `BLUE_COMPOSITOR_PID` Blue
/// itself set right before exec-ing into them (see [`exec_native`] — both
/// run their startup client through `sh -c`, so `getppid()` would only
/// give us that intermediate shell, not the compositor).
fn compositor_pid() -> Option<i32> {
    std::env::var("LABWC_PID")
        .ok()
        .or_else(|| std::env::var(ENV_COMPOSITOR_PID).ok())
        .and_then(|s| s.trim().parse().ok())
}

// ── Start-up ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum StartupAction {
    /// Run the Tauri shell in this process.
    RunShell,
    /// Replace this process by the given native compositor.
    LaunchNative { kind: BackendKind, binary: PathBuf, config: BackendConfig },
}

fn display_session_present() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

/// Decides what the classic `blue-environment` invocation should do.
pub fn plan_startup(args: &[String]) -> StartupAction {
    if args.iter().any(|a| a == ARG_NO_BACKEND) {
        return StartupAction::RunShell;
    }
    for kind in NATIVE_KINDS {
        if let Some(flag) = child_flag(kind) {
            if args.iter().any(|a| a == flag) {
                std::env::set_var(ENV_NATIVE_BACKEND, kind.as_str());
                return StartupAction::RunShell;
            }
        }
    }
    if let Ok(marker) = std::env::var(ENV_NATIVE_BACKEND) {
        if BackendKind::parse(&marker).is_some() {
            return StartupAction::RunShell;
        }
    }

    let cfg = load_config();
    if !cfg.compositor.is_native() {
        return StartupAction::RunShell; // hackeros-comp: as usual, unchanged
    }
    let Some(binary) = find_in_path(&cfg.binary) else {
        eprintln!(
            "[blue-backend] backend is '{}' but '{}' was not found in PATH — falling back to the classic hackeros-comp behaviour",
            cfg.compositor.as_str(),
            cfg.binary
        );
        return StartupAction::RunShell;
    };
    if display_session_present() && !args.iter().any(|a| a == ARG_START_BACKEND) {
        eprintln!("[blue-backend] a display session already exists — not nesting a compositor (use {ARG_START_BACKEND} to force)");
        return StartupAction::RunShell;
    }
    StartupAction::LaunchNative { kind: cfg.compositor, binary, config: cfg }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The string handed to `labwc -s` (labwc only — sway/wayfire name their
/// startup client from inside their own config, see their `*_config`
/// modules).
fn startup_command(exe: &Path, forward_args: &[String]) -> String {
    let mut cmd = format!("{} {}", shell_quote(&exe.to_string_lossy()), ARG_LABWC_CHILD);
    for a in forward_args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(a));
    }
    cmd
}

/// A single space-separated command string suitable for a config's `exec`
/// line (sway) or `[autostart]` entry (wayfire) — same quoting as
/// [`startup_command`], different trailing flag.
fn autostart_command(exe: &Path, child_flag: &str, forward_args: &[String]) -> String {
    let mut cmd = format!("{} {}", shell_quote(&exe.to_string_lossy()), child_flag);
    for a in forward_args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(a));
    }
    cmd
}

/// Ensures a session D-Bus is reachable before starting a compositor from a
/// bare TTY (most modern apps — GTK4/libadwaita, portals, file choosers —
/// need one, and unlike a display manager, a TTY login often doesn't start
/// one). Returns a wrapper binary to run the compositor through
/// (`dbus-run-session -- <compositor> ...`) when there's no session bus and
/// no way to point at an existing one; otherwise `None` (nothing needed, or
/// the address is set as an env var instead).
fn dbus_wrap_if_needed(cmd: &mut Command) -> bool {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        return false;
    }
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    let bus = Path::new(&runtime).join("bus");
    if bus.exists() {
        cmd.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", bus.display()));
        return false;
    }
    find_in_path("dbus-run-session").is_some()
}

/// Generates the chosen backend's config if needed and `exec`s it. Only
/// returns (with a message) if the exec failed, so the caller can fall
/// back to running the shell.
pub fn exec_native(kind: BackendKind, binary: &Path, cfg: &BackendConfig, args: &[String]) -> String {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return format!("cannot determine own path: {e}"),
    };
    let forward: Vec<String> = args
        .iter()
        .filter(|a| {
            !matches!(
                a.as_str(),
                ARG_START_BACKEND | ARG_NO_BACKEND | ARG_LABWC_CHILD | ARG_SWAY_CHILD | ARG_WAYFIRE_CHILD
            )
        })
        .cloned()
        .collect();

    let config_path_for_log;
    let mut cmd = Command::new(binary);
    match kind {
        BackendKind::Labwc => {
            if cfg.generate_config {
                let generated = labwc_config::ensure_default_config(cfg.config_dir.as_deref());
                log_generated(&generated.files);
            }
            if let Some(dir) = &cfg.config_dir {
                cmd.arg("-C").arg(dir);
            }
            config_path_for_log = cfg.config_dir.clone();
            cmd.args(&cfg.args).arg("-s").arg(startup_command(&exe, &forward));
        }
        BackendKind::Sway => {
            let child_cmd = autostart_command(&exe, ARG_SWAY_CHILD, &forward);
            let result = sway_config::ensure_startable_config(cfg.config_dir.as_deref(), cfg.generate_config, &child_cmd);
            let path = match result {
                Ok(r) => {
                    log_generated(&r.generated_files);
                    r.launch_config_path
                }
                Err(e) => return format!("could not prepare a sway config: {e}"),
            };
            config_path_for_log = Some(path.clone());
            cmd.args(&cfg.args).arg("-c").arg(&path);
        }
        BackendKind::Wayfire => {
            let child_cmd = autostart_command(&exe, ARG_WAYFIRE_CHILD, &forward);
            let result = wayfire_config::ensure_startable_config(cfg.config_dir.as_deref(), cfg.generate_config, &child_cmd);
            let path = match result {
                Ok(r) => {
                    log_generated(&r.generated_files);
                    r.launch_config_path
                }
                Err(e) => return format!("could not prepare a wayfire config: {e}"),
            };
            config_path_for_log = Some(path.clone());
            cmd.args(&cfg.args).arg("-c").arg(&path);
        }
        BackendKind::HackerosComp => unreachable!("exec_native is only called for native backends"),
    }
    let _ = &config_path_for_log;

    if dbus_wrap_if_needed(&mut cmd) {
        // Re-home the whole invocation under dbus-run-session, which forks
        // its own bus then execs the given command with the address set.
        let mut wrapped = Command::new("dbus-run-session");
        wrapped.arg("--");
        wrapped.arg(binary);
        wrapped.args(cmd.get_args());
        for (k, v) in cmd.get_envs() {
            if let Some(v) = v {
                wrapped.env(k, v);
            }
        }
        cmd = wrapped;
    }

    cmd.env(ENV_NATIVE_BACKEND, kind.as_str());
    // Own PID, before we vanish into the compositor: see ENV_COMPOSITOR_PID.
    cmd.env(ENV_COMPOSITOR_PID, std::process::id().to_string());
    cmd.env("XDG_SESSION_TYPE", "wayland");
    if std::env::var_os("XDG_CURRENT_DESKTOP").is_none() {
        cmd.env("XDG_CURRENT_DESKTOP", format!("{}:Blue", kind.as_str()));
    }
    // exec() only returns on failure.
    let err = cmd.exec();
    format!("exec {} failed: {err}", binary.display())
}

fn log_generated(files: &[PathBuf]) {
    if !files.is_empty() {
        eprintln!(
            "[blue-backend] no prepared config found — generated defaults: {}",
            files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        );
    }
}

// ── Native compositor commands (labwc / sway / wayfire) ─────────────────

fn parse_id(payload: &serde_json::Value) -> Option<u64> {
    payload.get("id").and_then(|v| v.as_u64())
}

/// Executes the `CompositorBridge` command set (`focus_window`,
/// `close_window`, …) on the active native backend. Same command names as
/// the hackeros-comp IPC, so callers don't care which backend is
/// underneath. Everything here that touches windows goes through
/// `toplevels` (the generic `wlr-foreign-toplevel-management` client), so
/// it is identical for labwc, sway and wayfire; only the handful of
/// functions right at the bottom of this section (reload/exit/keyboard
/// layout/cursor) branch per backend, because those have no standard
/// protocol and each compositor exposes them differently (or not at all).
pub fn native_command(cmd_type: &str, payload: &serde_json::Value) -> Result<(), String> {
    use toplevels::Cmd;
    let list = toplevels::list();
    match cmd_type {
        "focus_window" | "restore_window" => {
            let id = parse_id(payload).ok_or("missing id")?;
            if let Ok(mut p) = PEEK.lock() {
                if let Some(peek) = p.as_mut() {
                    peek.target = Some(id);
                }
            }
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
                _ => Err("half-screen tiling has no equivalent over the toplevel protocol on this backend — use a keybind (Super+Left/Right) instead".into()),
            }
        }
        "get_window_list" => Ok(()), // list is pushed as `compositor:window-list` on every change
        "raise_shell" | "show_desktop" => {
            // A permanent "go to the desktop": supersedes any temporary peek.
            let _ = PEEK.lock().map(|mut p| p.take());
            bump_epoch();
            for t in list.iter().filter(|t| !toplevels::is_shell_window(t) && !t.minimized) {
                toplevels::send(Cmd::SetMinimized(t.id, true));
            }
            focus_shell_window();
            Ok(())
        }
        // Shell overlays (Start menu, Control Center, Alt+Tab switcher…) are
        // drawn by the shell, which sits *below* native windows on every
        // native backend — so a maximized app would hide them completely.
        // While one is open, native windows are tucked away and brought
        // back afterwards.
        "overlay_open" => {
            overlay_open();
            Ok(())
        }
        "overlay_close" => {
            overlay_close();
            Ok(())
        }
        "reload_config" => reload_compositor(),
        "set_workspace_count" => set_workspace_count(payload),
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
        "set_keyboard_layout" => set_keyboard_layout(payload),
        "set_cursor" => set_cursor(payload),
        other => Err(format!("'{other}' is not supported by the {} backend", active().as_str())),
    }
}

/// Reloads the running compositor's configuration.
///
/// * **sway** exposes a real IPC command for this (`swaymsg reload`), which
///   applies immediately and is used as-is.
/// * **labwc** reconfigures on `SIGHUP` (the same thing its own `-r` flag
///   does internally) — sent directly to [`compositor_pid`] rather than
///   shelling out to `labwc -r`, since that avoids a second process and
///   works identically whether or not `labwc` is even on `$PATH` anymore.
/// * **wayfire** has no external reload trigger in this version (verified
///   against 0.9.0's source: no `SIGHUP` handler, no reload IPC method) —
///   settings changes on wayfire currently need a fresh session.
fn reload_compositor() -> Result<(), String> {
    match active() {
        BackendKind::Sway => run_swaymsg(&["reload"]),
        BackendKind::Labwc => send_signal(libc::SIGHUP),
        BackendKind::Wayfire => Err("wayfire has no live config-reload in this version — changes apply on the next login".into()),
        BackendKind::HackerosComp => Err("reload_config is not applicable to hackeros-comp".into()),
    }
}

/// Ends the compositor session (used for "Log out"). sway again gets the
/// real IPC command; labwc and wayfire both handle `SIGTERM` as a clean
/// shutdown (confirmed for wayfire directly in its `main.cpp`: `SIGTERM`
/// calls `core.shutdown()`).
pub fn exit_compositor() {
    match active() {
        BackendKind::Sway => {
            let _ = run_swaymsg(&["exit"]);
        }
        BackendKind::Labwc | BackendKind::Wayfire => {
            let _ = send_signal(libc::SIGTERM);
        }
        BackendKind::HackerosComp => {}
    }
}

fn send_signal(sig: i32) -> Result<(), String> {
    let pid = compositor_pid().ok_or("compositor PID is not known (unexpected — please file a bug)")?;
    let rc = unsafe { libc::kill(pid, sig) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

fn run_swaymsg(args: &[&str]) -> Result<(), String> {
    let bin = find_in_path("swaymsg").ok_or("swaymsg not found (it ships with sway)")?;
    let status = Command::new(bin).args(args).status().map_err(|e| e.to_string())?;
    status.success().then_some(()).ok_or_else(|| format!("swaymsg {} failed", args.join(" ")))
}

fn set_workspace_count(payload: &serde_json::Value) -> Result<(), String> {
    match active() {
        BackendKind::Labwc => {
            let count = payload.get("count").and_then(|v| v.as_u64()).unwrap_or(4).clamp(1, 16);
            let path = labwc_config::user_config_dir(load_config().config_dir.as_deref()).join("rc.xml");
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let start = text.find("<number>").ok_or("no <number> element in rc.xml")?;
            let end = text[start..].find("</number>").ok_or("malformed rc.xml")? + start;
            let updated = format!("{}<number>{}{}", &text[..start], count, &text[end..]);
            std::fs::write(&path, updated).map_err(|e| e.to_string())?;
            reload_compositor()
        }
        // sway's workspaces are dynamically named/created on demand rather
        // than a fixed count, and wayfire's grid size is explicitly
        // documented as not changeable at runtime — neither maps onto "set
        // a workspace count" the way labwc's fixed `<number>` does.
        _ => Err("this backend does not support changing the workspace count at runtime".into()),
    }
}

fn set_keyboard_layout(payload: &serde_json::Value) -> Result<(), String> {
    let layout = payload.get("layout").and_then(|v| v.as_str()).ok_or("missing layout")?;
    let variant = payload.get("variant").and_then(|v| v.as_str());
    match active() {
        BackendKind::Labwc => {
            set_environment_var(&labwc_config::user_config_dir(load_config().config_dir.as_deref()), "XKB_DEFAULT_LAYOUT", layout)?;
            if let Some(v) = variant {
                set_environment_var(&labwc_config::user_config_dir(load_config().config_dir.as_deref()), "XKB_DEFAULT_VARIANT", v)?;
            }
            Ok(()) // labwc reads its `environment` file at start-up: applies at next login
        }
        // sway applies input settings live over IPC — no restart needed.
        BackendKind::Sway => {
            run_swaymsg(&["input", "*", "xkb_layout", layout])?;
            if let Some(v) = variant {
                run_swaymsg(&["input", "*", "xkb_variant", v])?;
            }
            Ok(())
        }
        BackendKind::Wayfire => {
            let cfg = load_config();
            let path = wayfire_config::user_config_path(cfg.config_dir.as_deref());
            wayfire_config::set_ini_value(&path, "input", "xkb_layout", layout)?;
            if let Some(v) = variant {
                wayfire_config::set_ini_value(&path, "input", "xkb_variant", v)?;
            }
            Err("saved — wayfire re-reads this on the next login (no live-reload for this setting)".into())
        }
        BackendKind::HackerosComp => Err("not applicable to hackeros-comp".into()),
    }
}

fn set_cursor(payload: &serde_json::Value) -> Result<(), String> {
    let theme = payload.get("theme").and_then(|v| v.as_str());
    let size = payload.get("size").and_then(|v| v.as_u64());
    match active() {
        BackendKind::Labwc => {
            let dir = labwc_config::user_config_dir(load_config().config_dir.as_deref());
            if let Some(theme) = theme {
                set_environment_var(&dir, "XCURSOR_THEME", theme)?;
            }
            if let Some(size) = size {
                set_environment_var(&dir, "XCURSOR_SIZE", &size.to_string())?;
            }
            Ok(())
        }
        BackendKind::Sway => {
            // sway's `seat <n> xcursor_theme <theme> [<size>]` wants both in
            // one call; fall back to the current size if only the theme
            // changed (and vice versa) so neither is silently reset.
            let theme = theme.map(str::to_string).or_else(current_xcursor_theme).unwrap_or_else(|| "default".into());
            let size = size.unwrap_or(24);
            run_swaymsg(&["seat", "*", "xcursor_theme", &theme, &size.to_string()])
        }
        BackendKind::Wayfire => {
            let cfg = load_config();
            let path = wayfire_config::user_config_path(cfg.config_dir.as_deref());
            if let Some(theme) = theme {
                wayfire_config::set_ini_value(&path, "input", "cursor_theme", theme)?;
            }
            if let Some(size) = size {
                wayfire_config::set_ini_value(&path, "input", "cursor_size", &size.to_string())?;
            }
            Err("saved — wayfire re-reads this on the next login (no live-reload for this setting)".into())
        }
        BackendKind::HackerosComp => Err("not applicable to hackeros-comp".into()),
    }
}

fn current_xcursor_theme() -> Option<String> {
    std::env::var("XCURSOR_THEME").ok()
}

fn set_environment_var(dir: &Path, key: &str, value: &str) -> Result<(), String> {
    if value.contains('\n') || value.contains('\r') {
        return Err("invalid value".into());
    }
    let path = dir.join("environment");
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

// ── Overlay peek (shared by labwc / sway / wayfire) ─────────────────────

/// State of an in-progress "peek" (native windows hidden for an overlay).
struct Peek {
    /// Windows *we* minimized, least recently used first.
    minimized: Vec<u64>,
    /// Highest window id at the time — anything newer was launched meanwhile.
    baseline_id: u64,
    /// Window explicitly chosen while peeking.
    target: Option<u64>,
}

static PEEK: std::sync::Mutex<Option<Peek>> = std::sync::Mutex::new(None);
/// Bumped whenever the window arrangement is changed on purpose (overlay
/// opened, "go to desktop") so a still-pending delayed restore backs off.
static EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn bump_epoch() -> u64 {
    EPOCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

fn overlay_open() {
    let mut guard = match PEEK.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if guard.is_some() {
        return; // already peeking
    }
    bump_epoch();
    let list = toplevels::list();
    let mut visible: Vec<&toplevels::Toplevel> =
        list.iter().filter(|t| !toplevels::is_shell_window(t) && !t.minimized).collect();
    visible.sort_by_key(|t| t.last_active);
    for t in &visible {
        toplevels::send(toplevels::Cmd::SetMinimized(t.id, true));
    }
    *guard = Some(Peek {
        minimized: visible.iter().map(|t| t.id).collect(),
        baseline_id: list.iter().map(|t| t.id).max().unwrap_or(0),
        target: None,
    });
    drop(guard);
    focus_shell_window();
}

fn overlay_close() {
    let peek = match PEEK.lock() {
        Ok(mut g) => g.take(),
        Err(_) => None,
    };
    let Some(peek) = peek else { return };
    let epoch = EPOCH.load(std::sync::atomic::Ordering::SeqCst);
    let _ = std::thread::Builder::new().name("blue-peek-restore".into()).spawn(move || {
        // Anything mapped while the overlay was up (an app launched from the
        // Start menu) should keep focus rather than the previous window.
        let launched = toplevels::list()
            .into_iter()
            .filter(|t| t.id > peek.baseline_id && !toplevels::is_shell_window(t))
            .map(|t| t.id)
            .max();
        let target = peek.target.or(launched).or_else(|| peek.minimized.last().copied());
        for id in &peek.minimized {
            toplevels::send(toplevels::Cmd::SetMinimized(*id, false));
        }
        // Un-minimizing may move focus around; settle it explicitly.
        if let Some(id) = target {
            std::thread::sleep(std::time::Duration::from_millis(120));
            if EPOCH.load(std::sync::atomic::Ordering::SeqCst) == epoch {
                toplevels::send(toplevels::Cmd::Activate(id));
            }
        }
    });
}

/// Gives the shell's own toplevel keyboard focus. Used when a modal text
/// field opens while a native app still holds keyboard focus.
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
    /// Whether `active` is a backend Blue drives itself over native
    /// wlroots protocols (labwc/sway/wayfire) rather than hackeros-comp's
    /// own IPC. The frontend gates all native-only behaviour (peeking
    /// native windows for overlays, raising the shell, Alt+Tab forwarding)
    /// on this single flag instead of comparing `active` against a
    /// hardcoded backend name, so it never needs updating when a new
    /// native backend is added.
    pub is_native: bool,
    pub config_path: String,
    pub backend_available: bool,
    /// Whether the configured native backend already has a usable config
    /// (the person's own, or a previous Blue run's default) — `false`
    /// means the next start will generate one from scratch. Always `true`
    /// for `hackeros-comp`, which has no config of its own to generate.
    pub config_prepared: bool,
    pub hackeros_comp_available: bool,
    pub toplevel_tracking: bool,
    pub clipboard_watcher: bool,
}

pub fn info() -> BackendInfo {
    let cfg = load_config();
    let backend_available = if cfg.compositor.is_native() { find_in_path(&cfg.binary).is_some() } else { true };
    let config_prepared = match cfg.compositor {
        BackendKind::Labwc => labwc_config::has_prepared_config(cfg.config_dir.as_deref()),
        BackendKind::Sway => sway_config::has_prepared_config(cfg.config_dir.as_deref()),
        BackendKind::Wayfire => wayfire_config::has_prepared_config(cfg.config_dir.as_deref()),
        BackendKind::HackerosComp => true,
    };
    BackendInfo {
        configured: cfg.compositor.as_str(),
        active: active().as_str(),
        is_native: active().is_native(),
        config_path: cfg.source.display().to_string(),
        backend_available,
        config_prepared,
        hackeros_comp_available: find_in_path("hackeros-comp").is_some() || hackeros_socket_exists(),
        toplevel_tracking: toplevels::is_running(),
        clipboard_watcher: clipboard::is_available(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_backend_names() {
        assert_eq!(BackendKind::parse("labwc"), Some(BackendKind::Labwc));
        assert_eq!(BackendKind::parse(" LabWC "), Some(BackendKind::Labwc));
        assert_eq!(BackendKind::parse("hackeros-comp"), Some(BackendKind::HackerosComp));
        assert_eq!(BackendKind::parse("HackerOS_Comp"), Some(BackendKind::HackerosComp));
        assert_eq!(BackendKind::parse("sway"), Some(BackendKind::Sway));
        assert_eq!(BackendKind::parse("SWAY"), Some(BackendKind::Sway));
        assert_eq!(BackendKind::parse("wayfire"), Some(BackendKind::Wayfire));
        assert_eq!(BackendKind::parse("Wayfire-WM"), Some(BackendKind::Wayfire));
        assert_eq!(BackendKind::parse("gnome"), None);
    }

    #[test]
    fn is_native_matches_the_three_wlroots_backends() {
        assert!(!BackendKind::HackerosComp.is_native());
        assert!(BackendKind::Labwc.is_native());
        assert!(BackendKind::Sway.is_native());
        assert!(BackendKind::Wayfire.is_native());
    }

    #[test]
    fn replaces_compositor_preserving_rest() {
        let text = "! hi\n[backend]\n-> compositor => hackeros-comp\n-> labwc_binary => labwc\n\n[other]\n-> compositor => x\n";
        let out = replace_compositor_line(text, BackendKind::Wayfire);
        assert!(out.contains("[backend]\n-> compositor => wayfire\n-> labwc_binary => labwc"));
        assert!(out.contains("[other]\n-> compositor => x"));
        assert!(out.starts_with("! hi"));
    }

    #[test]
    fn adds_missing_section_or_key() {
        let out = replace_compositor_line("[ui]\n-> a => b\n", BackendKind::Sway);
        assert!(out.ends_with("[backend]\n-> compositor => sway\n"));
        let out = replace_compositor_line("[backend]\n-> labwc_binary => x\n[ui]\n", BackendKind::Wayfire);
        assert!(out.contains("-> labwc_binary => x\n-> compositor => wayfire\n[ui]"));
    }

    #[test]
    fn startup_command_quotes_path() {
        let c = startup_command(Path::new("/opt/it's/blue"), &["--dev".into()]);
        assert_eq!(c, "'/opt/it'\\''s/blue' --labwc-child '--dev'");
    }

    #[test]
    fn autostart_command_uses_the_right_child_flag() {
        let c = autostart_command(Path::new("/usr/bin/blue-environment"), ARG_SWAY_CHILD, &[]);
        assert_eq!(c, "'/usr/bin/blue-environment' --sway-child");
        let c = autostart_command(Path::new("/usr/bin/blue-environment"), ARG_WAYFIRE_CHILD, &[]);
        assert_eq!(c, "'/usr/bin/blue-environment' --wayfire-child");
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
        for name in ["labwc", "sway", "wayfire"] {
            assert_eq!(backend.get(&format!("{name}_binary")).unwrap().as_string().unwrap(), name);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn child_flags_are_distinct_and_map_back() {
        assert_eq!(child_flag(BackendKind::Labwc), Some(ARG_LABWC_CHILD));
        assert_eq!(child_flag(BackendKind::Sway), Some(ARG_SWAY_CHILD));
        assert_eq!(child_flag(BackendKind::Wayfire), Some(ARG_WAYFIRE_CHILD));
        assert_eq!(child_flag(BackendKind::HackerosComp), None);
    }
}
