use blue_environment::notification::{Store, Urgency, STORE};

use anyhow::Result;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

// Generuje: NotificationOverlay, NotificationCenter,
//           PopupItem, ActionItem, CenterItem
slint::include_modules!();

fn main() -> Result<()> {
    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::new("info"))
    .without_time()
    .init();

    info!("blue-notificationd starting...");

    // Lock plik — jeden daemon
    let uid       = nix::unistd::getuid().as_raw();
    let lock_path = format!("/run/user/{}/blue-notificationd.lock", uid);
    if std::path::Path::new(&lock_path).exists() { return Ok(()); }
    std::fs::write(&lock_path, std::process::id().to_string())?;
    let _lock = LockGuard(lock_path);

    // D-Bus w wątku tokio
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("tokio");
        rt.block_on(async {
            if let Err(e) = blue_environment::notification::dbus::run().await {
                tracing::error!("D-Bus: {}", e);
            }
        });
    });

    let overlay = NotificationOverlay::new()?;
    let center  = NotificationCenter::new()?;

    // Timer 500ms
    {
        let ow = overlay.as_weak();
        let cw = center.as_weak();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || {
            { STORE.lock().close_expired(); }
            let store = STORE.lock();
            if let Some(o) = ow.upgrade() { update_overlay(&o, &store); }
            if let Some(c) = cw.upgrade() { update_center(&c, &store); }
        });
        std::mem::forget(timer);
    }

    // Overlay callbacks
    {
        let ow = overlay.as_weak();
        overlay.on_popup_dismissed(move |id| {
            STORE.lock().close(id as u32);
            if let Some(o) = ow.upgrade() { update_overlay(&o, &STORE.lock()); }
        });
    }
    overlay.on_popup_action(|id, key| { info!("Action '{}' on #{}", key, id); });
    {
        let cw = center.as_weak();
        overlay.on_open_center(move || {
            if let Some(c) = cw.upgrade() { c.show().ok(); }
        });
    }

    // Center callbacks
    {
        let cw = center.as_weak();
        center.on_close_panel(move || {
            if let Some(c) = cw.upgrade() { c.hide().ok(); }
        });
    }
    {
        let cw = center.as_weak();
        center.on_dismiss(move |id| {
            STORE.lock().close(id as u32);
            if let Some(c) = cw.upgrade() { update_center(&c, &STORE.lock()); }
        });
    }
    {
        let cw = center.as_weak();
        center.on_clear_all(move || {
            STORE.lock().clear_all();
            if let Some(c) = cw.upgrade() { update_center(&c, &STORE.lock()); }
        });
    }

    overlay.show()?;
    center.hide()?;
    info!("blue-notificationd ready");
    overlay.run()?;
    Ok(())
}

// ── Overlay ───────────────────────────────────────────────

fn update_overlay(overlay: &NotificationOverlay, store: &Store) {
    let popups: Vec<PopupItem> = store.active_popups().iter().map(|n| PopupItem {
        nid:      n.id as i32,
        app_name: SharedString::from(n.app_name.as_str()),
                                                                  summary:  SharedString::from(n.summary.as_str()),
                                                                  body:     SharedString::from(n.body.as_str()),
                                                                  accent:   n.urgency.accent_color(),
                                                                  critical: n.urgency == Urgency::Critical,
                                                                  actions:  ModelRc::new(VecModel::from(
                                                                      n.actions.iter()
                                                                      .map(|(k, l)| ActionItem {
                                                                          key:   SharedString::from(k.as_str()),
                                                                           label: SharedString::from(l.as_str()),
                                                                      })
                                                                      .collect::<Vec<_>>(),
                                                                  )),
    }).collect();
    overlay.set_popups(ModelRc::new(VecModel::from(popups)));
    overlay.set_unread_count(store.items.len() as i32);
}

// ── Center ────────────────────────────────────────────────

fn update_center(center: &NotificationCenter, store: &Store) {
    let items: Vec<CenterItem> = store.items.iter().map(|n| CenterItem {
        id:       n.id as i32,
        app_name: SharedString::from(n.app_name.as_str()),
                                                        summary:  SharedString::from(n.summary.as_str()),
                                                        body:     SharedString::from(n.body.as_str()),
                                                        age:      SharedString::from(n.age_str().as_str()),
                                                        accent:   n.urgency.accent_color(),
                                                        critical: n.urgency == Urgency::Critical,
    }).collect();
    center.set_items(ModelRc::new(VecModel::from(items)));
}

struct LockGuard(String);
impl Drop for LockGuard {
    fn drop(&mut self) { std::fs::remove_file(&self.0).ok(); }
}
