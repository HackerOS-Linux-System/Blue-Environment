mod loader;

use slint::ComponentHandle;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};

use crate::ipc::{self, IpcMessage};

slint::include_modules!();

#[derive(Clone, Copy, PartialEq)]
enum Mode { Hidden, Compact, Fullscreen }

pub fn launch(_wayland_display: String) {
    std::thread::spawn(|| run());
}

pub fn run() {
    info!("App menu: starting...");
    wait_wayland();

    let apps = Arc::new(Mutex::new(loader::load_all()));
    info!("App menu: {} apps loaded", apps.lock().unwrap().len());

    let compact = match CompactAppMenu::new() {
        Ok(w) => w,
        Err(e) => { warn!("CompactAppMenu failed: {}", e); return; }
    };
    let fullscreen = match FullscreenLauncher::new() {
        Ok(w) => w,
        Err(e) => { warn!("FullscreenLauncher failed: {}", e); return; }
    };

    let mode = Arc::new(Mutex::new(Mode::Hidden));

    // ── Compact ────────────────────────────────────────

    let mc = mode.clone();
    let cw = compact.as_weak();
    compact.on_close_requested(move || {
        *mc.lock().unwrap() = Mode::Hidden;
        cw.upgrade().map(|w| w.hide().ok());
    });

    let cw2 = compact.as_weak();
    compact.on_app_launched(move |exec| {
        exec_app(&exec.to_string());
        cw2.upgrade().map(|w| w.hide().ok());
    });

    compact.on_search_changed(|_| {});

    // ── Fullscreen ─────────────────────────────────────

    let mf = mode.clone();
    let fw = fullscreen.as_weak();
    fullscreen.on_close_requested(move || {
        *mf.lock().unwrap() = Mode::Hidden;
        fw.upgrade().map(|w| w.hide().ok());
    });

    let fw2 = fullscreen.as_weak();
    fullscreen.on_app_launched(move |exec| {
        exec_app(&exec.to_string());
        fw2.upgrade().map(|w| w.hide().ok());
    });

    fullscreen.on_search_changed(|_| {});
    fullscreen.on_category_changed(|_| {});

    // ── IPC listener ───────────────────────────────────

    let cw3 = compact.as_weak();
    let fw3 = fullscreen.as_weak();
    let m3  = mode.clone();

    std::thread::spawn(move || {
        ipc::listen(move |msg| {
            let cw = cw3.clone();
            let fw = fw3.clone();
            let m  = m3.clone();
            slint::invoke_from_event_loop(move || {
                let mut cur = m.lock().unwrap();
                match msg {
                    IpcMessage::ToggleAppMenu => match *cur {
                        Mode::Compact => {
                            *cur = Mode::Hidden;
                            cw.upgrade().map(|w| w.hide().ok());
                        }
                        _ => {
                            *cur = Mode::Compact;
                            fw.upgrade().map(|w| w.hide().ok());
                            cw.upgrade().map(|w| w.show().ok());
                        }
                    },
                    IpcMessage::ToggleFullscreenLauncher => match *cur {
                        Mode::Fullscreen => {
                            *cur = Mode::Hidden;
                            fw.upgrade().map(|w| w.hide().ok());
                        }
                        _ => {
                            *cur = Mode::Fullscreen;
                            cw.upgrade().map(|w| w.hide().ok());
                            fw.upgrade().map(|w| w.show().ok());
                        }
                    },
                    _ => {}
                }
            }).ok();
        });
    });

    compact.hide().ok();
    fullscreen.hide().ok();

    info!("App menu ready");
    slint::run_event_loop().ok();
}

fn exec_app(exec: &str) {
    let mut clean = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' { chars.next(); } else { clean.push(c); }
    }
    let clean = clean.trim().to_string();
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if let Some(bin) = parts.first() {
        std::process::Command::new(bin)
        .args(&parts[1..])
        .env("XDG_CURRENT_DESKTOP", "Blue:HackerOS")
        .spawn()
        .ok();
    }
}

fn wait_wayland() {
    let uid = nix::unistd::getuid().as_raw();
    for _ in 0..60 {
        for i in 0..=5 {
            if std::path::Path::new(&format!("/run/user/{}/wayland-{}", uid, i)).exists() {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
