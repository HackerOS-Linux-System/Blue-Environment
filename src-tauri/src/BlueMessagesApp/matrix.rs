use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn session_path() -> PathBuf {
    super::messages_dir().join("matrix_session.json")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MatrixSession {
    pub homeserver: String, // e.g. "https://matrix.org", no trailing slash
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MatrixRoom {
    pub room_id: String,
    pub name: String,
}

fn read_session() -> Option<MatrixSession> {
    fs::read_to_string(session_path()).ok().and_then(|s| serde_json::from_str(&s).ok())
}
fn write_session(session: &MatrixSession) -> Result<(), String> {
    fs::create_dir_all(super::messages_dir()).map_err(|e| e.to_string())?;
    fs::write(session_path(), serde_json::to_string_pretty(session).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn matrix_has_session() -> bool {
    read_session().is_some()
}

#[tauri::command]
pub fn matrix_logout() -> Result<(), String> {
    let path = session_path();
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `POST /_matrix/client/v3/login` with `m.login.password` — the
/// simplest of the several login flows the spec defines (SSO and
/// token-based login exist too; password is the one every homeserver
/// implementation supports without extra setup, so it's the one this
/// starts with).
#[tauri::command]
pub async fn matrix_login(homeserver: String, username: String, password: String) -> Result<MatrixSession, String> {
    let homeserver = homeserver.trim_end_matches('/').to_string();
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "type": "m.login.password",
        "identifier": { "type": "m.id.user", "user": username },
        "password": password,
        "initial_device_display_name": "Blue Messages",
    });

    let resp = client
        .post(format!("{homeserver}/_matrix/client/v3/login"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Could not reach homeserver: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Login failed ({status}): {text}"));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let session = MatrixSession {
        homeserver,
        user_id: json.get("user_id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        access_token: json.get("access_token").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        device_id: json.get("device_id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
    };
    if session.access_token.is_empty() {
        return Err("Homeserver response had no access_token".to_string());
    }
    write_session(&session)?;
    Ok(session)
}

/// `GET /_matrix/client/v3/joined_rooms`, then one
/// `GET /rooms/{id}/state/m.room.name` per room for a display name
/// (falls back to the raw room id if the room has no name state event
/// set, e.g. an un-named direct-message room).
#[tauri::command]
pub async fn matrix_list_rooms() -> Result<Vec<MatrixRoom>, String> {
    let session = read_session().ok_or("Not logged in to Matrix")?;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/_matrix/client/v3/joined_rooms", session.homeserver))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to list rooms: {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let room_ids: Vec<String> = json
        .get("joined_rooms")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut rooms = Vec::new();
    for room_id in room_ids {
        let name = fetch_room_name(&client, &session, &room_id).await.unwrap_or_else(|| room_id.clone());
        rooms.push(MatrixRoom { room_id, name });
    }
    Ok(rooms)
}

async fn fetch_room_name(client: &reqwest::Client, session: &MatrixSession, room_id: &str) -> Option<String> {
    let resp = client
        .get(format!(
            "{}/_matrix/client/v3/rooms/{}/state/m.room.name",
            session.homeserver,
            urlencoding_room_id(room_id)
        ))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None; // no name state event set — not an error, just unnamed
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("name").and_then(|v| v.as_str()).map(String::from)
}

// Matrix room ids (`!opaque:server`) contain characters (`!`, `:`)
// that need percent-encoding in a URL path segment — hand-rolled here
// rather than pulling in the `urlencoding` crate a second time (it's
// already a dependency elsewhere in this crate, but importing it into
// this module for two characters isn't worth the extra `use`).
fn urlencoding_room_id(room_id: &str) -> String {
    room_id.replace('!', "%21").replace(':', "%3A")
}

/// Adds `room_id` as a local [`super::Conversation`] with
/// `channel: Matrix` and `participant: room_id` — the join between
/// "a Matrix room" and "a Blue Messages conversation" is exactly that
/// one field, deliberately: no separate Matrix-specific conversation
/// table, so every other command in `mod.rs`
/// (`messages_delete_conversation`, `messages_set_pinned`, ...) already
/// works on an imported Matrix conversation for free.
#[tauri::command]
pub async fn matrix_import_room(room_id: String, name: String) -> Result<super::Conversation, String> {
    let convo = super::create_conversation_internal(name, room_id, super::Channel::Matrix)?;
    Ok(convo)
}

/// Pulls the most recent messages for a Matrix-backed conversation via
/// `GET /rooms/{id}/messages?dir=b&limit=30` and merges any not
/// already in local storage — see module doc's "What's not" section
/// for why this is pull-based rather than a live stream.
#[tauri::command]
pub async fn matrix_refresh_thread(conversation_id: String) -> Result<Vec<super::Message>, String> {
    let session = read_session().ok_or("Not logged in to Matrix")?;
    let conversations = super::read_conversations();
    let convo = conversations
        .iter()
        .find(|c| c.id == conversation_id)
        .ok_or("Conversation not found")?;
    if convo.channel != super::Channel::Matrix {
        return Err("Not a Matrix conversation".to_string());
    }
    let room_id = convo.participant.clone();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "{}/_matrix/client/v3/rooms/{}/messages",
            session.homeserver,
            urlencoding_room_id(&room_id)
        ))
        .query(&[("dir", "b"), ("limit", "30")])
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch messages: {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let events = json.get("chunk").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let existing_ids = super::storage::message_ids_for(&conversation_id);
    let mut new_messages = Vec::new();

    for event in events.iter().rev() {
        let Some(event_type) = event.get("type").and_then(|v| v.as_str()) else { continue };
        if event_type != "m.room.message" {
            continue; // skip membership/state/reaction events — text messages only for now
        }
        let event_id = event.get("event_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        if event_id.is_empty() || existing_ids.contains(&event_id) {
            continue;
        }
        let body = event
            .get("content")
            .and_then(|c| c.get("body"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let sender = event.get("sender").and_then(|v| v.as_str()).unwrap_or_default();
        let origin_ms = event.get("origin_server_ts").and_then(|v| v.as_i64()).unwrap_or(0);
        let sent_at = chrono::DateTime::from_timestamp_millis(origin_ms)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        new_messages.push(super::Message {
            id: event_id,
            conversation_id: conversation_id.clone(),
            body,
            direction: if sender == session.user_id { super::MessageDirection::Outgoing } else { super::MessageDirection::Incoming },
            sent_at,
            read: false,
        });
    }

    super::storage::add_many(&new_messages)?;

    // Refresh the conversation's denormalized preview too — matches
    // `messages_send`'s own convention of keeping it in sync on
    // anything that adds a message, not just local sends.
    if let Some(newest) = new_messages.last() {
        let mut conversations = conversations;
        if let Some(c) = conversations.iter_mut().find(|c| c.id == conversation_id) {
            c.last_message_preview = newest.body.clone();
            c.last_message_at = newest.sent_at.clone();
        }
        let _ = super::write_conversations(&conversations);
    }

    Ok(super::storage::thread(&conversation_id))
}

/// `PUT /rooms/{id}/send/m.room.message/{txnId}` — the txn id just
/// needs to be unique per-sender for idempotency (the spec's own
/// retry-safety mechanism: resending the same txn id is a no-op),
/// current-timestamp-plus-random is enough here, no persistent
/// counter needed.
pub async fn send_to_room(session: &MatrixSession, room_id: &str, body: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let txn_id = format!("bluemsg-{}-{}", chrono::Utc::now().timestamp_millis(), body.len());
    let resp = client
        .put(format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            session.homeserver,
            urlencoding_room_id(room_id),
            txn_id
        ))
        .bearer_auth(&session.access_token)
        .json(&serde_json::json!({ "msgtype": "m.text", "body": body }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to send: {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(json.get("event_id").and_then(|v| v.as_str()).unwrap_or_default().to_string())
}

pub fn get_session() -> Option<MatrixSession> {
    read_session()
}
