use std::process::{Child, Command};
use tracing::{info, warn};

/// Set up Wayland session environment variables
pub fn setup_environment() {
    std::env::set_var("XDG_SESSION_TYPE", "wayland");
    std::env::set_var("XDG_CURRENT_DESKTOP", "Blue:HackerOS");
    std::env::set_var("DESKTOP_SESSION", "blue");

    // XDG runtime dir
    let uid = nix::unistd::getuid().as_raw();
    let runtime = format!("/run/user/{}", uid);
    std::fs::create_dir_all(&runtime).ok();
    std::env::set_var("XDG_RUNTIME_DIR", &runtime);

    // Qt
    std::env::set_var("QT_QPA_PLATFORM", "wayland;xcb");
    std::env::set_var("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1");
    std::env::set_var("QT_AUTO_SCREEN_SCALE_FACTOR", "1");

    // GTK
    std::env::set_var("GDK_BACKEND", "wayland,x11");
    std::env::set_var("CLUTTER_BACKEND", "wayland");

    // Mozilla
    std::env::set_var("MOZ_ENABLE_WAYLAND", "1");
    std::env::set_var("MOZ_DBUS_REMOTE", "1");

    // SDL
    std::env::set_var("SDL_VIDEODRIVER", "wayland");

    // Electron / Chromium
    std::env::set_var("ELECTRON_OZONE_PLATFORM_HINT", "auto");
    std::env::set_var("NIXOS_OZONE_WL", "1");

    info!("Session environment configured");
}

/// Start D-Bus session daemon if not already running
pub fn ensure_dbus() {
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok() {
        info!("D-Bus session already active");
        return;
    }

    info!("Starting D-Bus session daemon...");
    if let Ok(output) = Command::new("dbus-launch")
        .arg("--sh-syntax")
        .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim_end_matches(';');
                if let Some((key, val)) = line.split_once('=') {
                    let val = val.trim_matches('\'');
                    if key == "DBUS_SESSION_BUS_ADDRESS" || key == "DBUS_SESSION_BUS_PID" {
                        std::env::set_var(key, val);
                        info!("D-Bus: {}={}", key, val);
                    }
                }
            }
        }
}

/// Wait for the Wayland socket to appear (compositor just started)
pub fn wait_for_wayland_socket() -> Option<String> {
    let uid = nix::unistd::getuid().as_raw();
    let runtime = format!("/run/user/{}", uid);

    for _ in 0..50 {
        for i in 0..=5 {
            let name = format!("wayland-{}", i);
            let path = format!("{}/{}", runtime, name);
            if std::path::Path::new(&path).exists() {
                info!("Wayland socket ready: {}", path);
                std::env::set_var("WAYLAND_DISPLAY", &name);
                return Some(name);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    warn!("Wayland socket not found after 10s");
    None
}

/// Launch session daemons (panel, audio, portals, polkit)
/// Returns list of (name, child) for cleanup
pub fn start_daemons() -> Vec<(String, Child)> {
    let mut children = Vec::new();

    // These are all bundled in the blue-environment binary itself;
    // they communicate via IPC. External daemons below:
    let specs: &[(&str, &[&str])] = &[
        ("pipewire",              &["pipewire"]),
        ("pipewire-pulse",        &["pipewire-pulse"]),
        ("wireplumber",           &["wireplumber"]),
        ("xdg-desktop-portal",   &["xdg-desktop-portal"]),
        ("xdg-portal-wlr",       &["xdg-desktop-portal-wlr"]),
        ("polkit",                &["/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1"]),
        ("nm-applet",             &["nm-applet", "--indicator"]),
    ];

    for (name, args) in specs {
        match Command::new(args[0]).args(&args[1..]).spawn() {
            Ok(child) => {
                info!("Started: {}", name);
                children.push((name.to_string(), child));
            }
            Err(e) => warn!("Could not start {}: {}", name, e),
        }
    }

    children
}

/// Run XDG autostart entries
pub fn run_autostart() {
    let dirs = [
        "/etc/xdg/autostart/".to_string(),
        format!(
            "{}/.config/autostart/",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];

    for dir in &dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "desktop") {
                if let Some(exec) = parse_autostart(&path) {
                    let parts: Vec<&str> = exec.split_whitespace().collect();
                    if let Some(bin) = parts.first() {
                        Command::new(bin)
                        .args(&parts[1..])
                        .spawn()
                        .ok();
                        info!("Autostart: {}", exec);
                    }
                }
            }
        }
    }
}

fn parse_autostart(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut exec = None;
    let mut hidden = false;
    let mut only: Vec<String> = Vec::new();
    let mut not: Vec<String> = Vec::new();

    for line in content.lines() {
        if let Some((k, v)) = line.trim().split_once('=') {
            match k {
                "Exec" => exec = Some(v.to_string()),
                "Hidden" => hidden = v == "true",
                "OnlyShowIn" => only = v.split(';').map(String::from).collect(),
                "NotShowIn" => not = v.split(';').map(String::from).collect(),
                _ => {}
            }
        }
    }

    if hidden { return None; }
    if not.iter().any(|s| s == "Blue" || s == "HackerOS") { return None; }
    if !only.is_empty() && !only.iter().any(|s| s == "Blue" || s == "HackerOS") {
        return None;
    }

    exec.map(|e| clean_exec(&e))
}

fn clean_exec(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' { chars.next(); } else { out.push(c); }
    }
    out.trim().to_string()
}
