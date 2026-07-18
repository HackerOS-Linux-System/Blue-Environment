use crate::types::*;
use crate::{apps, cache, ai, packages, window_tracker};
use crate::session;
use crate::exploler_app;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as TokioCommand;
use serde::{Serialize, Deserialize};

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
    let session = session::detect_session();
    match session {
        session::SessionType::X11Client => {
            let _ = Command::new("xdotool")
            .args(["windowfocus", "--sync", &win_id])
            .spawn();
            true
        }
        session::SessionType::WaylandClient => {
            let _ = Command::new("swaymsg")
            .args(["[pid=", &win_id, "]", "focus"])
            .spawn();
            false
        }
        session::SessionType::Tty => true,
    }
}

pub fn resolve_path(path: &str) -> PathBuf {
    exploler_app::resolve_path(path)
}
