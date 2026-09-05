use std::collections::HashMap;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, Proxy};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SERVICE: &str = "org.freedesktop.ModemManager1";
const MANAGER_PATH: &str = "/org/freedesktop/ModemManager1";

/// Finds the D-Bus object path of the first modem ModemManager currently
/// knows about. Most desktops/laptops with cellular hardware have
/// exactly one, so "first" is an acceptable simplification rather than
/// exposing modem selection as its own setting for a v1.
async fn first_modem_path(conn: &Connection) -> Result<OwnedObjectPath, String> {
    let proxy = Proxy::new(conn, SERVICE, MANAGER_PATH, "org.freedesktop.DBus.ObjectManager")
        .await
        .map_err(|e| format!("could not reach ModemManager over D-Bus: {e}"))?;
    let objects: HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> = proxy
        .call("GetManagedObjects", &())
        .await
        .map_err(|e| format!("ModemManager GetManagedObjects failed: {e}"))?;

    objects
        .into_iter()
        .find(|(_, ifaces)| ifaces.contains_key("org.freedesktop.ModemManager1.Modem"))
        .map(|(path, _)| path)
        .ok_or_else(|| "No modem found — is a SIM/modem attached and unlocked, and is ModemManager running?".to_string())
}

/// Sends `text` to `number` through the first available modem.
pub async fn send_sms(number: &str, text: &str) -> Result<(), String> {
    let conn = Connection::system()
        .await
        .map_err(|e| format!("could not connect to the system D-Bus: {e}"))?;
    let modem_path = first_modem_path(&conn).await?;

    let messaging = Proxy::new(&conn, SERVICE, modem_path.as_str(), "org.freedesktop.ModemManager1.Modem.Messaging")
        .await
        .map_err(|e| e.to_string())?;

    let mut props: HashMap<&str, Value> = HashMap::new();
    props.insert("number", Value::from(number));
    props.insert("text", Value::from(text));

    let sms_path: OwnedObjectPath = messaging
        .call("Create", &(props,))
        .await
        .map_err(|e| format!("ModemManager could not create the SMS (Create): {e}"))?;

    let sms = Proxy::new(&conn, SERVICE, sms_path.as_str(), "org.freedesktop.ModemManager1.Sms")
        .await
        .map_err(|e| e.to_string())?;
    sms.call_method("Send", &())
        .await
        .map_err(|e| format!("ModemManager could not send the SMS (Send): {e}"))?;
    Ok(())
}

/// Pulls current SMS object paths from the modem's `Messaging.Messages`
/// property and returns `(number, text)` for each one not already in
/// `existing_texts` (a crude de-dup by exact text match, since
/// ModemManager's SMS objects don't expose a stable id this project
/// already tracks anywhere — good enough for "don't re-import the same
/// message every refresh", not a guarantee against a genuine duplicate
/// message with identical text).
pub async fn poll_incoming(existing_texts: &[String]) -> Result<Vec<(String, String)>, String> {
    let conn = Connection::system()
        .await
        .map_err(|e| format!("could not connect to the system D-Bus: {e}"))?;
    let modem_path = first_modem_path(&conn).await?;

    let messaging = Proxy::new(&conn, SERVICE, modem_path.as_str(), "org.freedesktop.ModemManager1.Modem.Messaging")
        .await
        .map_err(|e| e.to_string())?;
    let message_paths: Vec<OwnedObjectPath> = messaging
        .get_property("Messages")
        .await
        .map_err(|e| format!("could not read Messaging.Messages: {e}"))?;

    let mut results = Vec::new();
    for path in message_paths {
        let sms = match Proxy::new(&conn, SERVICE, path.as_str(), "org.freedesktop.ModemManager1.Sms").await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let number: String = sms.get_property("Number").await.unwrap_or_default();
        let text: String = sms.get_property("Text").await.unwrap_or_default();
        if text.is_empty() || existing_texts.iter().any(|t| t == &text) {
            continue;
        }
        results.push((number, text));
    }
    Ok(results)
}

/// Adds `phone_number` as a local conversation with `channel: Sms`,
/// routed through a locally-attached modem via ModemManager (this
/// module's original path — see [`send_sms`]/[`poll_incoming`]).
#[tauri::command]
pub fn sms_add_contact(phone_number: String, name: String) -> Result<super::Conversation, String> {
    super::create_conversation_internal(name, phone_number, super::Channel::Sms)
}

/// Adds `phone_number` as a local conversation with `channel: Sms`,
/// routed through a paired phone via Blue Connect rather than a local
/// modem (see [`send_sms_via_phone`]/[`poll_incoming_via_phone`] and
/// this module's doc for what that protocol does and doesn't cover).
#[tauri::command]
pub fn sms_add_phone_contact(device_id: String, phone_number: String, name: String) -> Result<super::Conversation, String> {
    super::create_conversation_with_device(name, phone_number, super::Channel::Sms, Some(device_id))
}

/// Lists currently paired Blue Connect devices, for the frontend's
/// "send SMS through..." picker when creating a new phone-relayed SMS
/// conversation.
#[tauri::command]
pub fn sms_list_paired_phones() -> Vec<crate::blue_connect::PairedDeviceHandlePublic> {
    crate::blue_connect::list_paired_devices()
        .into_iter()
        .map(|d| crate::blue_connect::PairedDeviceHandlePublic { device_id: d.device_id, device_name: d.device_name })
        .collect()
}

/// Sends an SMS through a paired phone using a KDE-Connect-style
/// `kdeconnect.sms.request` packet over the mutual-TLS transport
/// `BlueConnect::open_authenticated_connection` already provides.
///
/// ## Protocol scope and real-world interop
/// The packet type and field names (`sendSms`, `phoneNumber`,
/// `messageBody`) intentionally match real KDE Connect's own SMS
/// plugin as closely as this project's simpler identity/pairing
/// handshake allows, since matching the wire format is what gives this
/// *any* chance of working against a real KDE Connect/GSConnect Android
/// app rather than only another instance of this project. That said,
/// real interop isn't guaranteed and hasn't been tested against an
/// actual phone in this change — real KDE Connect negotiates plugin
/// capabilities via `incomingCapabilities`/`outgoingCapabilities` lists
/// in its identity packet before either side will act on a plugin
/// packet at all, and this project's identity packet (see `mod.rs`)
/// doesn't include those lists. A real KDE Connect app may therefore
/// simply ignore this packet. Talking to Blue Connect running on
/// another device (once/if a mobile counterpart exists) would not have
/// this gap, since both sides would be this same implementation.
pub async fn send_sms_via_phone(device_id: &str, phone_number: &str, body: &str) -> Result<(), String> {
    let handle = crate::blue_connect::get_paired_device(device_id)
        .ok_or("That device is no longer paired — re-pair it in Blue Connect and try again")?;
    let mut stream = crate::blue_connect::open_authenticated_connection(&handle).await?;

    let packet = serde_json::json!({
        "type": "kdeconnect.sms.request",
        "body": { "sendSms": true, "phoneNumber": phone_number, "messageBody": body },
    });
    let mut payload = serde_json::to_vec(&packet).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    stream.write_all(&payload).await.map_err(|e| format!("failed to send SMS request to {}: {e}", handle.device_name))?;
    stream.flush().await.map_err(|e| e.to_string())
}

/// Requests the phone's SMS history/inbox and returns whatever it
/// replies with as `(address, body)` pairs — same shape
/// [`poll_incoming`] (the ModemManager path) already returns, so
/// `sms_refresh_thread` in `mod.rs` can treat both sources identically.
///
/// Sends `kdeconnect.sms.request` with `requestAllConversations: true`
/// and waits (bounded by `timeout_secs`) for a `kdeconnect.sms.messages`
/// response — again matching real KDE Connect's actual packet types/
/// field names for the same interop-attempt reasons described in
/// [`send_sms_via_phone`]'s doc, with the same caveat about capability
/// negotiation not being implemented here.
pub async fn poll_incoming_via_phone(device_id: &str, timeout_secs: u64) -> Result<Vec<(String, String)>, String> {
    let handle = crate::blue_connect::get_paired_device(device_id)
        .ok_or("That device is no longer paired — re-pair it in Blue Connect and try again")?;
    let mut stream = crate::blue_connect::open_authenticated_connection(&handle).await?;

    let request = serde_json::json!({
        "type": "kdeconnect.sms.request",
        "body": { "requestAllConversations": true },
    });
    let mut payload = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    stream.write_all(&payload).await.map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; 65536]; // a full SMS history reply can be sizeable
    let n = match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs.max(1)), stream.read(&mut buf)).await {
        Ok(Ok(0)) => return Err(format!("{} closed the connection without replying", handle.device_name)),
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(format!("connection error while waiting for SMS history: {e}")),
        Err(_) => return Err(format!("{} did not reply with SMS history in time", handle.device_name)),
    };

    let json: serde_json::Value = serde_json::from_slice(&buf[..n]).map_err(|e| format!("malformed response: {e}"))?;
    let messages = json.get("body").and_then(|b| b.get("messages")).and_then(|m| m.as_array()).cloned().unwrap_or_default();

    Ok(messages
        .into_iter()
        .filter_map(|m| {
            let body = m.get("body").and_then(|v| v.as_str())?.to_string();
            let address = m.get("address").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            Some((address, body))
        })
        .collect())
}



/// Whether ModemManager currently reports at least one modem — used by
/// the frontend to decide whether to offer "New SMS conversation" at
/// all, rather than letting someone create one that can never send.
#[tauri::command]
pub async fn sms_modem_available() -> bool {
    let Ok(conn) = Connection::system().await else { return false; };
    first_modem_path(&conn).await.is_ok()
}

/// Refreshes an SMS conversation's thread by polling for new incoming
/// messages — through a paired phone via Blue Connect if the
/// conversation has a `phone_device_id` set, otherwise through a
/// locally-attached modem via ModemManager (the original path). Single
/// command for both so the frontend doesn't need to know which
/// transport a given SMS conversation uses.
#[tauri::command]
pub async fn sms_refresh_thread(conversation_id: String) -> Result<Vec<super::Message>, String> {
    let conversations = super::read_conversations();
    let convo = conversations.iter().find(|c| c.id == conversation_id).ok_or("Conversation not found")?.clone();
    if convo.channel != super::Channel::Sms {
        return Err("Not an SMS conversation".to_string());
    }

    if let Some(device_id) = &convo.phone_device_id {
        return refresh_thread_via_phone(&conversation_id, device_id).await;
    }

    let existing = super::storage::thread(&conversation_id).into_iter().map(|m| m.body).collect::<Vec<_>>();
    let incoming = poll_incoming(&existing).await?;

    let mut new_messages = Vec::new();
    for (i, (_number, text)) in incoming.into_iter().enumerate() {
        new_messages.push(super::Message {
            id: format!("sms-{}-{i}", chrono::Utc::now().timestamp_millis()),
            conversation_id: conversation_id.clone(),
            body: text,
            direction: super::MessageDirection::Incoming,
            sent_at: chrono::Utc::now().to_rfc3339(),
            read: false,
        });
    }
    if !new_messages.is_empty() {
        super::storage::add_many(&new_messages)?;
        let mut conversations = conversations;
        if let (Some(newest), Some(c)) = (new_messages.last(), conversations.iter_mut().find(|c| c.id == conversation_id)) {
            c.last_message_preview = newest.body.clone();
            c.last_message_at = newest.sent_at.clone();
            let _ = super::write_conversations(&conversations);
        }
    }
    Ok(super::storage::thread(&conversation_id))
}

/// The phone-relayed half of [`sms_refresh_thread`] — see
/// [`send_sms_via_phone`]/[`poll_incoming_via_phone`] for the protocol
/// this uses.
async fn refresh_thread_via_phone(conversation_id: &str, device_id: &str) -> Result<Vec<super::Message>, String> {
    let existing = super::storage::thread(conversation_id).into_iter().map(|m| m.body).collect::<Vec<_>>();
    let incoming = poll_incoming_via_phone(device_id, 10).await?;

    let mut new_messages = Vec::new();
    for (i, (_address, text)) in incoming.into_iter().enumerate() {
        if existing.contains(&text) {
            continue; // same crude dedup-by-exact-text as the ModemManager path — see poll_incoming's doc
        }
        new_messages.push(super::Message {
            id: format!("sms-phone-{}-{i}", chrono::Utc::now().timestamp_millis()),
            conversation_id: conversation_id.to_string(),
            body: text,
            direction: super::MessageDirection::Incoming,
            sent_at: chrono::Utc::now().to_rfc3339(),
            read: false,
        });
    }
    if !new_messages.is_empty() {
        super::storage::add_many(&new_messages)?;
        let mut conversations = super::read_conversations();
        if let (Some(newest), Some(c)) = (new_messages.last(), conversations.iter_mut().find(|c| c.id == conversation_id)) {
            c.last_message_preview = newest.body.clone();
            c.last_message_at = newest.sent_at.clone();
            let _ = super::write_conversations(&conversations);
        }
    }
    Ok(super::storage::thread(conversation_id))
}
