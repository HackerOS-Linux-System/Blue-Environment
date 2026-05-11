use std::env;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionType {
    WaylandClient,
    X11Client,
    Tty,
}

pub fn detect_session() -> SessionType {
    if env::var("WAYLAND_DISPLAY").is_ok() {
        return SessionType::WaylandClient;
    }
    if env::var("DISPLAY").is_ok() {
        return SessionType::X11Client;
    }
    SessionType::Tty
}

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
