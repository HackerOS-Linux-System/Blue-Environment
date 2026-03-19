// src-tauri/src/window_tracker.rs
// Tracks external application windows so Blue Environment can show them
// in taskbar and Alt+Tab — works in both X11 and Wayland sessions

use std::process::Command;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ExternalWindow {
    pub id: String,         // window ID (hex for X11, PID-based for Wayland)
    pub pid: u32,
    pub title: String,
    pub class: String,      // WM_CLASS — identifies which app
    pub icon_path: String,  // resolved from /proc/<pid>/exe → app dir
    pub is_minimized: bool,
    pub desktop: i32,       // virtual desktop number
}

/// Get list of external windows via wmctrl (X11) or xdotool
pub fn get_external_windows() -> Vec<ExternalWindow> {
    // Try wmctrl first (works on X11 and XWayland)
    if let Some(wins) = try_wmctrl() {
        return wins;
    }
    // Fallback: xdotool
    if let Some(wins) = try_xdotool() {
        return wins;
    }
    // Wayland native: parse /proc for GUI processes
    get_wayland_windows_from_proc()
}

fn try_wmctrl() -> Option<Vec<ExternalWindow>> {
    let out = Command::new("wmctrl")
    .args(["-l", "-p", "-G", "-x"])
    .output()
    .ok()?;

    if !out.status.success() { return None; }

    let mut windows = Vec::new();
    let text = String::from_utf8_lossy(&out.stdout);

    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(9, ' ')
        .filter(|s| !s.is_empty())
        .collect();

        if parts.len() < 8 { continue; }

        let win_id = parts[0].to_string();
        let desktop: i32 = parts[1].parse().unwrap_or(0);
        let pid: u32 = parts[2].parse().unwrap_or(0);
        let wm_class = parts[7].to_string();
        let title = parts.get(8).copied().unwrap_or("").to_string();

        // Skip our own window and desktop/panel windows
        if title.contains("Blue Environment") || title.is_empty() { continue; }
        if wm_class.contains("blue-environment") { continue; }

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

    if !out.status.success() { return None; }

    let mut windows = Vec::new();
    let text = String::from_utf8_lossy(&out.stdout);

    for win_id_str in text.lines() {
        let win_id = win_id_str.trim();
        if win_id.is_empty() { continue; }

        // Get window name and PID
        let name = Command::new("xdotool")
        .args(["getwindowname", win_id])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

        let pid: u32 = Command::new("xdotool")
        .args(["getwindowpid", win_id])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0))
        .unwrap_or(0);

        if name.is_empty() || name.contains("Blue Environment") { continue; }

        let icon_path = resolve_icon_for_pid(pid);

        windows.push(ExternalWindow {
            id: win_id.to_string(),
                     pid,
                     title: name,
                     class: String::new(),
                     icon_path,
                     is_minimized: false,
                     desktop: 0,
        });
    }

    Some(windows)
}

/// For Wayland native sessions — find GUI processes by checking
/// if they have open sockets to the Wayland compositor
fn get_wayland_windows_from_proc() -> Vec<ExternalWindow> {
    use std::fs;
    let mut windows = Vec::new();

    let wayland_display = std::env::var("WAYLAND_DISPLAY")
    .unwrap_or("wayland-1".to_string());
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
    .unwrap_or(format!("/run/user/{}", libc_getuid()));
    let socket_path = format!("{}/{}", runtime_dir, wayland_display);

    if let Ok(proc_dir) = fs::read_dir("/proc") {
        for entry in proc_dir.flatten() {
            let pid_str = entry.file_name().to_string_lossy().to_string();
            let pid: u32 = match pid_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Skip kernel threads and our own process
            if pid == std::process::id() { continue; }

            // Check if this process has the Wayland socket open
            let fd_dir = format!("/proc/{}/fd", pid);
            let has_wayland = fs::read_dir(&fd_dir)
            .map(|entries| {
                entries.flatten().any(|fd| {
                    fs::read_link(fd.path())
                    .map(|target| target.to_string_lossy().contains(&socket_path))
                    .unwrap_or(false)
                })
            })
            .unwrap_or(false);

            if !has_wayland { continue; }

            // Get process name
            let comm = fs::read_to_string(format!("/proc/{}/comm", pid))
            .unwrap_or_default()
            .trim()
            .to_string();

            if comm.is_empty() || comm == "blue-environment" { continue; }

            // Get cmdline for better name
            let cmdline = fs::read_to_string(format!("/proc/{}/cmdline", pid))
            .unwrap_or_default()
            .replace('\0', " ")
            .trim()
            .to_string();

            let display_name = cmdline
            .split_whitespace()
            .next()
            .unwrap_or(&comm)
            .split('/')
            .last()
            .unwrap_or(&comm)
            .to_string();

            let icon_path = resolve_icon_for_pid(pid);

            windows.push(ExternalWindow {
                id: format!("wayland-{}", pid),
                         pid,
                         title: display_name.clone(),
                         class: comm,
                         icon_path,
                         is_minimized: false,
                         desktop: 0,
            });
        }
    }

    windows
}

/// Try to find an icon for a process by:
/// 1. Checking ~/.hackeros/Blue-Environment/apps/<name>/icon.*
/// 2. Checking /usr/share/icons for the app name
/// 3. Returning empty string (frontend shows letter avatar)
fn resolve_icon_for_pid(pid: u32) -> String {
    if pid == 0 { return String::new(); }

    // Get executable path
    let exe = std::fs::read_link(format!("/proc/{}/exe", pid))
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_default();

    let exe_name = exe.split('/').last().unwrap_or("").to_string();
    if exe_name.is_empty() { return String::new(); }

    // Check ~/.hackeros/Blue-Environment/apps/<name>/icon.*
    if let Some(home) = dirs::home_dir() {
        let app_dir = home
        .join(".hackeros/Blue-Environment/apps")
        .join(&exe_name);

        for ext in &["icon.png", "icon.svg", "icon.jpg"] {
            let icon = app_dir.join(ext);
            if icon.exists() {
                return format!("file://{}", icon.to_string_lossy());
            }
        }
    }

    // Check system icons
    let icon_dirs = [
        format!("/usr/share/icons/hicolor/48x48/apps/{}.png", exe_name),
            format!("/usr/share/icons/hicolor/scalable/apps/{}.svg", exe_name),
                format!("/usr/share/pixmaps/{}.png", exe_name),
    ];

    for path in &icon_dirs {
        if std::path::Path::new(path).exists() {
            return format!("file://{}", path);
        }
    }

    String::new()
}

/// Focus an external window by ID
pub fn focus_window(win_id: &str) {
    // Try wmctrl first
    let _ = Command::new("wmctrl")
    .args(["-i", "-a", win_id])
    .spawn();

    // Also try xdotool
    let _ = Command::new("xdotool")
    .args(["windowfocus", "--sync", win_id])
    .spawn();
}

/// Minimize an external window
pub fn minimize_window(win_id: &str) {
    let _ = Command::new("xdotool")
    .args(["windowminimize", win_id])
    .spawn();
}

/// Close an external window gracefully
pub fn close_window(win_id: &str) {
    let _ = Command::new("wmctrl")
    .args(["-i", "-c", win_id])
    .spawn();
}

fn libc_getuid() -> u32 {
    unsafe {
        extern "C" { fn getuid() -> u32; }
        getuid()
    }
}
