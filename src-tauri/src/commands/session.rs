use crate::{apps, cache, window_tracker};
use crate::session;
use crate::exploler_app;
use std::path::PathBuf;
use std::process::Command;

use crate::cache::CachedApp;

#[tauri::command]
pub fn get_session_type() -> String {
    session::session_info()
}

#[tauri::command]
pub fn get_system_apps(force_refresh: bool) -> Vec<CachedApp> {
    apps::scan_desktop_apps(force_refresh)
}

#[tauri::command]
pub fn get_recent_apps() -> Vec<String> {
    cache::get_recent_apps()
}

#[tauri::command]
pub fn record_app_launch(app_id: String) {
    cache::record_app_launch(&app_id);
}

#[tauri::command]
pub fn invalidate_app_cache() {
    cache::invalidate_app_cache();
}

#[tauri::command]
pub fn launch_process(command: String, app_id: Option<String>) {
    if let Some(id) = app_id {
        cache::record_app_launch(&id);
    }
    let session = session::detect_session();
    std::thread::spawn(move || {
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        match session {
            session::SessionType::Tty => {
                cmd.env("WAYLAND_DISPLAY", "wayland-blue-1")
                .arg(format!("{} &", command));
            }
            _ => {
                cmd.arg(format!("{} &", command));
            }
        }
        let _ = cmd.spawn();
    });
}

#[tauri::command]
pub fn get_external_windows() -> Vec<window_tracker::ExternalWindow> {
    window_tracker::get_external_windows()
}

#[tauri::command]
pub fn focus_external_window(win_id: String) {
    window_tracker::focus_window(&win_id);
}

#[tauri::command]
pub fn minimize_external_window(win_id: String) {
    window_tracker::minimize_window(&win_id);
}

#[tauri::command]
pub fn close_external_window(win_id: String) {
    window_tracker::close_window(&win_id);
}

#[tauri::command]
pub fn embed_external_window(win_id: String, _parent_id: String) -> bool {
    // Native Wayland windows (ids sourced from the compositor's own IPC,
    // see `window_tracker::get_wayland_windows_via_compositor_ipc`) route
    // through the compositor directly.
    if let Some(id) = win_id.strip_prefix("blue:") {
        if let Ok(id) = id.parse::<u64>() {
            window_tracker::focus_window(&format!("blue:{id}"));
            return true;
        }
        return false;
    }

    let session = session::detect_session();
    match session {
        session::SessionType::X11Client => {
            let _ = Command::new("xdotool")
            .args(["windowfocus", "--sync", &win_id])
            .spawn();
            true
        }
        session::SessionType::WaylandClient => {
            // Previously called `swaymsg` here — sway's own IPC protocol,
            // which HackerOS-Comp does not implement and never will
            // (this whole project *is* the compositor, not a client of
            // someone else's). A `win_id` reaching this arm without the
            // `blue:` prefix means it came from an X11/XWayland source
            // (`wmctrl`/`xdotool`) while the *session* itself is
            // Wayland — i.e. an XWayland-mapped window, which `xdotool`
            // (used by the X11Client arm above) already handles
            // correctly via XWayland's X11 socket. There's no remaining
            // case here that legitimately needs external-tool control:
            // genuinely native Wayland windows always carry the `blue:`
            // prefix and are handled by the branch above.
            false
        }
        session::SessionType::Tty => true,
    }
}

pub fn resolve_path(path: &str) -> PathBuf {
    exploler_app::resolve_path(path)
}
