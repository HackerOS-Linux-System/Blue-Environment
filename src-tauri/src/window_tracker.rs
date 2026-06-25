use std::process::Command;

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
    if let Some(wins) = try_wmctrl() {
        return wins;
    }
    if let Some(wins) = try_xdotool() {
        return wins;
    }
    get_wayland_windows_from_proc()
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

fn get_wayland_windows_from_proc() -> Vec<ExternalWindow> {
    use std::fs;

    let wayland_display =
        std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-1".to_string());
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", get_uid()));
    let socket_path = format!("{}/{}", runtime_dir, wayland_display);

    let mut windows = Vec::new();
    let self_pid = std::process::id();

    let proc_dir = match fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return windows,
    };

    for entry in proc_dir.flatten() {
        let pid_str = entry.file_name().to_string_lossy().to_string();
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if pid == self_pid {
            continue;
        }

        let fd_dir = format!("/proc/{}/fd", pid);
        let has_wayland = fs::read_dir(&fd_dir)
            .map(|entries| {
                entries.flatten().any(|fd| {
                    fs::read_link(fd.path())
                        .map(|t| t.to_string_lossy().contains(&socket_path))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if !has_wayland {
            continue;
        }

        let comm = fs::read_to_string(format!("/proc/{}/comm", pid))
            .unwrap_or_default()
            .trim()
            .to_string();

        if comm.is_empty() || comm == "blue-environment" || comm == "blue-compositor" {
            continue;
        }

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

        windows.push(ExternalWindow {
            id: format!("wayland-{}", pid),
            pid,
            title: display_name,
            class: comm,
            icon_path: resolve_icon_for_pid(pid),
            is_minimized: false,
            desktop: 0,
        });
    }

    windows
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

    // Check legendaryos app dirs first — a bundled icon shipped with the
    // app itself always wins over a generic theme lookup.
    if let Some(home) = dirs::home_dir() {
        let app_dir = home
            .join(".legendaryos/Blue-Environment/apps")
            .join(&exe_name);
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
    // hicolor/pixmaps paths like this used to. This is the most common
    // path for tracked X11/external windows, since most of them don't
    // live under the legendaryos apps dir at all.
    crate::icon_resolver::resolve_icon(&exe_name)
}

fn get_uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn focus_window(win_id: &str) {
    let _ = Command::new("wmctrl").args(["-i", "-a", win_id]).spawn();
    let _ = Command::new("xdotool")
        .args(["windowfocus", "--sync", win_id])
        .spawn();
}

pub fn minimize_window(win_id: &str) {
    let _ = Command::new("xdotool")
        .args(["windowminimize", win_id])
        .spawn();
}

pub fn close_window(win_id: &str) {
    let _ = Command::new("wmctrl").args(["-i", "-c", win_id]).spawn();
    // Fallback: send WM_DELETE_WINDOW via xdotool
    let _ = Command::new("xdotool")
        .args(["key", "--window", win_id, "alt+F4"])
        .spawn();
}
