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
#[tauri::command(async)]
pub fn sms_add_contact(phone_number: String, name: String) -> Result<super::Conversation, String> {
    super::create_conversation_internal(name, phone_number, super::Channel::Sms)
}

/// Adds `phone_number` as a local conversation with `channel: Sms`,
/// routed through a paired phone via Blue Connect rather than a local
/// modem (see [`send_sms_via_phone`]/[`poll_incoming_via_phone`] and
/// this module's doc for what that protocol does and doesn't cover).
#[tauri::command(async)]
pub fn sms_add_phone_contact(device_id: String, phone_number: String, name: String) -> Result<super::Conversation, String> {
    super::create_conversation_with_device(name, phone_number, super::Channel::Sms, Some(device_id))
}

/// Lists currently paired Blue Connect devices, for the frontend's
/// "send SMS through..." picker when creating a new phone-relayed SMS
/// conversation.
#[tauri::command(async)]
pub fn sms_list_paired_phones() -> Vec<crate::blue_connect::PairedDeviceHandlePublic> {
    crate::blue_connect::list_paired_devices()
        .into_iter()
        .map(|d| crate::blue_connect::PairedDeviceHandlePublic { device_id: d.device_id, device_name: d.device_name })
        .collect()
}

// ───────────────────────── phone relay (Blue Connect) ─────────────────────────
//
// Wire format (both directions, one JSON object + `\n` per packet, same
// framing as `BlueConnect/plugin_proto.rs`):
//
//   desktop → phone  `kdeconnect.sms.request`  {sendSms:true, phoneNumber, messageBody}
//   desktop → phone  `kdeconnect.sms.request`  {requestAllConversations:true}
//   phone → desktop  `kdeconnect.sms.messages` {messages:[{address, body, date, type}]}
//
// `type` is Android's `Telephony.Sms.TYPE` (1 = inbox, 2 = sent). The packet
// names match KDE Connect's SMS plugin; the phone side that implements them
// is `SmsPlugin.kt` in Blue Connect for Android (it also advertises the
// capability in its identity packet). A stock KDE Connect app may ignore the
// packets — that is a KDE Connect limitation, not a gap of this transport.

/// Longest message body accepted for a phone relay (the phone splits it into
/// multipart SMS itself).
pub const MAX_PHONE_SMS_CHARS: usize = 4000;
/// Upper bound for one framed reply line from the phone.
const MAX_REPLY_BYTES: usize = 1024 * 1024;

const SMS_TYPE_INBOX: i32 = 1;
const SMS_TYPE_SENT: i32 = 2;

/// One row of the phone's SMS store as reported by `kdeconnect.sms.messages`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneSms {
    pub address: String,
    pub body: String,
    /// Milliseconds since the Unix epoch (0 when the phone did not send one).
    pub date_ms: i64,
    /// Android `Telephony.Sms.TYPE`.
    pub kind: i32,
}

/// A conversation the phone already has, summarised for the "start from the
/// phone's history" picker in Blue Messages.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PhoneThread {
    pub address: String,
    pub last_body: String,
    pub last_date_ms: i64,
    pub count: u32,
}

/// Same sanity rule as the Android plugin: digits plus the usual separators.
pub fn is_plausible_number(n: &str) -> bool {
    let n = n.trim();
    (3..=32).contains(&n.len())
        && n.chars().all(|c| c.is_ascii_digit() || "+-() *#".contains(c))
        && n.chars().filter(|c| c.is_ascii_digit()).count() >= 3
}

/// Digits only (a leading `+` is dropped too — `+48 600 100 200` and
/// `600100200` must compare equal).
pub fn normalize_number(n: &str) -> String {
    n.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Whether two phone numbers denote the same subscriber. Country prefixes and
/// trunk zeros differ between how a person types a number and how the phone
/// stores it, so numbers with at least 7 digits are compared by their last 9
/// digits; shorter ones (service numbers such as `1234`) must match exactly.
pub fn numbers_match(a: &str, b: &str) -> bool {
    let (a, b) = (normalize_number(a), normalize_number(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a.len() < 7 || b.len() < 7 {
        return a == b;
    }
    let tail = |s: &str| s[s.len().saturating_sub(9)..].to_string();
    tail(&a) == tail(&b)
}

/// Deterministic message id, so the same phone message is never imported
/// twice no matter how often the thread is refreshed.
pub fn stable_message_id(device_id: &str, m: &PhoneSms) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(device_id.as_bytes());
    h.update([0]);
    h.update(normalize_number(&m.address).as_bytes());
    h.update([0]);
    h.update(m.date_ms.to_le_bytes());
    h.update([0]);
    h.update(m.kind.to_le_bytes());
    h.update([0]);
    h.update(m.body.as_bytes());
    let digest = h.finalize();
    let hex: String = digest.iter().take(10).map(|b| format!("{b:02x}")).collect();
    format!("sms-phone-{hex}")
}

/// Parses the body of a `kdeconnect.sms.messages` packet. Rows without a body
/// are dropped; the result is sorted oldest → newest (the phone sends newest
/// first).
pub fn parse_phone_messages(packet: &serde_json::Value) -> Vec<PhoneSms> {
    let rows = packet
        .get("body")
        .and_then(|b| b.get("messages"))
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<PhoneSms> = rows
        .into_iter()
        .filter_map(|m| {
            let body = m.get("body").and_then(|v| v.as_str())?.to_string();
            if body.is_empty() {
                return None;
            }
            Some(PhoneSms {
                address: m.get("address").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                body,
                date_ms: m.get("date").and_then(|v| v.as_i64()).unwrap_or(0),
                kind: m.get("type").and_then(|v| v.as_i64()).unwrap_or(SMS_TYPE_INBOX as i64) as i32,
            })
        })
        .collect();
    out.sort_by_key(|m| m.date_ms);
    out
}

/// Groups messages by subscriber, newest thread first.
pub fn group_threads(messages: &[PhoneSms]) -> Vec<PhoneThread> {
    let mut threads: Vec<PhoneThread> = Vec::new();
    for m in messages {
        if m.address.trim().is_empty() {
            continue;
        }
        match threads.iter_mut().find(|t| numbers_match(&t.address, &m.address)) {
            Some(t) => {
                t.count += 1;
                if m.date_ms >= t.last_date_ms {
                    t.last_date_ms = m.date_ms;
                    t.last_body = m.body.clone();
                }
            }
            None => threads.push(PhoneThread {
                address: m.address.clone(),
                last_body: m.body.clone(),
                last_date_ms: m.date_ms,
                count: 1,
            }),
        }
    }
    threads.sort_by(|a, b| b.last_date_ms.cmp(&a.last_date_ms));
    threads
}

/// Reads `\n`-framed JSON packets from `stream` until one of type `want`
/// arrives. Other packets (pings, battery, …) that happen to be in flight are
/// skipped instead of being mistaken for the reply, and a reply that arrives
/// in several TLS records is reassembled.
async fn read_packet_of_type<S>(stream: &mut S, want: &str, timeout_secs: u64) -> Result<serde_json::Value, String>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let fut = async {
        let mut pending: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = pending.drain(..=pos).collect();
                let line = &line[..line.len() - 1];
                if line.is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) {
                    if v.get("type").and_then(|t| t.as_str()) == Some(want) {
                        return Ok(v);
                    }
                }
            }
            if pending.len() > MAX_REPLY_BYTES {
                return Err("the phone's reply is too large".to_string());
            }
            match stream.read(&mut chunk).await {
                Ok(0) => return Err("the phone closed the connection without replying".to_string()),
                Ok(n) => pending.extend_from_slice(&chunk[..n]),
                Err(e) => return Err(format!("connection error while waiting for the reply: {e}")),
            }
        }
    };
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs.max(1)), fut).await {
        Ok(r) => r,
        Err(_) => Err("the phone did not reply in time".to_string()),
    }
}

/// Sends an SMS through a paired phone (`kdeconnect.sms.request`) over the
/// mutual-TLS transport `BlueConnect::open_authenticated_connection` provides.
pub async fn send_sms_via_phone(device_id: &str, phone_number: &str, body: &str) -> Result<(), String> {
    if !is_plausible_number(phone_number) {
        return Err(format!("\"{phone_number}\" is not a valid phone number"));
    }
    if body.trim().is_empty() {
        return Err("The message is empty".to_string());
    }
    if body.chars().count() > MAX_PHONE_SMS_CHARS {
        return Err(format!("The message is longer than {MAX_PHONE_SMS_CHARS} characters"));
    }
    let handle = crate::blue_connect::get_paired_device(device_id)
        .ok_or("That device is no longer paired — re-pair it in Blue Connect and try again")?;
    let mut stream = crate::blue_connect::open_authenticated_connection(&handle).await?;

    let packet = serde_json::json!({
        "type": "kdeconnect.sms.request",
        "body": { "sendSms": true, "phoneNumber": phone_number.trim(), "messageBody": body },
    });
    let mut payload = serde_json::to_vec(&packet).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    stream.write_all(&payload).await.map_err(|e| format!("failed to send SMS request to {}: {e}", handle.device_name))?;
    stream.flush().await.map_err(|e| e.to_string())
}

/// Asks the phone for its SMS history and returns every row it sent.
pub async fn fetch_phone_messages(device_id: &str, timeout_secs: u64) -> Result<Vec<PhoneSms>, String> {
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

    let reply = read_packet_of_type(&mut stream, "kdeconnect.sms.messages", timeout_secs)
        .await
        .map_err(|e| format!("{}: {e}", handle.device_name))?;
    Ok(parse_phone_messages(&reply))
}

/// Kept for callers that only want `(address, body)` pairs (the shape
/// [`poll_incoming`] returns for the ModemManager path).
pub async fn poll_incoming_via_phone(device_id: &str, timeout_secs: u64) -> Result<Vec<(String, String)>, String> {
    Ok(fetch_phone_messages(device_id, timeout_secs)
        .await?
        .into_iter()
        .map(|m| (m.address, m.body))
        .collect())
}

/// Lists the conversations the paired phone already has, so a new Blue
/// Messages SMS conversation can be started from the phone's own history.
#[tauri::command]
pub async fn sms_list_phone_threads(device_id: String) -> Result<Vec<PhoneThread>, String> {
    let messages = fetch_phone_messages(&device_id, 10).await?;
    Ok(group_threads(&messages))
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

/// The phone-relayed half of [`sms_refresh_thread`]: imports only the rows
/// that belong to this conversation's number (inbox → incoming, sent items
/// typed on the phone → outgoing), each exactly once.
async fn refresh_thread_via_phone(conversation_id: &str, device_id: &str) -> Result<Vec<super::Message>, String> {
    let conversations = super::read_conversations();
    let participant = conversations
        .iter()
        .find(|c| c.id == conversation_id)
        .map(|c| c.participant.clone())
        .ok_or("Conversation not found")?;

    let existing = super::storage::thread(conversation_id);
    let known_ids: std::collections::HashSet<String> = existing.iter().map(|m| m.id.clone()).collect();
    let known_outgoing: std::collections::HashSet<String> = existing
        .iter()
        .filter(|m| m.direction == super::MessageDirection::Outgoing)
        .map(|m| m.body.clone())
        .collect();

    let rows = fetch_phone_messages(device_id, 10).await?;
    let new_messages = select_new_messages(conversation_id, device_id, &participant, &rows, &known_ids, &known_outgoing);

    if !new_messages.is_empty() {
        super::storage::add_many(&new_messages)?;
        let incoming = new_messages.iter().filter(|m| m.direction == super::MessageDirection::Incoming).count() as u32;
        let mut conversations = conversations;
        if let (Some(newest), Some(c)) = (new_messages.last(), conversations.iter_mut().find(|c| c.id == conversation_id)) {
            c.last_message_preview = newest.body.clone();
            c.last_message_at = newest.sent_at.clone();
            c.unread_count = c.unread_count.saturating_add(incoming);
            let _ = super::write_conversations(&conversations);
        }
    }
    Ok(super::storage::thread(conversation_id))
}

/// Pure selection step of [`refresh_thread_via_phone`] (unit-tested below).
fn select_new_messages(
    conversation_id: &str,
    device_id: &str,
    participant: &str,
    rows: &[PhoneSms],
    known_ids: &std::collections::HashSet<String>,
    known_outgoing_bodies: &std::collections::HashSet<String>,
) -> Vec<super::Message> {
    let mut out = Vec::new();
    for m in rows {
        if !numbers_match(&m.address, participant) {
            continue; // another conversation's message
        }
        let direction = match m.kind {
            SMS_TYPE_INBOX => super::MessageDirection::Incoming,
            // Sent from this computer: already stored when it was sent.
            SMS_TYPE_SENT if known_outgoing_bodies.contains(&m.body) => continue,
            SMS_TYPE_SENT => super::MessageDirection::Outgoing,
            _ => continue, // drafts, outbox, failed …
        };
        let id = stable_message_id(device_id, m);
        if known_ids.contains(&id) || out.iter().any(|x: &super::Message| x.id == id) {
            continue;
        }
        let sent_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(m.date_ms)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339();
        out.push(super::Message {
            id,
            conversation_id: conversation_id.to_string(),
            body: m.body.clone(),
            read: direction == super::MessageDirection::Outgoing,
            direction,
            sent_at,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn row(address: &str, body: &str, date_ms: i64, kind: i32) -> PhoneSms {
        PhoneSms { address: address.into(), body: body.into(), date_ms, kind }
    }

    #[test]
    fn numbers_match_ignores_country_prefix_and_formatting() {
        assert!(numbers_match("+48 600-100-200", "600100200"));
        assert!(numbers_match("0048600100200", "600 100 200"));
        assert!(!numbers_match("600100200", "600100201"));
        assert!(!numbers_match("", "600100200"));
    }

    #[test]
    fn short_service_numbers_must_match_exactly() {
        assert!(numbers_match("1234", "1234"));
        assert!(!numbers_match("1234", "01234"));
    }

    #[test]
    fn plausible_number_rules() {
        assert!(is_plausible_number("+48 600 100 200"));
        assert!(!is_plausible_number("ab"));
        assert!(!is_plausible_number("600; rm -rf"));
    }

    #[test]
    fn parse_sorts_oldest_first_and_drops_empty_bodies() {
        let v = serde_json::json!({"type":"kdeconnect.sms.messages","body":{"messages":[
            {"address":"600100200","body":"b","date":2000,"type":1},
            {"address":"600100200","body":"a","date":1000,"type":2},
            {"address":"600100200","body":"","date":3000,"type":1},
        ]}});
        let rows = parse_phone_messages(&v);
        assert_eq!(rows.iter().map(|r| r.body.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn threads_are_grouped_by_subscriber() {
        let rows = vec![
            row("+48600100200", "hi", 1, SMS_TYPE_INBOX),
            row("600100200", "hello", 5, SMS_TYPE_SENT),
            row("700111222", "yo", 3, SMS_TYPE_INBOX),
        ];
        let t = group_threads(&rows);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].count, 2);
        assert_eq!(t[0].last_body, "hello");
    }

    #[test]
    fn selection_filters_by_number_direction_and_duplicates() {
        let rows = vec![
            row("+48600100200", "from her", 10, SMS_TYPE_INBOX),
            row("700111222", "someone else", 11, SMS_TYPE_INBOX),
            row("600100200", "typed on phone", 12, SMS_TYPE_SENT),
            row("600100200", "sent from desktop", 13, SMS_TYPE_SENT),
            row("600100200", "draft", 14, 3),
        ];
        let known_ids = HashSet::new();
        let known_out: HashSet<String> = ["sent from desktop".to_string()].into_iter().collect();
        let got = select_new_messages("c1", "dev", "600 100 200", &rows, &known_ids, &known_out);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].body, "from her");
        assert_eq!(got[0].direction, super::super::MessageDirection::Incoming);
        assert_eq!(got[1].body, "typed on phone");
        assert_eq!(got[1].direction, super::super::MessageDirection::Outgoing);

        // Second refresh with the ids now known imports nothing.
        let ids: HashSet<String> = got.iter().map(|m| m.id.clone()).collect();
        assert!(select_new_messages("c1", "dev", "600100200", &rows, &ids, &known_out).is_empty());
    }

    #[tokio::test]
    async fn reader_skips_other_packets_and_reassembles_split_replies() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let writer = tokio::spawn(async move {
            server.write_all(b"{\"type\":\"kdeconnect.ping\",\"body\":{}}\n{\"type\":\"kdeconnect.sms.mess").await.unwrap();
            server.flush().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            server.write_all(b"ages\",\"body\":{\"messages\":[]}}\n").await.unwrap();
            server.flush().await.unwrap();
        });
        let v = read_packet_of_type(&mut client, "kdeconnect.sms.messages", 5).await.unwrap();
        assert_eq!(v["type"], "kdeconnect.sms.messages");
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn reader_reports_a_closed_connection() {
        let (mut client, server) = tokio::io::duplex(64);
        drop(server);
        assert!(read_packet_of_type(&mut client, "x", 2).await.is_err());
    }
}
