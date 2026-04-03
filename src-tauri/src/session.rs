// src-tauri/src/session.rs
// Detects whether we're running in TTY (need compositor) or existing DE session

use std::env;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionType {
    /// Running inside existing Wayland session (KDE, GNOME, etc.)
    WaylandClient,
    /// Running inside existing X11 session
    X11Client,
    /// Running in raw TTY — needs compositor
    Tty,
}

pub fn detect_session() -> SessionType {
    // Check for Wayland display socket
    if env::var("WAYLAND_DISPLAY").is_ok() {
        return SessionType::WaylandClient;
    }
    // Check for X11 display
    if env::var("DISPLAY").is_ok() {
        return SessionType::X11Client;
    }
    // No display server — raw TTY
    SessionType::Tty
}

#[allow(dead_code)]
pub fn is_tty() -> bool {
    detect_session() == SessionType::Tty
}

pub fn session_info() -> String {
    match detect_session() {
        SessionType::WaylandClient => format!(
            "wayland:{}",
            env::var("WAYLAND_DISPLAY").unwrap_or("unknown".into())
        ),
        SessionType::X11Client => format!(
            "x11:{}",
            env::var("DISPLAY").unwrap_or("unknown".into())
        ),
        SessionType::Tty => "tty".to_string(),
    }
}
