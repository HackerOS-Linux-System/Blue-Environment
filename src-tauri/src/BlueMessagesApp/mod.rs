pub mod matrix;
pub mod sms;
mod secretstore;
pub mod storage;
mod xml_stream;
mod scram;
pub mod xmpp;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub fn messages_dir() -> PathBuf {
    // Overridable for tests (see storage.rs's `isolated_test_env`) —
    // same pattern `themes.rs`'s `BLUE_THEMES_DIR` already uses for the
    // identical reason: tests can't share `~/.config`, and a real
    // system path isn't guaranteed writable/present in every
    // environment this crate's test suite runs in.
    if let Ok(dir) = std::env::var("BLUE_MESSAGES_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-messages")
}
fn conversations_path() -> PathBuf { messages_dir().join("conversations.json") }

/// Where a conversation's messages come from/go to.
///
/// - `Local`: this file's own storage only — a note-to-self thread, a
///   draft, ... Fully implemented.
/// - `Matrix`: real — see `matrix.rs`. Needs a homeserver + account.
/// - `Xmpp`: real, best-effort — see `xmpp.rs`'s module doc. Real
///   STARTTLS + SCRAM-SHA-256/SHA-1 (falling back to SASL PLAIN) +
///   bind, with a persistent, auto-reconnecting background connection
///   for live receiving (pushed to the frontend via a Tauri event) —
///   not just a pull-on-refresh design. Sending still opens its own
///   short-lived connection per message rather than sharing the
///   persistent one; see `xmpp.rs`'s doc for why.
/// - `Sms`: real for the ModemManager path, **not implemented** for the
///   phone-pairing path — see `sms.rs`'s module doc. Sending/receiving
///   through a USB/serial AT-command modem via ModemManager's D-Bus API
///   works when such a modem exists; there is still no code here that
///   talks to a paired Android phone (that would need something like
///   KDE Connect's own protocol — see `BlueConnect/mod.rs` — layered on
///   top, which is a separate, larger addition).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Local,
    Sms,
    Xmpp,
    Matrix,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    /// Free-form; a phone number, a JID, a Matrix room id — meaning
    /// depends on `channel`. For `Channel::Local` this is just a
    /// display label the person chose.
    pub participant: String,
    pub channel: Channel,
    pub created_at: String,
    /// Denormalized onto the conversation (rather than computed from
    /// message storage on every load) purely so the conversation list
    /// can render instantly without a thread query per row — kept in
    /// sync by [`messages_send`]/`matrix::matrix_refresh_thread`.
    pub last_message_preview: String,
    pub last_message_at: String,
    pub unread_count: u32,
    pub pinned: bool,
    /// Only set for `channel: Sms` conversations that relay through a
    /// paired phone via Blue Connect (see `sms.rs`'s
    /// `send_sms_via_phone`/`poll_incoming_via_phone`) rather than a
    /// locally-attached modem. `None` means "use ModemManager" — the
    /// original, still-supported SMS path — so existing SMS
    /// conversations created before this field existed keep working
    /// exactly as before without any migration needed.
    #[serde(default)]
    pub phone_device_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageDirection {
    Outgoing,
    Incoming,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub body: String,
    pub direction: MessageDirection,
    pub sent_at: String,
    /// `false` until the conversation has been opened/scrolled past
    /// this message — drives `Conversation.unread_count`.
    pub read: bool,
}

fn read_conversations() -> Vec<Conversation> {
    fs::read_to_string(conversations_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_conversations(items: &[Conversation]) -> Result<(), String> {
    fs::create_dir_all(messages_dir()).map_err(|e| e.to_string())?;
    fs::write(conversations_path(), serde_json::to_string_pretty(items).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Seeds one welcome conversation on first run — same "don't hand back
/// a completely empty app on first launch" convention as
/// `BlueTasksApp::ensure_default_list`.
fn ensure_default_conversation(items: &mut Vec<Conversation>) -> Vec<Conversation> {
    if items.is_empty() {
        let now = chrono::Utc::now().to_rfc3339();
        items.push(Conversation {
            id: "welcome".to_string(),
            title: "Blue Messages".to_string(),
            participant: "Blue Environment".to_string(),
            channel: Channel::Local,
            created_at: now.clone(),
            last_message_preview: "Welcome to Blue Messages — start a new conversation to begin.".to_string(),
            last_message_at: now.clone(),
            unread_count: 0,
            pinned: true,
            phone_device_id: None,
        });
        let _ = write_conversations(items);
        let _ = storage::add(&Message {
            id: "welcome-1".to_string(),
            conversation_id: "welcome".to_string(),
            body: "Welcome to Blue Messages — start a new conversation to begin.".to_string(),
            direction: MessageDirection::Incoming,
            sent_at: now,
            read: true,
        });
    }
    items.clone()
}

#[tauri::command]
pub fn messages_load_conversations() -> Vec<Conversation> {
    let mut items = read_conversations();
    let mut items = ensure_default_conversation(&mut items);
    // Pinned first, then most recently active — matches how the
    // frontend's conversation list is meant to render, kept here
    // rather than duplicated client-side so every future consumer of
    // this command gets the same ordering for free.
    items.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.last_message_at.cmp(&a.last_message_at)));
    items
}

/// Shared by the `messages_create_conversation` command and
/// `matrix::matrix_import_room` (a Matrix room import is, from local
/// storage's point of view, just creating a conversation with
/// `channel: Matrix` and `participant: <room id>` — see that function's
/// own doc). `phone_device_id` is `None` for every caller except
/// `sms::sms_add_phone_contact`.
fn create_conversation_internal(title: String, participant: String, channel: Channel) -> Result<Conversation, String> {
    create_conversation_with_device(title, participant, channel, None)
}

fn create_conversation_with_device(title: String, participant: String, channel: Channel, phone_device_id: Option<String>) -> Result<Conversation, String> {
    let mut items = read_conversations();
    let now = chrono::Utc::now().to_rfc3339();
    let convo = Conversation {
        id: format!("{}-{}", chrono::Utc::now().timestamp_millis(), &title.len()),
        title,
        participant,
        channel,
        created_at: now.clone(),
        last_message_preview: String::new(),
        last_message_at: now,
        unread_count: 0,
        pinned: false,
        phone_device_id,
    };
    items.push(convo.clone());
    write_conversations(&items)?;
    Ok(convo)
}

#[tauri::command]
pub fn messages_create_conversation(title: String, participant: String, channel: Channel) -> Result<Conversation, String> {
    create_conversation_internal(title, participant, channel)
}

#[tauri::command]
pub fn messages_delete_conversation(id: String) -> Result<(), String> {
    let mut items = read_conversations();
    items.retain(|c| c.id != id);
    write_conversations(&items)?;
    storage::delete_for_conversation(&id)
}

#[tauri::command]
pub fn messages_set_pinned(id: String, pinned: bool) -> Result<(), String> {
    let mut items = read_conversations();
    if let Some(c) = items.iter_mut().find(|c| c.id == id) {
        c.pinned = pinned;
    }
    write_conversations(&items)
}

#[tauri::command]
pub fn messages_load_thread(conversation_id: String) -> Vec<Message> {
    storage::thread(&conversation_id)
}

/// Sends `body` as an outgoing message in `conversation_id`. For a
/// `Channel::Matrix` conversation this also actually delivers it (see
/// `matrix::send_to_room`) — a failed send is reported as an error
/// rather than silently downgrading to "saved locally only", since a
/// person watching the compose box has no other way to know their
/// message didn't really go anywhere.
#[tauri::command]
pub async fn messages_send(conversation_id: String, body: String) -> Result<Message, String> {
    let conversations = read_conversations();
    let convo = conversations.iter().find(|c| c.id == conversation_id).cloned();

    if let Some(c) = &convo {
        match c.channel {
            Channel::Matrix => {
                let session = matrix::get_session().ok_or("Not logged in to Matrix")?;
                matrix::send_to_room(&session, &c.participant, &body).await?;
            }
            Channel::Xmpp => {
                let participant = c.participant.clone();
                let body_for_send = body.clone();
                // xmpp::send_message is blocking (raw std::net I/O, see
                // its module doc) — run it on a blocking thread so it
                // doesn't stall this async command's executor while it
                // waits on the network.
                tokio::task::spawn_blocking(move || xmpp::send_message(&participant, &body_for_send))
                    .await
                    .map_err(|e| format!("XMPP send task panicked: {e}"))??;
            }
            Channel::Sms => {
                match &c.phone_device_id {
                    Some(device_id) => sms::send_sms_via_phone(device_id, &c.participant, &body).await?,
                    None => sms::send_sms(&c.participant, &body).await?,
                }
            }
            Channel::Local => {}
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    let message = Message {
        id: format!("{}-{}", chrono::Utc::now().timestamp_millis(), body.len()),
        conversation_id: conversation_id.clone(),
        body: body.clone(),
        direction: MessageDirection::Outgoing,
        sent_at: now.clone(),
        read: true, // the sender has obviously "read" their own outgoing message
    };
    storage::add(&message)?;

    let mut conversations = conversations;
    if let Some(c) = conversations.iter_mut().find(|c| c.id == conversation_id) {
        c.last_message_preview = body;
        c.last_message_at = now;
    }
    write_conversations(&conversations)?;

    Ok(message)
}

/// Marks every message in `conversation_id` read and zeroes its unread
/// count — called when the frontend opens a conversation's thread.
#[tauri::command]
pub fn messages_mark_read(conversation_id: String) -> Result<(), String> {
    storage::mark_read(&conversation_id)?;

    let mut conversations = read_conversations();
    if let Some(c) = conversations.iter_mut().find(|c| c.id == conversation_id) {
        c.unread_count = 0;
    }
    write_conversations(&conversations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn isolated_test_env() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("blue-messages-mod-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("BLUE_MESSAGES_DIR", &dir);
        dir
    }

    #[test]
    fn first_load_seeds_a_welcome_conversation() {
        let dir = isolated_test_env();
        let items = messages_load_conversations();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "welcome");
        assert!(items[0].pinned);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn create_and_delete_conversation_round_trip() {
        let dir = isolated_test_env();
        let convo = create_conversation_internal("Test".to_string(), "someone".to_string(), Channel::Local).unwrap();
        assert!(read_conversations().iter().any(|c| c.id == convo.id));

        messages_delete_conversation(convo.id.clone()).unwrap();
        assert!(!read_conversations().iter().any(|c| c.id == convo.id));

        let _ = fs::remove_dir_all(dir);
    }
}
