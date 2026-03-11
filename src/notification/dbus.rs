use anyhow::Result;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info};
use zbus::{interface, Connection, SignalContext};
use zvariant::Value;

use super::{strip_markup, Notification, Urgency, STORE};

pub struct NotificationsServer;

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationsServer {
    // ── Capabilities ──────────────────────────────────────
    fn get_capabilities(&self) -> Vec<&str> {
        vec!["body", "body-markup", "actions", "persistence",
        "icon-static", "action-icons", "sound"]
    }

    // ── Główna metoda: przyjmij powiadomienie ─────────────
    fn notify(
        &self,
        app_name:        &str,
        replaces_id:     u32,
        app_icon:        &str,
        summary:         &str,
        body:            &str,
        actions:         Vec<String>,
        hints:           HashMap<String, Value<'_>>,
        expire_timeout:  i32,
    ) -> u32 {
        debug!("notify() from '{}': {}", app_name, summary);

        // Urgency z hints (byte 0/1/2)
        let urgency = hints
        .get("urgency")
        .and_then(|v| if let Value::U8(u) = v { Some(*u) } else { None })
        .map(Urgency::from_u8)
        .unwrap_or_default();

        // Timeout: -1 = default, 0 = nigdy, >0 = ms
        let expire_ms = match expire_timeout {
            0  => None,
            -1 => urgency.default_timeout_ms(),
            ms => Some(ms as u64),
        };

        // Actions: ["key1","Label1","key2","Label2",...]
        let action_pairs: Vec<(String, String)> = actions
        .chunks(2)
        .filter_map(|c| if c.len() == 2 { Some((c[0].clone(), c[1].clone())) } else { None })
        .collect();

        let n = Notification {
            id: 0,
            app_name:    app_name.to_string(),
            app_icon:    app_icon.to_string(),
            summary:     summary.to_string(),
            body:        strip_markup(body),
            urgency,
            actions:     action_pairs,
            expire_ms,
            created_at:  Instant::now(),
            replaces_id,
        };

        let id = STORE.lock().add(n);
        info!("#{} [{}] {} — {}", id, app_name, summary, body);
        id
    }

    // ── Zamknij konkretne powiadomienie ───────────────────
    fn close_notification(&self, id: u32) {
        debug!("close_notification({})", id);
        STORE.lock().close(id);
    }

    // ── Info o serwerze ───────────────────────────────────
    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("blue-notificationd", "HackerOS", "0.1.0", "1.2")
    }

    // ── Sygnały ───────────────────────────────────────────
    #[zbus(signal)]
    async fn notification_closed(
        ctxt: &SignalContext<'_>,
        id: u32, reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        ctxt: &SignalContext<'_>,
        id: u32, action_key: &str,
    ) -> zbus::Result<()>;
}

// ── Uruchom serwer D-Bus ──────────────────────────────────

pub async fn run() -> Result<()> {
    let conn = Connection::session().await?;

    conn.object_server()
    .at("/org/freedesktop/Notifications", NotificationsServer)
    .await?;

    conn.request_name("org.freedesktop.Notifications").await?;

    info!("D-Bus: org.freedesktop.Notifications zarejestrowany");
    // Trzymaj connection przy życiu
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
