use matrix_sdk::{
    authentication::matrix::MatrixSession as SdkSession,
    config::SyncSettings,
    room::MessagesOptions,
    ruma::{
        events::{
            room::message::{OriginalSyncRoomMessageEvent, RoomMessageEventContent},
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
        },
        OwnedRoomId, RoomId, UserId,
    },
    store::RoomLoadSettings,
    Client, Room, SessionMeta, SessionTokens,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as AsyncMutex;

fn session_path() -> PathBuf {
    super::messages_dir().join("matrix_session.json")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MatrixSession {
    pub homeserver: String, // np. "https://matrix.org"
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MatrixRoom {
    pub room_id: String,
    pub name: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct MatrixIncomingEvent {
    conversation_id: String,
    message: super::Message,
}

fn read_session() -> Option<MatrixSession> {
    fs::read_to_string(session_path()).ok().and_then(|s| serde_json::from_str(&s).ok())
}
fn write_session(session: &MatrixSession) -> Result<(), String> {
    fs::create_dir_all(super::messages_dir()).map_err(|e| e.to_string())?;
    fs::write(session_path(), serde_json::to_string_pretty(session).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub fn get_session() -> Option<MatrixSession> {
    read_session()
}

// ── klient współdzielony ─────────────────────────────────────────────
static CLIENT: AsyncMutex<Option<Client>> = AsyncMutex::const_new(None);
static SYNC_GENERATION: AtomicU64 = AtomicU64::new(0);

fn normalize_homeserver(hs: &str) -> Result<String, String> {
    let h = hs.trim().trim_end_matches('/');
    if h.is_empty() {
        return Err("Podaj adres serwera Matrix".into());
    }
    let with_scheme = if h.contains("://") { h.to_string() } else { format!("https://{h}") };
    if !(with_scheme.starts_with("https://") || with_scheme.starts_with("http://")) {
        return Err("Adres serwera musi zaczynać się od https://".into());
    }
    Ok(with_scheme)
}

async fn build_client(homeserver: &str) -> Result<Client, String> {
    Client::builder()
        .homeserver_url(homeserver)
        .build()
        .await
        .map_err(|e| format!("Nie można połączyć z serwerem Matrix: {e}"))
}

/// Klient odtworzony z zapisanej sesji (tokenu) — cache'owany na czas procesu.
async fn client() -> Result<Client, String> {
    let mut guard = CLIENT.lock().await;
    if let Some(c) = guard.as_ref() {
        return Ok(c.clone());
    }
    let s = read_session().ok_or("Not logged in to Matrix")?;
    let c = build_client(&s.homeserver).await?;
    let session = SdkSession {
        meta: SessionMeta {
            user_id: UserId::parse(&s.user_id).map_err(|e| format!("Zły user_id: {e}"))?,
            device_id: s.device_id.clone().into(),
        },
        tokens: SessionTokens { access_token: s.access_token.clone(), refresh_token: None },
    };
    c.matrix_auth()
        .restore_session(session, RoomLoadSettings::default())
        .await
        .map_err(|e| format!("Nie można odtworzyć sesji Matrix: {e}"))?;
    *guard = Some(c.clone());
    Ok(c)
}

#[tauri::command(async)]
pub fn matrix_has_session() -> bool {
    read_session().is_some()
}

#[tauri::command]
pub async fn matrix_logout() -> Result<(), String> {
    SYNC_GENERATION.fetch_add(1, Ordering::SeqCst); // zatrzymuje sync w tle
    if let Some(c) = CLIENT.lock().await.take() {
        let _ = c.logout().await; // unieważnia token na serwerze (best-effort)
    }
    let path = session_path();
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Logowanie hasłem (`m.login.password`) przez `matrix-sdk`.
#[tauri::command]
pub async fn matrix_login(app: AppHandle, homeserver: String, username: String, password: String) -> Result<MatrixSession, String> {
    let hs = normalize_homeserver(&homeserver)?;
    let c = build_client(&hs).await?;
    let resp = c
        .matrix_auth()
        .login_username(username.trim(), &password)
        .initial_device_display_name("Blue Messages")
        .await
        .map_err(|e| format!("Logowanie Matrix nie powiodło się: {e}"))?;
    let session = MatrixSession {
        homeserver: hs,
        user_id: resp.user_id.to_string(),
        access_token: resp.access_token,
        device_id: resp.device_id.to_string(),
    };
    write_session(&session)?;
    *CLIENT.lock().await = Some(c);
    start_background_sync(app);
    Ok(session)
}

// ── synchronizacja w tle (odbiór na żywo) ────────────────────────────

/// Treść tekstowa zdarzenia wiadomości (`None` dla nie-tekstowych).
fn text_of(ev: &OriginalSyncRoomMessageEvent) -> String {
    ev.content.body().to_string()
}

fn ms_to_rfc3339(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).map(|d| d.to_rfc3339()).unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
}

/// Dopisuje wiadomość do rozmowy powiązanej z pokojem (jeśli taka istnieje).
/// Zwraca zapisaną wiadomość, gdy była nowa.
fn store_incoming(room_id: &str, event_id: &str, sender: &str, own_id: &str, body: String, ts_ms: i64) -> Option<(String, super::Message)> {
    let mut conversations = super::read_conversations();
    let convo_id = conversations
        .iter()
        .find(|c| c.channel == super::Channel::Matrix && c.participant == room_id)
        .map(|c| c.id.clone())?;
    if super::storage::message_ids_for(&convo_id).contains(&event_id.to_string()) {
        return None;
    }
    let outgoing = sender == own_id;
    let message = super::Message {
        id: event_id.to_string(),
        conversation_id: convo_id.clone(),
        body,
        direction: if outgoing { super::MessageDirection::Outgoing } else { super::MessageDirection::Incoming },
        sent_at: ms_to_rfc3339(ts_ms),
        read: outgoing,
    };
    super::storage::add_many(std::slice::from_ref(&message)).ok()?;
    if let Some(c) = conversations.iter_mut().find(|c| c.id == convo_id) {
        c.last_message_preview = message.body.clone();
        c.last_message_at = message.sent_at.clone();
        if !outgoing {
            c.unread_count += 1;
        }
        let _ = super::write_conversations(&conversations);
    }
    Some((convo_id, message))
}

/// Startuje `sync` w tle (idempotentnie — poprzednia pętla wygasa).
pub fn start_background_sync(app: AppHandle) {
    if read_session().is_none() {
        return;
    }
    let generation = SYNC_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        let Ok(c) = client().await else { return };
        let own = c.user_id().map(|u| u.to_string()).unwrap_or_default();
        let app2 = app.clone();
        c.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, room: Room| {
            let app = app2.clone();
            let own = own.clone();
            async move {
                let ts: i64 = ev.origin_server_ts.0.into();
                if let Some((cid, msg)) = store_incoming(room.room_id().as_str(), ev.event_id.as_str(), ev.sender.as_str(), &own, text_of(&ev), ts) {
                    if msg.direction == super::MessageDirection::Incoming {
                        let _ = app.emit("blue-messages://matrix-incoming", MatrixIncomingEvent { conversation_id: cid, message: msg });
                    }
                }
            }
        });
        // `Client::sync` to wbudowana pętla długiego odpytywania (zarządza tokenem
        // `since`); wychodzi tylko błędem, więc ponawiamy z backoffem. Równolegle
        // czuwamy nad generacją, żeby wylogowanie/nowy login zatrzymały pętlę.
        let settings = SyncSettings::default().timeout(Duration::from_secs(30));
        let mut backoff = Duration::from_secs(2);
        loop {
            if SYNC_GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            let watch = async {
                while SYNC_GENERATION.load(Ordering::SeqCst) == generation {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            };
            tokio::select! {
                r = c.sync(settings.clone()) => {
                    if let Err(e) = r {
                        tracing::warn!("Matrix sync: {e}; ponawiam za {backoff:?}");
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(Duration::from_secs(300));
                    }
                }
                _ = watch => return,
            }
        }
    });
}

// ── komendy ──────────────────────────────────────────────────────────

/// Lista pokoi, do których należy konto.
#[tauri::command]
pub async fn matrix_list_rooms(app: AppHandle) -> Result<Vec<MatrixRoom>, String> {
    let c = client().await?;
    if c.joined_rooms().is_empty() {
        // Zanim zadziała sync w tle, wczytaj stan jednorazowo.
        c.sync_once(SyncSettings::default().timeout(Duration::from_secs(0)))
            .await
            .map_err(|e| format!("Synchronizacja nie powiodła się: {e}"))?;
    }
    start_background_sync(app);
    let mut rooms = Vec::new();
    for r in c.joined_rooms() {
        let name = r.display_name().await.map(|n| n.to_string()).unwrap_or_else(|_| r.room_id().to_string());
        rooms.push(MatrixRoom { room_id: r.room_id().to_string(), name });
    }
    rooms.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(rooms)
}

/// Rozmowa Blue Messages z `participant` = id pokoju Matrix.
#[tauri::command]
pub async fn matrix_import_room(room_id: String, name: String) -> Result<super::Conversation, String> {
    RoomId::parse(&room_id).map_err(|e| format!("Nieprawidłowe id pokoju: {e}"))?;
    super::create_conversation_internal(name, room_id, super::Channel::Matrix)
}

async fn room_for(c: &Client, room_id: &str) -> Result<Room, String> {
    let id: OwnedRoomId = RoomId::parse(room_id).map_err(|e| format!("Nieprawidłowe id pokoju: {e}"))?;
    if c.get_room(&id).is_none() {
        let _ = c.sync_once(SyncSettings::default().timeout(Duration::from_secs(0))).await;
    }
    c.get_room(&id).ok_or_else(|| "Nie znaleziono pokoju (czy konto nadal w nim jest?)".to_string())
}

/// Dociąga ostatnie wiadomości pokoju (`/messages`, 30 sztuk) i scala z magazynem.
#[tauri::command]
pub async fn matrix_refresh_thread(conversation_id: String) -> Result<Vec<super::Message>, String> {
    let conversations = super::read_conversations();
    let convo = conversations.iter().find(|c| c.id == conversation_id).ok_or("Conversation not found")?;
    if convo.channel != super::Channel::Matrix {
        return Err("Not a Matrix conversation".to_string());
    }
    let room_id = convo.participant.clone();
    let c = client().await?;
    let own = c.user_id().map(|u| u.to_string()).unwrap_or_default();
    let room = room_for(&c, &room_id).await?;

    let mut opts = MessagesOptions::backward();
    opts.limit = 30u32.into();
    let page = room.messages(opts).await.map_err(|e| format!("Nie udało się pobrać wiadomości: {e}"))?;

    // `chunk` idzie od najnowszych — odwracamy, żeby zapisywać chronologicznie.
    for ev in page.chunk.iter().rev() {
        let Ok(AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(m)))) = ev.raw().deserialize() else {
            continue; // członkostwa, reakcje, zdarzenia stanu — tylko wiadomości
        };
        let ts: i64 = m.origin_server_ts.0.into();
        let _ = store_incoming(&room_id, m.event_id.as_str(), m.sender.as_str(), &own, m.content.body().to_string(), ts);
    }
    Ok(super::storage::thread(&conversation_id))
}

/// Wysyła tekst do pokoju; zwraca id zdarzenia.
pub async fn send_to_room(_session: &MatrixSession, room_id: &str, body: &str) -> Result<String, String> {
    let c = client().await?;
    let room = room_for(&c, room_id).await?;
    let resp = room
        .send(RoomMessageEventContent::text_plain(body))
        .await
        .map_err(|e| format!("Nie udało się wysłać: {e}"))?;
    Ok(resp.event_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homeserver_is_normalized() {
        assert_eq!(normalize_homeserver("matrix.org").unwrap(), "https://matrix.org");
        assert_eq!(normalize_homeserver(" https://example.com/ ").unwrap(), "https://example.com");
        assert!(normalize_homeserver("  ").is_err());
        assert!(normalize_homeserver("ftp://x").is_err());
    }

    #[test]
    fn timestamps_convert() {
        assert!(ms_to_rfc3339(0).starts_with("1970-01-01"));
    }

    #[test]
    fn session_format_is_backward_compatible() {
        let s: MatrixSession = serde_json::from_str(
            r#"{"homeserver":"https://matrix.org","user_id":"@a:matrix.org","access_token":"t","device_id":"D"}"#,
        )
        .unwrap();
        assert_eq!(s.user_id, "@a:matrix.org");
    }
}
