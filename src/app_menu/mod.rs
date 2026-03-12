mod loader;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};

use crate::ipc::{self, IpcMessage};

slint::include_modules!();

#[derive(Clone, Copy, PartialEq)]
enum Mode { Hidden, Compact, Fullscreen }

// ── Wewnętrzna reprezentacja aplikacji ───────────────────
#[derive(Clone)]
struct AppData {
    name:       String,
    comment:    String,
    exec:       String,
    icon:       slint::Image,
    categories: Vec<String>,
}

pub fn launch(_wayland_display: String) {
    std::thread::spawn(|| run());
}

pub fn run() {
    info!("App menu: starting...");
    wait_ipc();

    // ── Wczytaj i przygotuj aplikacje ─────────────────────
    let raw = loader::load_all();
    info!("App menu: {} apps loaded", raw.len());

    let all_apps: Arc<Vec<AppData>> = Arc::new(
        raw.iter().map(|a| AppData {
            name:       a.name.clone(),
                       comment:    a.comment.clone().unwrap_or_default(),
                       exec:       a.exec.clone(),
                       icon:       resolve_icon(a.icon.as_deref().unwrap_or("")),
                       categories: a.categories.clone(),
        }).collect()
    );

    // ── Dane użytkownika z systemu ────────────────────────
    let username = std::env::var("USER")
    .or_else(|_| std::env::var("LOGNAME"))
    .unwrap_or_else(|_| {
        std::process::Command::new("whoami").output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "user".to_string())
    });
    let hostname = std::fs::read_to_string("/etc/hostname")
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|_| {
        std::process::Command::new("hostname").output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "hackeros".to_string())
    });
    info!("App menu: user={}@{}", username, hostname);

    // ── Twórz okna ───────────────────────────────────────
    let compact = match CompactAppMenu::new() {
        Ok(w) => w,
        Err(e) => { warn!("CompactAppMenu failed: {}", e); return; }
    };
    let fullscreen = match FullscreenLauncher::new() {
        Ok(w) => w,
        Err(e) => { warn!("FullscreenLauncher failed: {}", e); return; }
    };

    // Ustaw dane użytkownika
    compact.set_username(SharedString::from(username.as_str()));
    compact.set_hostname(SharedString::from(hostname.as_str()));

    // Załaduj początkową listę (brak filtra)
    compact.set_apps(ModelRc::new(VecModel::from(compact_items(&all_apps, ""))));
    fullscreen.set_apps(ModelRc::new(VecModel::from(fs_items(&all_apps, "", "All"))));

    let mode = Arc::new(Mutex::new(Mode::Hidden));

    // ── ZAPOBIEGAJ destroy przy zamknięciu okna ───────────
    // Bez tego Alt+F4 lub kliknięcie X niszczy event_loop
    // i ponowne show() nie działa
    {
        let cw = compact.as_weak();
        let mc = mode.clone();
        compact.window().on_close_requested(move || {
            *mc.lock().unwrap() = Mode::Hidden;
            if let Some(w) = cw.upgrade() {
                w.set_search_text(SharedString::from(""));
                w.hide().ok();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }
    {
        let fw = fullscreen.as_weak();
        let mf = mode.clone();
        fullscreen.window().on_close_requested(move || {
            *mf.lock().unwrap() = Mode::Hidden;
            if let Some(w) = fw.upgrade() {
                w.set_search_text(SharedString::from(""));
                w.hide().ok();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    // ── Compact: callbacks ────────────────────────────────
    {
        let mc2 = mode.clone();
        let cw2 = compact.as_weak();
        compact.on_close_requested(move || {
            *mc2.lock().unwrap() = Mode::Hidden;
            if let Some(w) = cw2.upgrade() {
                w.set_search_text(SharedString::from(""));
                w.hide().ok();
            }
        });
    }
    {
        let mc3 = mode.clone();
        let cw3 = compact.as_weak();
        compact.on_app_launched(move |exec| {
            exec_app(&exec.to_string());
            *mc3.lock().unwrap() = Mode::Hidden;
            if let Some(w) = cw3.upgrade() {
                w.set_search_text(SharedString::from(""));
                w.hide().ok();
            }
        });
    }
    {
        // WYSZUKIWANIE — kluczowa naprawka
        let apps4 = all_apps.clone();
        let cw4   = compact.as_weak();
        compact.on_search_changed(move |query| {
            let Some(w) = cw4.upgrade() else { return };
            let q = query.to_string();
            let items = compact_items(&apps4, &q);
            w.set_apps(ModelRc::new(VecModel::from(items)));
        });
    }

    // ── Fullscreen: callbacks ─────────────────────────────
    {
        let mf2 = mode.clone();
        let fw2 = fullscreen.as_weak();
        fullscreen.on_close_requested(move || {
            *mf2.lock().unwrap() = Mode::Hidden;
            if let Some(w) = fw2.upgrade() {
                w.set_search_text(SharedString::from(""));
                w.hide().ok();
            }
        });
    }
    {
        let mf3 = mode.clone();
        let fw3 = fullscreen.as_weak();
        fullscreen.on_app_launched(move |exec| {
            exec_app(&exec.to_string());
            *mf3.lock().unwrap() = Mode::Hidden;
            if let Some(w) = fw3.upgrade() {
                w.set_search_text(SharedString::from(""));
                w.hide().ok();
            }
        });
    }
    {
        // Wyszukiwanie w fullscreen
        let apps_fs = all_apps.clone();
        let fw4     = fullscreen.as_weak();
        fullscreen.on_search_changed(move |query| {
            let Some(w) = fw4.upgrade() else { return };
            let q   = query.to_string();
            let cat = w.get_active_category().to_string();
            w.set_apps(ModelRc::new(VecModel::from(fs_items(&apps_fs, &q, &cat))));
        });
    }
    {
        // Kategoria w fullscreen
        let apps_cat = all_apps.clone();
        let fw5      = fullscreen.as_weak();
        fullscreen.on_category_changed(move |cat| {
            let Some(w) = fw5.upgrade() else { return };
            let q   = w.get_search_text().to_string();
            let cat = cat.to_string();
            w.set_apps(ModelRc::new(VecModel::from(fs_items(&apps_cat, &q, &cat))));
        });
    }

    // ── IPC: toggle show/hide przez channel + Timer ───────
    let (tx, rx) = std::sync::mpsc::channel::<IpcMessage>();
    std::thread::spawn(move || {
        ipc::listen(move |msg| {
            match &msg {
                IpcMessage::ToggleAppMenu | IpcMessage::ToggleFullscreenLauncher => {
                    tx.send(msg).ok();
                }
                _ => {}
            }
        });
    });

    let cw_ipc = compact.as_weak();
    let fw_ipc = fullscreen.as_weak();
    let m_ipc  = mode.clone();
    let ipc_timer = slint::Timer::default();
    ipc_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
                    move || {
                        while let Ok(msg) = rx.try_recv() {
                            let mut cur = m_ipc.lock().unwrap();
                            match msg {
                                IpcMessage::ToggleAppMenu => match *cur {
                                    Mode::Compact => {
                                        // Drugie kliknięcie = zamknij
                                        *cur = Mode::Hidden;
                                        if let Some(w) = cw_ipc.upgrade() {
                                            w.set_search_text(SharedString::from(""));
                                            w.hide().ok();
                                        }
                                    }
                                    _ => {
                                        *cur = Mode::Compact;
                                        fw_ipc.upgrade().map(|w| w.hide().ok());
                                        if let Some(w) = cw_ipc.upgrade() {
                                            // Reset wyszukiwania przy każdym otwarciu
                                            w.set_search_text(SharedString::from(""));
                                            position_compact_window(&w);
                                            w.show().ok();
                                        }
                                    }
                                },
                                IpcMessage::ToggleFullscreenLauncher => match *cur {
                                    Mode::Fullscreen => {
                                        *cur = Mode::Hidden;
                                        if let Some(w) = fw_ipc.upgrade() {
                                            w.set_search_text(SharedString::from(""));
                                            w.hide().ok();
                                        }
                                    }
                                    _ => {
                                        *cur = Mode::Fullscreen;
                                        cw_ipc.upgrade().map(|w| w.hide().ok());
                                        fw_ipc.upgrade().map(|w| w.show().ok());
                                    }
                                },
                                _ => {}
                            }
                        }
                    },
    );
    std::mem::forget(ipc_timer);

    compact.hide().ok();
    fullscreen.hide().ok();

    info!("App menu ready");
    slint::run_event_loop().ok();
}

// ── Filtrowanie: compact list ─────────────────────────────

fn compact_items(apps: &[AppData], query: &str) -> Vec<AppItem> {
    let q = query.to_lowercase();
    let iter: Box<dyn Iterator<Item = &AppData>> = if q.is_empty() {
        Box::new(apps.iter().take(18))
    } else {
        Box::new(apps.iter().filter(move |a|
        a.name.to_lowercase().contains(&q)
        || a.comment.to_lowercase().contains(&q)
        || a.exec.split('/').last().unwrap_or(&a.exec)
        .to_lowercase().contains(&q)
        ).take(18))
    };
    iter.map(|a| AppItem {
        name:    SharedString::from(a.name.as_str()),
             comment: SharedString::from(a.comment.as_str()),
             icon:    a.icon.clone(),
             exec:    SharedString::from(a.exec.as_str()),
    }).collect()
}

// ── Filtrowanie: fullscreen grid ──────────────────────────

fn fs_items(apps: &[AppData], query: &str, category: &str) -> Vec<FsAppItem> {
    let q   = query.to_lowercase();
    let cat = category.to_lowercase();

    apps.iter()
    .filter(|a| {
        // Filtr kategorii
        let cat_ok = cat == "all" || a.categories.iter()
        .any(|c| c.to_lowercase().contains(&cat));
        // Filtr wyszukiwania
        let q_ok = q.is_empty()
        || a.name.to_lowercase().contains(&q)
        || a.comment.to_lowercase().contains(&q);
        cat_ok && q_ok
    })
    .map(|a| FsAppItem {
        name: SharedString::from(a.name.as_str()),
         icon: a.icon.clone(),
         exec: SharedString::from(a.exec.as_str()),
    })
    .collect()
}

// ── Pozycjonowanie CompactAppMenu ─────────────────────────
// Lewy dolny róg nad przyciskiem launchera (48px od lewej, nad panelem)

fn position_compact_window(w: &CompactAppMenu) {
    use crate::panel::detect_screen_size;
    let (_, sh) = detect_screen_size();
    // x=8, y = screen_h - 42(panel) - 520(menu_height) - 8(gap)
    let y = (sh - 42 - 520 - 8).max(0) as i32;
    w.window().set_position(slint::PhysicalPosition { x: 8, y });
}

// ── Ikona ─────────────────────────────────────────────────

fn resolve_icon(name: &str) -> slint::Image {
    if name.starts_with('/') {
        let p = std::path::Path::new(name);
        if p.exists() {
            if let Ok(img) = slint::Image::load_from_path(p) { return img; }
        }
    }
    let sizes  = ["48x48", "32x32", "64x64", "128x128", "scalable"];
    let themes = ["hicolor", "breeze", "Papirus", "Adwaita"];
    let exts   = ["png", "svg", "xpm"];
    let base   = std::path::Path::new("/usr/share/icons");
    for theme in &themes {
        for size in &sizes {
            for cat in &["apps", "categories", "mimetypes"] {
                for ext in &exts {
                    let p = base.join(theme).join(size).join(cat)
                    .join(format!("{}.{}", name, ext));
                    if p.exists() {
                        if let Ok(img) = slint::Image::load_from_path(&p) { return img; }
                    }
                }
            }
        }
    }
    for ext in &exts {
        let p = std::path::Path::new("/usr/share/pixmaps")
        .join(format!("{}.{}", name, ext));
        if p.exists() {
            if let Ok(img) = slint::Image::load_from_path(&p) { return img; }
        }
    }
    slint::Image::default()
}

// ── Exec ──────────────────────────────────────────────────

pub fn exec_app(exec: &str) {
    let clean: String = {
        let mut s = String::new();
        let mut chars = exec.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' { chars.next(); } else { s.push(c); }
        }
        s.trim().to_string()
    };
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if let Some(bin) = parts.first() {
        std::process::Command::new(bin)
        .args(&parts[1..])
        .env("XDG_CURRENT_DESKTOP", "Blue:HackerOS")
        .spawn()
        .ok();
    }
}

fn wait_ipc() {
    let path = crate::ipc::socket_path();
    for _ in 0..100 {
        if path.exists() { return; }
        std::thread::sleep(Duration::from_millis(100));
    }
    warn!("IPC socket not ready after 10s");
}
