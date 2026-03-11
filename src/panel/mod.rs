use chrono::Local;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::time::Duration;
use tracing::info;

use crate::ipc::{self, IpcMessage};

// Generuje wszystkie typy z ui/all.slint (BluePanel, WindowItem, TrayEntry, CalDay, ...)
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

fn run_panel() -> anyhow::Result<()> {
    let panel = BluePanel::new()?;

    // ── Zegar ─────────────────────────────────────────────
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

    // ── Sieć ─────────────────────────────────────────────
    {
        let pw = panel.as_weak();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(5), move || {
            if let Some(p) = pw.upgrade() { update_network_status(&p); }
        });
        std::mem::forget(timer);
    }

    // ── IPC ───────────────────────────────────────────────
    {
        let pw = panel.as_weak();
        std::thread::spawn(move || {
            ipc::listen(move |msg| {
                let pw2 = pw.clone();
                match msg {
                    IpcMessage::WorkspaceChanged { active, total } => {
                        slint::invoke_from_event_loop(move || {
                            let Some(p) = pw2.upgrade() else { return };
                            p.set_active_workspace(active as i32);
                            p.set_workspace_count(total as i32);
                        }).ok();
                    }
                    IpcMessage::WindowListUpdate { windows } => {
                        slint::invoke_from_event_loop(move || {
                            let Some(p) = pw2.upgrade() else { return };
                            let items: Vec<WindowItem> = windows.iter()
                            .map(|w| WindowItem {
                                id:        w.id as i32,
                                title:     SharedString::from(w.title.as_str()),
                                 focused:   w.focused,
                                 minimized: w.minimized,
                            })
                            .collect();
                            p.set_open_windows(ModelRc::new(VecModel::from(items)));
                        }).ok();
                    }
                    _ => {}
                }
            });
        });
    }

    // ── Callbacks ─────────────────────────────────────────
    panel.on_launcher_clicked(|| { ipc::send(&IpcMessage::ToggleAppMenu); });
    panel.on_workspace_clicked(|i| { ipc::send(&IpcMessage::SwitchWorkspace { index: i as usize }); });
    panel.on_window_clicked(|id| { ipc::send(&IpcMessage::FocusWindow { id: id as u64 }); });
    panel.on_settings_clicked(|| { let _ = std::process::Command::new("blue-settings").spawn(); });
    panel.on_show_power_menu(|| { ipc::send(&IpcMessage::ShowLogoutDialog); });
    panel.on_notifications_clicked(|| { ipc::send(&IpcMessage::ToggleNotificationCenter); });
    panel.on_volume_clicked(|| {
        let _ = std::process::Command::new("pavucontrol").spawn()
        .or_else(|_| std::process::Command::new("alsamixer").spawn());
    });
    panel.on_network_clicked(|| { let _ = std::process::Command::new("nm-connection-editor").spawn(); });
    panel.on_tray_clicked(|_| {});

    // ── Init ─────────────────────────────────────────────
    update_tray(&panel);
    update_network_status(&panel);
    let now = Local::now();
    panel.set_clock_time(SharedString::from(now.format("%H:%M").to_string().as_str()));
    panel.set_clock_date(SharedString::from(now.format("%a %d %b").to_string().as_str()));
    update_calendar(&panel, &now);

    info!("Panel uruchomiony");
    panel.run()?;
    Ok(())
}

// ── Tray ──────────────────────────────────────────────────

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

// ── Sieć ─────────────────────────────────────────────────

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

// ── Kalendarz ─────────────────────────────────────────────

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
