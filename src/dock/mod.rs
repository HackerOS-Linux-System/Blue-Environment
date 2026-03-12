use slint::{ComponentHandle, Image, SharedString, VecModel, ModelRc};
use std::time::Duration;
use tracing::{info, warn};

use crate::ipc::{self, IpcMessage};

slint::include_modules!();

// ── Domyślne wpisy docku ─────────────────────────────────

#[derive(Debug, Clone)]
struct PinnedApp {
    name: String,
    exec: String,
    icon: String,
}

fn default_pinned() -> Vec<PinnedApp> {
    let config_path = dirs_next::config_dir()
    .unwrap_or_default()
    .join("blue-environment")
    .join("dock.toml");

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if let Ok(table) = content.parse::<toml::Value>() {
            if let Some(apps) = table.get("apps").and_then(|v| v.as_array()) {
                let parsed: Vec<PinnedApp> = apps.iter().filter_map(|app| {
                    Some(PinnedApp {
                        name: app.get("name")?.as_str()?.to_string(),
                         exec: app.get("exec")?.as_str()?.to_string(),
                         icon: app.get("icon").and_then(|v| v.as_str())
                         .unwrap_or("application-x-executable").to_string(),
                    })
                }).collect();
                if !parsed.is_empty() {
                    info!("Dock: {} pinned apps from config", parsed.len());
                    return parsed;
                }
            }
        }
    }

    // Popularne aplikacje jako fallback
    vec![
        PinnedApp { name: "Terminal".into(),      exec: "konsole".into(),       icon: "utilities-terminal".into() },
        PinnedApp { name: "Przeglądarka".into(),  exec: "firefox".into(),       icon: "firefox".into() },
        PinnedApp { name: "Pliki".into(),         exec: "dolphin".into(),       icon: "system-file-manager".into() },
        PinnedApp { name: "Edytor kodu".into(),   exec: "kate".into(),          icon: "kate".into() },
        PinnedApp { name: "VS Code".into(),       exec: "code".into(),          icon: "com.visualstudio.code".into() },
        PinnedApp { name: "Ustawienia".into(),    exec: "blue-settings".into(), icon: "preferences-system".into() },
        PinnedApp { name: "Muzyka".into(),        exec: "elisa".into(),         icon: "elisa".into() },
    ]
}

// ── Rozwiązywanie ikony ───────────────────────────────────

fn resolve_icon(name: &str) -> Image {
    if name.starts_with('/') && std::path::Path::new(name).exists() {
        if let Ok(img) = slint::Image::load_from_path(std::path::Path::new(name)) {
            return img;
        }
    }
    let sizes  = ["48x48", "64x64", "32x32", "128x128", "scalable"];
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
                        if let Ok(img) = slint::Image::load_from_path(&p) {
                            return img;
                        }
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
    Image::default()
}

// ── Budowanie wpisów ──────────────────────────────────────

fn build_entries(pinned: &[PinnedApp], wins: &[crate::ipc::WindowInfo]) -> Vec<DockEntry> {
    pinned.iter().map(|app| {
        let exec_bin  = app.exec.split_whitespace().next().unwrap_or(&app.exec);
        let app_low   = app.name.to_lowercase();
        let exec_low  = exec_bin.to_lowercase();

        let running = wins.iter().any(|w| {
            let id = w.app_id.to_lowercase();
            id.contains(&app_low) || id.contains(&exec_low)
            || app_low.contains(&id) || exec_low.contains(&id)
        });
        let active = wins.iter().any(|w| {
            let id = w.app_id.to_lowercase();
            w.focused && (id.contains(&app_low) || id.contains(&exec_low))
        });

        DockEntry {
            exec:    SharedString::from(app.exec.as_str()),
                      name:    SharedString::from(app.name.as_str()),
                      icon:    resolve_icon(&app.icon),
                      running,
                      active,
        }
    }).collect()
}

// ── Główna funkcja ────────────────────────────────────────

pub fn run() {
    info!("Dock: starting...");
    wait_ipc();

    let pinned = default_pinned();
    info!("Dock: {} pinned apps", pinned.len());

    let dock = match BlueDock::new() {
        Ok(d) => d,
        Err(e) => { warn!("BlueDock::new() failed: {}", e); return; }
    };

    // Początkowe wpisy (brak okien)
    dock.set_entries(ModelRc::new(VecModel::from(
        build_entries(&pinned, &[])
    )));

    // ── App clicked ──────────────────────────────────────
    dock.on_app_clicked(|exec| {
        crate::app_menu::exec_app(&exec.to_string());
    });

    // ── IPC channel: WindowListUpdate ────────────────────
    let (win_tx, win_rx) = std::sync::mpsc::channel::<Vec<crate::ipc::WindowInfo>>();
    {
        let tx = win_tx;
        std::thread::spawn(move || {
            ipc::listen(move |msg| {
                if let IpcMessage::WindowListUpdate { windows } = msg {
                    tx.send(windows).ok();
                }
            });
        });
    }

    // ── Timer: update running/active + auto-hide ──────────
    let dw = dock.as_weak();
    let pinned_c = pinned.clone();
    let mut current_wins: Vec<crate::ipc::WindowInfo> = Vec::new();
    let mut hover_ticks: u32 = 0;   // countdown po opuszczeniu hover

    let main_timer = slint::Timer::default();
    main_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(120),
                     move || {
                         let Some(d) = dw.upgrade() else { return };

                         // Odbierz najnowszy stan okien
                         let mut got_update = false;
                         while let Ok(wins) = win_rx.try_recv() {
                             current_wins = wins;
                             got_update = true;
                         }
                         if got_update {
                             let entries = build_entries(&pinned_c, &current_wins);
                             d.set_entries(ModelRc::new(VecModel::from(entries)));
                         }

                         // Auto-hide logic:
                         // visible  gdy: brak okien LUB hover (hover_ticks > 0)
                         // hidden   gdy: są okna I brak hoveru przez > 1s
                         let has_wins = !current_wins.is_empty();
                         if hover_ticks > 0 { hover_ticks -= 1; }

                         let should_show = !has_wins || hover_ticks > 0;
                         let cur_hide    = d.get_hide_progress();

                         if should_show  && cur_hide > 0.01 { d.set_hide_progress(0.0); }
                         if !should_show && cur_hide < 0.99 { d.set_hide_progress(1.0); }
                     },
    );
    std::mem::forget(main_timer);

    // Callback hover z docku → resetuj countdown
    let dw2 = dock.as_weak();
    let mut hover_ticks_shared = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    // Uwaga: hover_ticks jest w timerze — używamy property docku jako sygnał
    dock.on_dock_hovered(move |hovered| {
        if let Some(d) = dw2.upgrade() {
            if hovered {
                d.set_hide_progress(0.0);
            }
        }
    });

    // ── Pozycjonowanie: centrum-dół, nad panelem ──────────
    {
        let dw3 = dock.as_weak();
        let pos_timer = slint::Timer::default();
        pos_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
                        move || {
                            let Some(d) = dw3.upgrade() else { return };
                            let (sw, sh) = crate::panel::detect_screen_size();
                            // y = sh - 42 (panel) - 72 (dock height) - 6 (gap) = sh - 120
                            let target_y = (sh - 120) as i32;
                            let cur = d.window().position();
                            if cur.x != 0 || (cur.y - target_y).abs() > 2 {
                                d.window().set_size(slint::PhysicalSize {
                                    width:  sw as u32,
                                    height: 72,
                                });
                                d.window().set_position(slint::PhysicalPosition {
                                    x: 0,
                                    y: target_y,
                                });
                            }
                        },
        );
        std::mem::forget(pos_timer);
    }

    dock.show().ok();
    info!("Dock ready");
    slint::run_event_loop().ok();
}

fn wait_ipc() {
    let path = crate::ipc::socket_path();
    for _ in 0..100 {
        if path.exists() { return; }
        std::thread::sleep(Duration::from_millis(100));
    }
    warn!("IPC socket not ready after 10s");
}
