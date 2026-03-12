use chrono::Local;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::time::Duration;
use tracing::info;

use crate::ipc::{self, IpcMessage};

slint::include_modules!();

pub fn launch(wayland_display: String) {
    std::thread::spawn(move || {
        std::env::set_var("WAYLAND_DISPLAY", &wayland_display);
        std::thread::sleep(Duration::from_millis(500));
        if let Err(e) = run_panel() {
            tracing::error!("Panel error: {}", e);
        }
    });
}

pub fn run_panel() -> anyhow::Result<()> {
    let panel = BluePanel::new()?;

    // ── Zegar (co 1s) ─────────────────────────────────────
    {
        let pw = panel.as_weak();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || {
            let Some(p) = pw.upgrade() else { return };
            let now = Local::now();
            p.set_clock_time(SharedString::from(now.format("%H:%M").to_string().as_str()));
            p.set_clock_date(SharedString::from(now.format("%a %d %b").to_string().as_str()));
            update_calendar(&p, &now);
        });
        std::mem::forget(timer);
    }

    // ── Sieć (co 5s) ──────────────────────────────────────
    {
        let pw = panel.as_weak();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(5), move || {
            if let Some(p) = pw.upgrade() { update_network_status(&p); }
        });
        std::mem::forget(timer);
    }

    // ── Bateria (co 30s) ──────────────────────────────────
    {
        let pw = panel.as_weak();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(30), move || {
            if let Some(p) = pw.upgrade() { update_battery(&p); }
        });
        std::mem::forget(timer);
        // Pierwszy odczyt natychmiast
        if let Some(p) = panel.as_weak().upgrade() { update_battery(&p); }
    }

    // ── Głośność (co 3s) ──────────────────────────────────
    {
        let pw = panel.as_weak();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(3), move || {
            if let Some(p) = pw.upgrade() { update_volume(&p); }
        });
        std::mem::forget(timer);
        if let Some(p) = panel.as_weak().upgrade() { update_volume(&p); }
    }

    // ── Bluetooth (co 10s) ────────────────────────────────
    {
        let pw = panel.as_weak();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(10), move || {
            if let Some(p) = pw.upgrade() { update_bluetooth(&p); }
        });
        std::mem::forget(timer);
        if let Some(p) = panel.as_weak().upgrade() { update_bluetooth(&p); }
    }

    // ── IPC przez channel + Timer ─────────────────────────
    {
        use crate::ipc::IpcMessage as M;
        #[allow(clippy::large_enum_variant)]
        enum PanelEvent {
            Workspace { active: usize, total: usize },
            Windows(Vec<crate::ipc::WindowInfo>),
        }
        let (tx, rx) = std::sync::mpsc::channel::<PanelEvent>();

        std::thread::spawn(move || {
            ipc::listen(move |msg| {
                match msg {
                    M::WorkspaceChanged { active, total } => {
                        tx.send(PanelEvent::Workspace { active, total }).ok();
                    }
                    M::WindowListUpdate { windows } => {
                        tx.send(PanelEvent::Windows(windows)).ok();
                    }
                    _ => {}
                }
            });
        });

        let pw = panel.as_weak();
        let ipc_timer = slint::Timer::default();
        ipc_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
                        move || {
                            let Some(p) = pw.upgrade() else { return };
                            while let Ok(ev) = rx.try_recv() {
                                match ev {
                                    PanelEvent::Workspace { active, total } => {
                                        p.set_active_workspace(active as i32);
                                        p.set_workspace_count(total as i32);
                                    }
                                    PanelEvent::Windows(windows) => {
                                        let items: Vec<WindowItem> = windows.iter()
                                        .map(|w| WindowItem {
                                            id:        w.id as i32,
                                            title:     SharedString::from(w.title.as_str()),
                                             focused:   w.focused,
                                             minimized: w.minimized,
                                        })
                                        .collect();
                                        p.set_open_windows(ModelRc::new(VecModel::from(items)));
                                    }
                                }
                            }
                        },
        );
        std::mem::forget(ipc_timer);
    }

    // ── Callbacks ─────────────────────────────────────────
    panel.on_launcher_clicked(|| { ipc::send(&IpcMessage::ToggleAppMenu); });
    panel.on_workspace_clicked(|i| { ipc::send(&IpcMessage::SwitchWorkspace { index: i as usize }); });
    panel.on_window_clicked(|id| { ipc::send(&IpcMessage::FocusWindow { id: id as u64 }); });
    panel.on_settings_clicked(|| {
        let _ = std::process::Command::new("blue-settings").spawn();
    });
    panel.on_show_power_menu(|| { ipc::send(&IpcMessage::ShowLogoutDialog); });
    panel.on_notifications_clicked(|| { ipc::send(&IpcMessage::ToggleNotificationCenter); });
    panel.on_network_clicked(|| {
        let _ = std::process::Command::new("nm-connection-editor")
        .spawn()
        .or_else(|_| std::process::Command::new("plasma-nm").spawn());
    });
    panel.on_bluetooth_clicked(|| {
        let _ = std::process::Command::new("bluedevil-wizard")
        .spawn()
        .or_else(|_| std::process::Command::new("blueman-manager").spawn());
    });
    panel.on_tray_clicked(|_| {});

    // Głośność: zmiana z suwaka
    {
        let pw = panel.as_weak();
        panel.on_volume_changed(move |val| {
            let pct = (val * 100.0).round() as u32;
            // pactl / wpctl set-volume
            let _ = std::process::Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{}%", pct)])
            .spawn()
            .or_else(|_| std::process::Command::new("pactl")
            .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", pct)])
            .spawn());
            if let Some(p) = pw.upgrade() {
                p.set_volume(val);
            }
        });
    }
    // Mute toggle
    {
        let pw2 = panel.as_weak();
        panel.on_volume_mute_toggle(move || {
            let _ = std::process::Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
            .spawn()
            .or_else(|_| std::process::Command::new("pactl")
            .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
            .spawn());
            if let Some(p) = pw2.upgrade() {
                update_volume(&p);
            }
        });
    }
    // Stary volume-clicked (fallback pavucontrol)
    panel.on_volume_clicked(|| {
        // Obsłużone przez popup — fallback jeśli popup nie działa
    });

    // ── Init ──────────────────────────────────────────────
    update_tray(&panel);
    update_network_status(&panel);
    update_battery(&panel);
    update_volume(&panel);
    update_bluetooth(&panel);
    let now = Local::now();
    panel.set_clock_time(SharedString::from(now.format("%H:%M").to_string().as_str()));
    panel.set_clock_date(SharedString::from(now.format("%a %d %b").to_string().as_str()));
    update_calendar(&panel, &now);

    info!("Panel uruchomiony");

    // ── Pozycjonowanie na dole ekranu ─────────────────────
    {
        let (screen_w, screen_h) = detect_screen_size();
        info!("Screen size detected: {}x{}", screen_w, screen_h);
        let pw = panel.as_weak();
        let pos_timer = slint::Timer::default();
        pos_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
                        move || {
                            let Some(p) = pw.upgrade() else { return };
                            let cur_pos  = p.window().position();
                            let target_y = (screen_h - 42) as i32;
                            if cur_pos.x != 0 || (cur_pos.y - target_y).abs() > 2 {
                                p.window().set_size(slint::PhysicalSize {
                                    width:  screen_w as u32,
                                    height: 42,
                                });
                                p.window().set_position(slint::PhysicalPosition {
                                    x: 0,
                                    y: target_y,
                                });
                            }
                        },
        );
        std::mem::forget(pos_timer);
    }

    panel.show()?;

    // Wymusz pozycję przez xdotool (KDE/X11 fallback) po 300ms
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = std::process::Command::new("xdotool")
        .args(["search", "--name", "BluePanel",
              "windowmove", "0", "--sync"])
        .spawn();
    });

    slint::run_event_loop()?;
    Ok(())
}

// ══════════════════════════════════════════════════════════
// Bateria — /sys/class/power_supply/
// ══════════════════════════════════════════════════════════

fn update_battery(panel: &BluePanel) {
    let supply_dir = std::path::Path::new("/sys/class/power_supply");
    if !supply_dir.exists() {
        panel.set_bat_present(false);
        return;
    }

    // Znajdź BAT* lub battery*
    let bat_path = std::fs::read_dir(supply_dir)
    .ok()
    .and_then(|mut d| d.find_map(|e| {
        let e = e.ok()?;
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("BAT") || name.to_lowercase().starts_with("battery") {
            Some(e.path())
        } else {
            None
        }
    }));

    let Some(bat) = bat_path else {
        panel.set_bat_present(false);
        return;
    };

    let read = |f: &str| -> Option<String> {
        std::fs::read_to_string(bat.join(f))
        .ok()
        .map(|s| s.trim().to_string())
    };

    // Procent
    let percent = read("capacity")
    .and_then(|s| s.parse::<i32>().ok())
    .unwrap_or(0)
    .clamp(0, 100);

    // Status: Charging / Discharging / Full / Unknown
    let status = read("status").unwrap_or_else(|| "Unknown".to_string());
    let charging = status == "Charging";
    let full     = status == "Full";

    panel.set_bat_present(true);
    panel.set_bat_percent(percent);
    panel.set_bat_charging(charging);
    panel.set_bat_full(full);
}

// ══════════════════════════════════════════════════════════
// Głośność — wpctl (PipeWire) lub pactl (PulseAudio)
// ══════════════════════════════════════════════════════════

fn update_volume(panel: &BluePanel) {
    // Próbuj wpctl (PipeWire — nowszy, preferowany)
    if let Some((vol, muted)) = read_volume_wpctl() {
        panel.set_volume(vol);
        panel.set_muted(muted);
        return;
    }
    // Fallback: pactl
    if let Some((vol, muted)) = read_volume_pactl() {
        panel.set_volume(vol);
        panel.set_muted(muted);
    }
}

fn read_volume_wpctl() -> Option<(f32, bool)> {
    let out = std::process::Command::new("wpctl")
    .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
    .output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    // Format: "Volume: 0.75" lub "Volume: 0.75 [MUTED]"
    let muted = text.contains("[MUTED]");
    let vol: f32 = text
    .split_whitespace()
    .nth(1)?
    .parse().ok()?;
    Some((vol.clamp(0.0, 1.0), muted))
}

fn read_volume_pactl() -> Option<(f32, bool)> {
    let out = std::process::Command::new("pactl")
    .args(["get-sink-volume", "@DEFAULT_SINK@"])
    .output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    // Format: "Volume: front-left: 65536 /  100% / 0.00 dB,   front-right: ..."
    let pct: f32 = text
    .split('/')
    .nth(1)?
    .trim()
    .trim_end_matches('%')
    .parse().ok()?;

    let muted_out = std::process::Command::new("pactl")
    .args(["get-sink-mute", "@DEFAULT_SINK@"])
    .output().ok()?;
    let muted_text = String::from_utf8(muted_out.stdout).ok()?;
    let muted = muted_text.contains("yes");

    Some((pct / 100.0, muted))
}

// ══════════════════════════════════════════════════════════
// Bluetooth — D-Bus org.bluez
// ══════════════════════════════════════════════════════════

fn update_bluetooth(panel: &BluePanel) {
    // Sprawdź czy bluetooth adapter istnieje przez rfkill/sys
    let bt_enabled = std::path::Path::new("/sys/class/bluetooth").exists()
    && std::fs::read_dir("/sys/class/bluetooth")
    .map(|d| d.count() > 0)
    .unwrap_or(false);

    if !bt_enabled {
        panel.set_bt_enabled(false);
        panel.set_bt_connected(false);
        panel.set_bt_device(SharedString::from(""));
        return;
    }

    panel.set_bt_enabled(true);

    // Sprawdź podłączone urządzenia przez bluetoothctl
    let connected = if let Ok(out) = std::process::Command::new("bluetoothctl")
    .args(["info"])
    .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if text.contains("Connected: yes") {
            // Wyciągnij nazwę urządzenia
            let name = text.lines()
            .find(|l| l.trim().starts_with("Name:"))
            .map(|l| l.trim().trim_start_matches("Name:").trim().to_string())
            .unwrap_or_default();
            panel.set_bt_device(SharedString::from(name.as_str()));
            true
        } else {
            panel.set_bt_device(SharedString::from(""));
            false
        }
    } else {
        // Fallback: sprawdź przez /sys
        let connected = std::fs::read_dir("/sys/class/bluetooth")
        .ok()
        .map(|d| d.filter_map(|e| e.ok()).any(|e| {
            std::fs::read_to_string(e.path().join("connected"))
            .map(|s| s.trim() == "1")
            .unwrap_or(false)
        }))
        .unwrap_or(false);
        panel.set_bt_device(SharedString::from(""));
        connected
    };

    panel.set_bt_connected(connected);
}

// ══════════════════════════════════════════════════════════
// System Tray
// ══════════════════════════════════════════════════════════

fn update_tray(panel: &BluePanel) {
    let daemons: &[(&str, &str)] = &[
        ("pipewire",       "🎵"),
        ("NetworkManager", "📡"),
        ("blueman-applet", "🔷"),
    ];
    let items: Vec<TrayEntry> = daemons.iter()
    .filter(|(n, _)| process_running(n))
    .map(|(n, i)| TrayEntry {
        name: SharedString::from(*n),
         icon: SharedString::from(*i),
    })
    .collect();
    panel.set_tray_items(ModelRc::new(VecModel::from(items)));
}

fn process_running(name: &str) -> bool {
    std::fs::read_dir("/proc")
    .map(|d| d.filter_map(|e| e.ok()).any(|e| {
        std::fs::read_to_string(e.path().join("comm"))
        .map(|s| s.trim() == name)
        .unwrap_or(false)
    }))
    .unwrap_or(false)
}

// ══════════════════════════════════════════════════════════
// Sieć
// ══════════════════════════════════════════════════════════

fn update_network_status(panel: &BluePanel) {
    let (connected, wifi, ssid, strength) = detect_network();
    panel.set_net_connected(connected);
    panel.set_net_wifi(wifi);
    panel.set_net_ssid(SharedString::from(ssid.as_str()));
    panel.set_net_strength(strength);
}

fn detect_network() -> (bool, bool, String, i32) {
    let net_dir = std::path::Path::new("/sys/class/net");
    if !net_dir.exists() { return (false, false, String::new(), 0); }

    if let Ok(entries) = std::fs::read_dir(net_dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let name  = e.file_name().to_string_lossy().to_string();
            let state = std::fs::read_to_string(e.path().join("operstate"))
            .unwrap_or_default();
            if state.trim() != "up" { continue; }
            if name.starts_with("wl") {
                let ssid = std::process::Command::new("iwgetid")
                .args([&name, "-r"]).output().ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
                return (true, true, ssid, parse_wifi_strength(&name));
            }
            if name.starts_with("en") || name.starts_with("eth") {
                return (true, false, name, 100);
            }
        }
    }
    (false, false, String::new(), 0)
}

fn parse_wifi_strength(iface: &str) -> i32 {
    std::fs::read_to_string("/proc/net/wireless")
    .unwrap_or_default()
    .lines()
    .find(|l| l.contains(iface))
    .and_then(|l| l.split_whitespace().nth(3))
    .and_then(|s| s.trim_end_matches('.').parse::<f32>().ok())
    .map(|v| ((v + 90.0) / 60.0 * 100.0).clamp(0.0, 100.0) as i32)
    .unwrap_or(50)
}

// ══════════════════════════════════════════════════════════
// Kalendarz
// ══════════════════════════════════════════════════════════

fn update_calendar(panel: &BluePanel, now: &chrono::DateTime<Local>) {
    use chrono::{Datelike, NaiveDate};

    let year  = now.year();
    let month = now.month();
    let today = now.day() as i32;

    let months = ["Styczeń","Luty","Marzec","Kwiecień","Maj","Czerwiec",
    "Lipiec","Sierpień","Wrzesień","Październik","Listopad","Grudzień"];
    panel.set_calendar_month(SharedString::from(
        format!("{} {}", months[(month - 1) as usize], year).as_str(),
    ));
    panel.set_today_day(today);

    let first       = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let start_off   = first.weekday().num_days_from_monday() as i32;
    let days_in_mon = month_days(year, month) as i32;
    let days_prev   = month_days(year, if month == 1 { 12 } else { month - 1 }) as i32;

    let cells: Vec<CalDay> = (0..42i32).map(|i| {
        let d = i - start_off + 1;
        if d < 1 {
            CalDay { day: days_prev + d,   other_month: true,  today: false }
        } else if d > days_in_mon {
            CalDay { day: d - days_in_mon, other_month: true,  today: false }
        } else {
            CalDay { day: d,               other_month: false, today: d == today }
        }
    }).collect();

    panel.set_calendar_days(ModelRc::new(VecModel::from(cells)));
}

fn month_days(year: i32, month: u32) -> u32 {
    use chrono::NaiveDate;
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1).unwrap()
    .signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
    .num_days() as u32
}

// ══════════════════════════════════════════════════════════
// Wykrywanie rozdzielczości ekranu
// ══════════════════════════════════════════════════════════

pub fn detect_screen_size() -> (i32, i32) {
    // 1. xrandr (X11 / XWayland)
    if let Ok(out) = std::process::Command::new("xrandr").args(["--current"]).output() {
        if let Ok(text) = String::from_utf8(out.stdout) {
            for line in text.lines() {
                if line.contains(" connected") {
                    if let Some(res) = line.split_whitespace()
                        .find(|s| s.contains('x') && s.chars().next()
                        .map(|c| c.is_ascii_digit()).unwrap_or(false))
                        {
                            let part = res.split('+').next().unwrap_or(res);
                            let mut p = part.splitn(2, 'x');
                            if let (Some(w), Some(h)) = (p.next(), p.next()) {
                                if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                                    if w > 100 && h > 100 { return (w, h); }
                                }
                            }
                        }
                }
            }
        }
    }
    // 2. wlr-randr (wlroots Wayland)
    if let Ok(out) = std::process::Command::new("wlr-randr").output() {
        if let Ok(text) = String::from_utf8(out.stdout) {
            for line in text.lines() {
                if line.contains("current") {
                    for token in line.split_whitespace() {
                        if token.contains('x') {
                            let mut p = token.splitn(2, 'x');
                            if let (Some(w), Some(h)) = (p.next(), p.next()) {
                                let h = h.split('@').next().unwrap_or(h);
                                if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                                    if w > 100 && h > 100 { return (w, h); }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // 3. kscreen-doctor (KDE Plasma Wayland)
    if let Ok(out) = std::process::Command::new("kscreen-doctor").args(["-o"]).output() {
        if let Ok(text) = String::from_utf8(out.stdout) {
            for line in text.lines() {
                for token in line.split_whitespace() {
                    if let Some(res) = token.split('@').next() {
                        if res.contains('x') {
                            let mut p = res.splitn(2, 'x');
                            if let (Some(w), Some(h)) = (p.next(), p.next()) {
                                if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                                    if w > 100 && h > 100 { return (w, h); }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // 4. /sys/class/drm
    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.filter_map(|e| e.ok()) {
            let modes_path = entry.path().join("modes");
            if let Ok(modes) = std::fs::read_to_string(&modes_path) {
                if let Some(first) = modes.lines().next() {
                    let mut p = first.splitn(2, 'x');
                    if let (Some(w), Some(h)) = (p.next(), p.next()) {
                        if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                            if w > 100 && h > 100 { return (w, h); }
                        }
                    }
                }
            }
        }
    }
    (1920, 1080)
}
