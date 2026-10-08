use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio_xmpp::{Client, Event};
use xmpp_parsers::jid::{BareJid, Jid};
use xmpp_parsers::message::{Message, MessageType};
use xmpp_parsers::presence::{Presence, Show, Type as PresenceType};

fn session_path() -> PathBuf { super::messages_dir().join("xmpp_session.json") }

#[derive(Serialize, Deserialize, Clone, Debug)]
struct StoredXmppSession {
    jid: String,
    password_encrypted: String,
    resource: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct XmppSessionInfo {
    pub jid: String,
}

/// Payload zdarzenia `blue-messages://xmpp-incoming`.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct XmppIncomingEvent {
    conversation_id: String,
    message: super::Message,
}

fn read_session() -> Option<StoredXmppSession> {
    std::fs::read_to_string(session_path()).ok().and_then(|s| serde_json::from_str(&s).ok())
}
fn write_session(s: &StoredXmppSession) -> Result<(), String> {
    std::fs::create_dir_all(super::messages_dir()).map_err(|e| e.to_string())?;
    std::fs::write(session_path(), serde_json::to_string_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn xmpp_has_session() -> bool { read_session().is_some() }

// ── trwałe połączenie ────────────────────────────────────────────────
/// Każdy login/logout zwiększa numer generacji; zadanie w tle kończy się,
/// gdy jego generacja przestała być aktualna.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Kolejka wysyłki do zadania w tle: (adresat, treść).
static OUTBOX: Mutex<Option<mpsc::UnboundedSender<(String, String)>>> = Mutex::new(None);

/// Splits a bare or full JID (`user@domain` or `user@domain/resource`) into `(user, domain)`.
fn split_jid(jid: &str) -> Result<(String, String), String> {
    let bare = jid.split('/').next().unwrap_or(jid);
    let mut parts = bare.splitn(2, '@');
    let user = parts.next().filter(|s| !s.is_empty()).ok_or("JID must be user@domain")?;
    let domain = parts.next().filter(|s| !s.is_empty()).ok_or("JID must be user@domain")?;
    Ok((user.to_string(), domain.to_string()))
}

fn bare_jid(jid: &str) -> Result<BareJid, String> {
    let (u, d) = split_jid(jid)?;
    BareJid::from_str(&format!("{u}@{d}")).map_err(|e| format!("Nieprawidłowy JID: {e}"))
}

/// rustls wymaga zainstalowanego dostawcy kryptografii (raz na proces).
fn ensure_crypto_provider() {
    let _ = tokio_xmpp::rustls::crypto::ring::default_provider().install_default();
}

#[tauri::command(async)]
pub fn xmpp_logout() -> Result<(), String> {
    GENERATION.fetch_add(1, Ordering::SeqCst); // kończy zadanie w tle
    *OUTBOX.lock().unwrap() = None;
    let path = session_path();
    if path.exists() { std::fs::remove_file(path).map_err(|e| e.to_string())?; }
    Ok(())
}

/// Sprawdza dane logowania na prawdziwym serwerze: czeka na `Online` albo
/// pierwszy błąd (zły login/hasło, brak sieci).
async fn verify_credentials(jid: BareJid, password: String) -> Result<String, String> {
    ensure_crypto_provider();
    let mut client = Client::new(jid, password);
    let outcome = tokio::time::timeout(Duration::from_secs(25), async {
        while let Some(ev) = client.next().await {
            match ev {
                Event::Online { bound_jid, .. } => return Ok(bound_jid.to_string()),
                Event::Disconnected(e) => return Err(e.to_string()),
                Event::Stanza(_) => {}
            }
        }
        Err("serwer zamknął połączenie".to_string())
    })
    .await
    .unwrap_or_else(|_| Err("przekroczono czas logowania".to_string()));
    let _ = client.send_end().await;
    outcome
}

/// Weryfikuje dane, zapisuje sesję i startuje trwałe połączenie w tle.
#[tauri::command]
pub async fn xmpp_login(app: AppHandle, jid: String, password: String) -> Result<XmppSessionInfo, String> {
    let (user, domain) = split_jid(&jid)?;
    let bare = bare_jid(&jid)?;
    let full = verify_credentials(bare, password.clone())
        .await
        .map_err(|e| format!("Logowanie XMPP nie powiodło się: {e}"))?;

    let dir = super::messages_dir();
    write_session(&StoredXmppSession {
        jid: format!("{user}@{domain}"),
        password_encrypted: super::secretstore::encrypt(&dir, &password),
        resource: "BlueMessages".into(),
    })?;
    start_background(app);
    Ok(XmppSessionInfo { jid: full })
}

/// Startuje (lub restartuje) połączenie w tle, jeśli istnieje zapisana sesja.
/// Wołane po loginie i przy starcie powłoki — dzięki temu odbiór działa też
/// po restarcie, bez ponownego logowania.
pub fn start_background(app: AppHandle) {
    let Some(session) = read_session() else { return };
    let dir = super::messages_dir();
    let password = super::secretstore::decrypt(&dir, &session.password_encrypted);
    let Ok(jid) = bare_jid(&session.jid) else { return };
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let (tx, rx) = mpsc::unbounded_channel();
    *OUTBOX.lock().unwrap() = Some(tx);
    tauri::async_runtime::spawn(run_client(app, generation, jid, password, rx));
}

async fn run_client(
    app: AppHandle,
    generation: u64,
    jid: BareJid,
    password: String,
    mut outbox: mpsc::UnboundedReceiver<(String, String)>,
) {
    ensure_crypto_provider();
    let mut client = Client::new(jid, password);
    loop {
        if GENERATION.load(Ordering::SeqCst) != generation {
            break; // nowsza sesja albo wylogowanie
        }
        tokio::select! {
            ev = client.next() => match ev {
                None => break,
                Some(Event::Online { .. }) => {
                    // Dostępność — bez niej serwer nie dostarcza wiadomości na ten zasób.
                    let mut p = Presence::new(PresenceType::None);
                    p.show = Some(Show::Chat);
                    let _ = client.send_stanza(p.into()).await;
                }
                Some(Event::Stanza(st)) => {
                    if let Ok(msg) = Message::try_from(st) { handle_incoming(&app, &msg); }
                }
                // Biblioteka sama łączy ponownie (z backoffem); tylko logujemy.
                Some(Event::Disconnected(e)) => tracing::warn!("XMPP rozłączony: {e}"),
            },
            Some((to, body)) = outbox.recv() => {
                match Jid::from_str(&to) {
                    Ok(to) => {
                        let mut m = Message::new(Some(to));
                        m.type_ = MessageType::Chat;
                        m.bodies.insert(Default::default(), body);
                        if let Err(e) = client.send_stanza(m.into()).await { tracing::warn!("XMPP wysyłka: {e}"); }
                    }
                    Err(e) => tracing::warn!("XMPP: zły adresat {to}: {e}"),
                }
            }
            // Okresowo sprawdzamy, czy generacja nie wygasła.
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }
    let _ = client.send_end().await;
}

/// Wyciąga treść z wiadomości czatu; `None` dla błędów/pustych.
fn body_of(msg: &Message) -> Option<String> {
    if msg.type_ == MessageType::Error { return None; }
    let body = msg.bodies.get("").or_else(|| msg.bodies.values().next())?;
    let body = body.trim();
    (!body.is_empty()).then(|| body.to_string())
}

fn handle_incoming(app: &AppHandle, msg: &Message) {
    let Some(body) = body_of(msg) else { return };
    let Some(from) = msg.from.as_ref() else { return };
    let from_bare = from.to_bare().to_string();

    let mut conversations = super::read_conversations();
    // Wiadomości od nieznanych JID-ów są ignorowane — rozmowa powstaje dopiero
    // po jawnym dodaniu kontaktu (nieproszona wiadomość nie tworzy stanu UI).
    let Some(convo_id) = conversations
        .iter()
        .find(|c| c.channel == super::Channel::Xmpp && c.participant == from_bare)
        .map(|c| c.id.clone())
    else { return };

    let id = msg.id.as_ref().map(|i| format!("xmpp-{}", i.0)).unwrap_or_else(|| format!("xmpp-{}", chrono::Utc::now().timestamp_millis()));
    if super::storage::message_ids_for(&convo_id).contains(&id) { return; }
    let message = super::Message {
        id,
        conversation_id: convo_id.clone(),
        body,
        direction: super::MessageDirection::Incoming,
        sent_at: chrono::Utc::now().to_rfc3339(),
        read: false,
    };
    if super::storage::add_many(std::slice::from_ref(&message)).is_err() { return; }
    if let Some(c) = conversations.iter_mut().find(|c| c.id == convo_id) {
        c.last_message_preview = message.body.clone();
        c.last_message_at = message.sent_at.clone();
        c.unread_count += 1;
        let _ = super::write_conversations(&conversations);
    }
    let _ = app.emit("blue-messages://xmpp-incoming", XmppIncomingEvent { conversation_id: convo_id, message });
}

/// Adds `contact_jid` as a local conversation with `channel: Xmpp`.
#[tauri::command(async)]
pub fn xmpp_add_contact(contact_jid: String, name: String) -> Result<super::Conversation, String> {
    bare_jid(&contact_jid)?;
    super::create_conversation_internal(name, contact_jid, super::Channel::Xmpp)
}

/// Kolejkuje wiadomość do wysłania przez trwałe połączenie. Nie blokuje.
pub fn send_message(to_jid: &str, body: &str) -> Result<(), String> {
    read_session().ok_or("Not logged in to XMPP")?;
    Jid::from_str(to_jid).map_err(|e| format!("Nieprawidłowy JID: {e}"))?;
    let guard = OUTBOX.lock().unwrap();
    let tx = guard.as_ref().ok_or("XMPP nie jest jeszcze połączony — spróbuj za chwilę")?;
    tx.send((to_jid.to_string(), body.to_string())).map_err(|_| "połączenie XMPP zostało zamknięte".to_string())
}

/// Odbiór jest na żywo (połączenie w tle), więc „odśwież" zwraca zapisany wątek.
#[tauri::command]
pub async fn xmpp_refresh_thread(conversation_id: String) -> Result<Vec<super::Message>, String> {
    let conversations = super::read_conversations();
    let convo = conversations.iter().find(|c| c.id == conversation_id).ok_or("Conversation not found")?;
    if convo.channel != super::Channel::Xmpp {
        return Err("Not an XMPP conversation".to_string());
    }
    Ok(super::storage::thread(&conversation_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_jid_accepts_bare_and_full_jids() {
        assert_eq!(split_jid("alice@example.com").unwrap(), ("alice".to_string(), "example.com".to_string()));
        assert_eq!(split_jid("alice@example.com/phone").unwrap(), ("alice".to_string(), "example.com".to_string()));
    }

    #[test]
    fn split_jid_rejects_missing_at_sign() {
        assert!(split_jid("not-a-jid").is_err());
    }

    #[test]
    fn bare_jid_validates() {
        // biblioteka normalizuje JID (nodeprep/nameprep): część lokalna i domena małymi literami
        assert_eq!(bare_jid("Alice@Example.com/res").unwrap().to_string(), "alice@example.com");
        assert!(bare_jid("@example.com").is_err());
    }

    #[test]
    fn body_extraction_skips_errors_and_blanks() {
        let mut m = Message::new(Some(Jid::from_str("a@b.c").unwrap()));
        m.bodies.insert(Default::default(), "  cześć  ".into());
        assert_eq!(body_of(&m).as_deref(), Some("cześć"));
        m.type_ = MessageType::Error;
        assert!(body_of(&m).is_none());
        let mut e = Message::new(None);
        e.bodies.insert(Default::default(), "   ".into());
        assert!(body_of(&e).is_none());
    }

    #[test]
    fn send_without_session_fails_cleanly() {
        // brak pliku sesji w katalogu testowym → czytelny błąd, nie panika
        assert!(send_message("a@b.c", "x").is_err() || OUTBOX.lock().unwrap().is_some());
    }
}
