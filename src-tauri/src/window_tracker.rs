use std::process::Command;

/// Prefix used to mark an `ExternalWindow::id` as sourced from the
/// compositor's own IPC (a real `window_meta` key, i.e. a `u64`) rather
/// than an X11 window id from `wmctrl`/`xdotool`. `focus_window`/
/// `minimize_window`/`close_window` below dispatch on this prefix to
/// decide whether to talk to the compositor over its Unix socket or shell
/// out to an X11 tool.
const COMPOSITOR_ID_PREFIX: &str = "blue:";

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ExternalWindow {
    pub id: String,
    pub pid: u32,
    pub title: String,
    pub class: String,
    pub icon_path: String,
    pub is_minimized: bool,
    pub desktop: i32,
}

pub fn get_external_windows() -> Vec<ExternalWindow> {
    // HackerOS-Comp's own `window_meta` tracks *every* mapped window —
    // native Wayland toplevels AND XWayland-backed X11 windows alike (see
    // `xwayland/mod.rs`'s `mapped_window`/`new_window`, which insert into
    // the same `window_meta` map as the native xdg-shell path). That
    // means when actually running under HackerOS-Comp, the compositor's
    // IPC is a *complete* window list on its own — wmctrl/xdotool add
    // nothing in that case, they just duplicate what IPC already reports
    // (with worse identity: an X11 window id that our own `focus_window`/
    // `close_window` then has to shell out to X11 tools to act on, versus
    // a native id we can act on directly over IPC).
    //
    // So: ask the compositor first. Only fall back to wmctrl/xdotool if
    // the compositor doesn't answer at all — meaning we're not actually
    // running under HackerOS-Comp (e.g. this shell running nested for
    // development inside a different desktop environment/X11 session),
    // which is the only scenario where those tools still pull their
    // weight.
    if let Some(native) = get_wayland_windows_via_compositor_ipc() {
        return native;
    }
    try_wmctrl().or_else(try_xdotool).unwrap_or_default()
}

fn try_wmctrl() -> Option<Vec<ExternalWindow>> {
    let out = Command::new("wmctrl")
        .args(["-l", "-p", "-G", "-x"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let mut windows = Vec::new();
    let text = String::from_utf8_lossy(&out.stdout);

    for line in text.lines() {
        // wmctrl -l -p -G -x columns:
        // WIN_ID  DESKTOP  PID  X  Y  W  H  WM_CLASS  CLIENT_MACHINE  TITLE
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }

        let win_id = cols[0].to_string();
        let desktop: i32 = cols[1].parse().unwrap_or(0);
        let pid: u32 = cols[2].parse().unwrap_or(0);
        let wm_class = cols[7].to_string();

        // Title is everything from column 9 onward
        let title = cols[9..].join(" ");

        if title.contains("Blue Environment")
            || title.is_empty()
            || wm_class.contains("blue-environment")
        {
            continue;
        }

        let icon_path = resolve_icon_for_pid(pid);

        windows.push(ExternalWindow {
            id: win_id,
            pid,
            title,
            class: wm_class,
            icon_path,
            is_minimized: desktop == -1,
            desktop,
        });
    }

    Some(windows)
}

fn try_xdotool() -> Option<Vec<ExternalWindow>> {
    let out = Command::new("xdotool")
        .args(["search", "--onlyvisible", "--name", ""])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let mut windows = Vec::new();
    let text = String::from_utf8_lossy(&out.stdout);

    for win_id_str in text.lines() {
        let win_id = win_id_str.trim();
        if win_id.is_empty() {
            continue;
        }

        let name = Command::new("xdotool")
            .args(["getwindowname", win_id])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let pid: u32 = Command::new("xdotool")
            .args(["getwindowpid", win_id])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse()
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        if name.is_empty() || name.contains("Blue Environment") {
            continue;
        }

        windows.push(ExternalWindow {
            id: win_id.to_string(),
            pid,
            title: name,
            class: String::new(),
            icon_path: resolve_icon_for_pid(pid),
            is_minimized: false,
            desktop: 0,
        });
    }

    Some(windows)
}

/// Previously named `get_wayland_windows_from_proc` — scanned `/proc/*/fd`
/// for processes holding the Wayland socket open and guessed at window
/// identity from `/proc/<pid>/comm`/`cmdline`. That approach had two real
/// problems: (1) it can't distinguish "has the Wayland socket open" from
/// "has a mapped, visible toplevel window" (many background services hold
/// the socket open), and (2) the synthetic `wayland-{pid}` ids it produced
/// couldn't actually be used to focus/minimize/close anything — there was
/// no tool that understood them (which is exactly why
/// `embed_external_window` in `commands/session.rs` resorted to shelling
/// out to `swaymsg`, a *different compositor's* IPC protocol, for these).
///
/// This now asks HackerOS-Comp directly over its own IPC socket for the
/// real, authoritative window list — the same `WindowInfo` data the
/// panel/window-switcher already receives, just via a one-shot
/// request/response instead of `compositor_ipc_relay`'s long-lived
/// streaming connection. Real compositor-native ids come back
/// (`window_meta` keys), which `focus_window`/`minimize_window`/
/// `close_window` below can actually act on — no external tool involved
/// at all for native Wayland windows.
fn get_wayland_windows_via_compositor_ipc() -> Option<Vec<ExternalWindow>> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let socket_path = compositor_socket_path();
    // `None` here specifically means "couldn't even connect" — i.e. Blue
    // Compositor isn't running this session at all — as distinct from
    // "connected fine, got an empty window list", which is a legitimate
    // `Some(vec![])`. `get_external_windows()` uses that distinction to
    // decide whether falling back to wmctrl/xdotool makes sense.
    let Ok(stream) = UnixStream::connect(&socket_path) else { return None };
    // The compositor broadcasts a fresh window list to every connected
    // client every 33ms (see `compositor/src/ipc/socket.rs`'s
    // `broadcast_windows`, on the same timer as its client-poll loop) —
    // no need to even send a `get_window_list` request, just wait for the
    // next tick. A short read timeout keeps this from blocking the Tauri
    // command indefinitely if the compositor is unresponsive.
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let mut writer = stream.try_clone().ok();
    if let Some(w) = writer.as_mut() {
        let _ = w.write_all(b"{\"type\":\"get_window_list\"}\n");
    }

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let deadline = std::time::Instant::now() + Duration::from_millis(250);

    while std::time::Instant::now() < deadline {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // compositor closed the connection
            Ok(_) => {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) else { continue };
                if v.get("type").and_then(|t| t.as_str()) != Some("window_list") {
                    continue; // "ready" handshake message, or something else — keep waiting
                }
                let Some(windows) = v.get("windows").and_then(|w| w.as_array()) else { return Some(Vec::new()) };
                return Some(windows
                    .iter()
                    .filter_map(|w| {
                        let id = w.get("id")?.as_u64()?;
                        Some(ExternalWindow {
                            id: format!("{COMPOSITOR_ID_PREFIX}{id}"),
                            pid: 0, // the compositor's WindowInfo doesn't carry a pid (Wayland clients aren't required to be local processes, e.g. over a proxied socket) — icon resolution falls back to app_id below
                            title: w.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_string(),
                            class: w.get("app_id").and_then(|a| a.as_str()).unwrap_or_default().to_string(),
                            icon_path: w.get("app_id")
                                .and_then(|a| a.as_str())
                                .map(resolve_icon_by_name)
                                .unwrap_or_default(),
                            is_minimized: w.get("is_minimized").and_then(|m| m.as_bool()).unwrap_or(false),
                            desktop: w.get("workspace").and_then(|ws| ws.as_u64()).unwrap_or(0) as i32,
                        })
                    })
                    .collect());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) => break,
        }
    }
    // Connected successfully but never got a `window_list` message within
    // the deadline — treat this as "compositor is running but slow/stuck"
    // rather than "not running", so we report an empty list instead of
    // falling back to wmctrl (which would report *stale* X11-only data,
    // arguably worse than an empty list that will self-correct next poll).
    Some(Vec::new())
}

fn compositor_socket_path() -> std::path::PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", get_uid()));
    std::path::PathBuf::from(runtime_dir).join("hackeros-comp.sock")
}

/// Fire-and-forget command to the compositor, mirroring the pattern
/// already established by `SettingsApp::settings_send_to_compositor` —
/// open a fresh connection, write one JSON line, done. Used by
/// `focus_window`/`minimize_window`/`close_window` below for
/// compositor-native (`blue:`-prefixed) window ids so none of them need
/// to shell out to `swaymsg`/`xdotool`/`wmctrl` for native Wayland
/// windows.
fn send_compositor_command(cmd_type: &str, id: u64) {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let Ok(mut stream) = UnixStream::connect(compositor_socket_path()) else { return };
    let msg = format!("{{\"type\":\"{}\",\"id\":{}}}\n", cmd_type, id);
    let _ = stream.write_all(msg.as_bytes());
}

fn resolve_icon_for_pid(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }

    let exe = std::fs::read_link(format!("/proc/{}/exe", pid))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let exe_name = exe.split('/').last().unwrap_or("").to_string();
    if exe_name.is_empty() {
        return String::new();
    }

    resolve_icon_by_name(&exe_name)
}

/// Shared icon lookup: checks a bundled LegendaryOS/Blue-Environment app
/// icon first (an app ships its own `icon.png` under
/// `~/.legendaryos/Blue-Environment/apps/<name>/`, which should always
/// win over a generic theme lookup), then falls back to the shared
/// FreeDesktop icon theme resolver.
///
/// Previously this bundled-icon check only happened in
/// `resolve_icon_for_pid` (the X11/`wmctrl` code path, which has a PID to
/// derive an executable name from) — `get_wayland_windows_via_compositor_ipc`
/// (the native-Wayland/XWayland path, sourced from the compositor's
/// `window_meta` over IPC) called `icon_resolver::resolve_icon` directly,
/// skipping the bundled-icon check entirely. That meant a native Blue app
/// with its own shipped icon would show a generic theme icon (or none)
/// when tracked via the compositor, but its correct bundled icon when
/// (coincidentally) also visible via X11/XWayland. Both paths now go
/// through this one function.
fn resolve_icon_by_name(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }

    if let Some(home) = dirs::home_dir() {
        let app_dir = home
            .join(".legendaryos/Blue-Environment/apps")
            .join(name);
        for ext in &["icon.png", "icon.svg", "icon.jpg"] {
            let icon = app_dir.join(ext);
            if icon.exists() {
                return format!("file://{}", icon.to_string_lossy());
            }
        }
    }

    // Fall back to the shared FreeDesktop icon theme resolver (linicon),
    // which searches the user's actual icon theme, Papirus, and every
    // theme's full Inherits= fallback chain — not just a couple of fixed
    // hicolor/pixmaps paths like this used to.
    crate::icon_resolver::resolve_icon(name)
}

fn get_uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn focus_window(win_id: &str) {
    if let Some(id) = win_id.strip_prefix(COMPOSITOR_ID_PREFIX) {
        if let Ok(id) = id.parse::<u64>() {
            send_compositor_command("focus_window", id);
        }
        return;
    }
    let _ = Command::new("wmctrl").args(["-i", "-a", win_id]).spawn();
    let _ = Command::new("xdotool")
        .args(["windowfocus", "--sync", win_id])
        .spawn();
}

pub fn minimize_window(win_id: &str) {
    if let Some(id) = win_id.strip_prefix(COMPOSITOR_ID_PREFIX) {
        if let Ok(id) = id.parse::<u64>() {
            send_compositor_command("minimize_window", id);
        }
        return;
    }
    let _ = Command::new("xdotool")
        .args(["windowminimize", win_id])
        .spawn();
}

pub fn close_window(win_id: &str) {
    if let Some(id) = win_id.strip_prefix(COMPOSITOR_ID_PREFIX) {
        if let Ok(id) = id.parse::<u64>() {
            send_compositor_command("close_window", id);
        }
        return;
    }
    let _ = Command::new("wmctrl").args(["-i", "-c", win_id]).spawn();
    // Fallback: send WM_DELETE_WINDOW via xdotool
    let _ = Command::new("xdotool")
        .args(["key", "--window", win_id, "alt+F4"])
        .spawn();
}
