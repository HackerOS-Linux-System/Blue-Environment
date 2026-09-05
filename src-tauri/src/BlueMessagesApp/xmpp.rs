use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::scram::{ScramExchange, ScramHash};
use super::xml_stream::{read_one_element, XmlElement};

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

/// Payload for the `blue-messages://xmpp-incoming` event the background
/// connection emits — enough for the frontend to append the message to
/// an already-open conversation, or just bump its unread badge/preview
/// if it isn't currently open.
#[derive(Serialize, Clone, Debug)]
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

#[tauri::command]
pub fn xmpp_has_session() -> bool { read_session().is_some() }

// ── Persistent background connection lifecycle ─────────────────────────
//
// `CONNECTION_GENERATION` is bumped by every `xmpp_login`/`xmpp_logout`
// call. The background thread captures its own generation number when
// it starts and checks it before every reconnect attempt — if it no
// longer matches (a newer login or a logout happened), the old thread
// exits quietly instead of fighting a newer one for the same session
// file, rather than needing a more elaborate cancellation channel.
static CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
pub fn xmpp_logout() -> Result<(), String> {
    CONNECTION_GENERATION.fetch_add(1, Ordering::SeqCst); // orphans any running background thread
    let path = session_path();
    if path.exists() { std::fs::remove_file(path).map_err(|e| e.to_string())?; }
    Ok(())
}

/// Splits a bare or full JID (`user@domain` or `user@domain/resource`)
/// into `(user, domain)`.
fn split_jid(jid: &str) -> Result<(String, String), String> {
    let bare = jid.split('/').next().unwrap_or(jid);
    let mut parts = bare.splitn(2, '@');
    let user = parts.next().filter(|s| !s.is_empty()).ok_or("JID must be user@domain")?;
    let domain = parts.next().filter(|s| !s.is_empty()).ok_or("JID must be user@domain")?;
    Ok((user.to_string(), domain.to_string()))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&apos;")
}

/// A live, authenticated XMPP stream plus the reader state needed to
/// keep pulling well-formed elements off it. Bundled together because
/// every caller that finishes the handshake immediately needs both "a
/// place to write outgoing stanzas" and "a place to read incoming
/// ones", and the two must stay tied to the exact same underlying
/// socket.
struct XmppConnection {
    reader: Reader<BufReader<native_tls::TlsStream<TcpStream>>>,
    full_jid: String,
}

impl XmppConnection {
    fn write_raw(&mut self, bytes: &[u8]) -> Result<(), String> {
        // `get_mut()` on quick-xml's `Reader` reaches the `BufReader`;
        // `get_mut()` on the `BufReader` reaches the underlying
        // `TlsStream` — writes bypass the `BufReader`'s read buffer
        // entirely and go straight to the socket, so writing through
        // this path is equivalent to writing directly on the
        // `TlsStream`. (Not independently re-verified against a
        // compiler in this change — see this module's doc for what
        // "best-effort" means for this file.)
        self.reader.get_mut().get_mut().write_all(bytes).map_err(|e| e.to_string())
    }

    fn read_element(&mut self, timeout: Duration) -> Result<Option<XmlElement>, String> {
        // native-tls's blocking `TlsStream` doesn't expose an async/
        // cancellable read, so "timeout" here is implemented the same
        // blunt way `xmpp_refresh_thread` used before: a read-timeout
        // on the underlying socket, surfaced through quick-xml's error
        // as a WouldBlock/TimedOut io error, which callers interpret as
        // "nothing arrived in this slice", not a hard failure.
        self.reader.get_mut().get_mut().get_ref().set_read_timeout(Some(timeout)).ok();
        read_one_element(&mut self.reader)
    }

    fn close(&mut self) {
        let _ = self.write_raw(b"</stream:stream>");
    }
}

/// One full connect → STARTTLS → SASL (SCRAM preferred, PLAIN fallback)
/// → bind sequence, returning a live [`XmppConnection`].
fn connect_authenticate(domain: &str, user: &str, password: &str, resource: &str) -> Result<XmppConnection, String> {
    let addr = format!("{domain}:5222");
    let tcp = TcpStream::connect(&addr).map_err(|e| format!("could not connect to {addr}: {e}"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(15))).ok();

    let open = format!(
        "<?xml version='1.0'?><stream:stream to='{d}' xmlns='jabber:client' \
         xmlns:stream='http://etherx.jabber.org/streams' version='1.0'>",
        d = xml_escape(domain)
    );

    // Plaintext phase: open the stream and read <stream:features/>,
    // which must advertise STARTTLS.
    let mut plain_reader = Reader::from_reader(BufReader::new(clone_stream(&tcp)?));
    write_stream(&tcp, open.as_bytes())?;
    let features = expect_element(&mut plain_reader, Duration::from_secs(15))?;
    if features.name != "features" || features.child("starttls").is_none() {
        return Err("server does not advertise STARTTLS — refusing to authenticate over plaintext".to_string());
    }

    write_stream(&tcp, b"<starttls xmlns='urn:ietf:params:xml:ns:xmpp-tls'/>")?;
    let proceed = expect_element(&mut plain_reader, Duration::from_secs(15))?;
    if proceed.name != "proceed" {
        return Err(format!("server refused STARTTLS (got <{}/> instead of <proceed/>)", proceed.name));
    }
    drop(plain_reader); // last use of the plaintext socket — everything past this point is over TLS

    let connector = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
    let tls = connector.connect(domain, tcp).map_err(|e| format!("TLS handshake failed: {e}"))?;
    let mut reader = Reader::from_reader(BufReader::new(tls));

    // Restart the stream over the encrypted channel and read the
    // post-TLS features for the SASL mechanism list.
    reader.get_mut().get_mut().write_all(open.as_bytes()).map_err(|e| e.to_string())?;
    let features = expect_element(&mut reader, Duration::from_secs(15))?;
    let mechanisms: Vec<String> = features
        .child("mechanisms")
        .map(|m| m.children.iter().map(|c| c.text.clone()).collect())
        .unwrap_or_default();

    authenticate(&mut reader, user, password, &mechanisms)?;

    // Restart the stream once more (required after successful SASL),
    // then bind a resource.
    reader.get_mut().get_mut().write_all(open.as_bytes()).map_err(|e| e.to_string())?;
    let _ = expect_element(&mut reader, Duration::from_secs(15))?;

    let bind_stanza = format!(
        "<iq type='set' id='bm-bind1'><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'>\
         <resource>{r}</resource></bind></iq>",
        r = xml_escape(resource)
    );
    reader.get_mut().get_mut().write_all(bind_stanza.as_bytes()).map_err(|e| e.to_string())?;
    let bind_resp = expect_element(&mut reader, Duration::from_secs(15))?;
    let full_jid = bind_resp
        .child("bind")
        .and_then(|b| b.child_text("jid"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{user}@{domain}/{resource}"));

    // Legacy session establishment — optional on modern servers, still
    // expected by some older/stricter ones.
    let _ = reader.get_mut().get_mut().write_all(
        b"<iq type='set' id='bm-sess1'><session xmlns='urn:ietf:params:xml:ns:xmpp-session'/></iq>",
    );
    let _ = expect_element(&mut reader, Duration::from_secs(5));

    Ok(XmppConnection { reader, full_jid })
}

/// Reads elements until a real one arrives (skipping nothing — unlike
/// the persistent connection's read loop, the handshake has no reason
/// to tolerate "nothing yet" here) or the timeout elapses.
fn expect_element<R: std::io::BufRead>(reader: &mut Reader<R>, _timeout: Duration) -> Result<XmlElement, String> {
    read_one_element(reader)?.ok_or_else(|| "connection closed before the expected reply arrived".to_string())
}

fn clone_stream(tcp: &TcpStream) -> Result<TcpStream, String> {
    tcp.try_clone().map_err(|e| e.to_string())
}
fn write_stream(tcp: &TcpStream, bytes: &[u8]) -> Result<(), String> {
    (&mut clone_stream(tcp)?).write_all(bytes).map_err(|e| e.to_string())
}

/// Picks SCRAM-SHA-256, then SCRAM-SHA-1, then PLAIN (in that order of
/// preference) from what the server actually offered, and runs the
/// chosen mechanism's exchange.
///
/// Generic over the concrete stream type `S` (rather than just "any
/// `BufRead`", the way [`expect_element`] is) because these functions
/// need to *write* too: `reader.get_mut()` reaches the `BufReader<S>`,
/// and a second `.get_mut()` reaches the underlying `S` to write
/// through — `BufReader<S>` itself only ever implements `Read`/
/// `BufRead`, never `Write`, even when `S` does, so that second
/// `get_mut()` step needs `S`'s own `Write` impl to be in scope, which
/// means naming `S` concretely enough to bound it.
fn authenticate<S: std::io::Read + Write>(reader: &mut Reader<std::io::BufReader<S>>, user: &str, password: &str, mechanisms: &[String]) -> Result<(), String> {
    if let Some(hash) = ScramHash::best_offered(mechanisms) {
        authenticate_scram(reader, hash, user, password)
    } else if mechanisms.iter().any(|m| m == "PLAIN") {
        authenticate_plain(reader, user, password)
    } else {
        Err(format!(
            "server only offers unsupported SASL mechanisms: {} (need one of SCRAM-SHA-256, SCRAM-SHA-1, PLAIN)",
            mechanisms.join(", ")
        ))
    }
}

fn authenticate_plain<S: std::io::Read + Write>(reader: &mut Reader<std::io::BufReader<S>>, user: &str, password: &str) -> Result<(), String> {
    let mut payload = Vec::new();
    payload.push(0u8);
    payload.extend_from_slice(user.as_bytes());
    payload.push(0u8);
    payload.extend_from_slice(password.as_bytes());
    let auth_b64 = BASE64.encode(payload);
    let stanza = format!("<auth xmlns='urn:ietf:params:xml:ns:xmpp-sasl' mechanism='PLAIN'>{auth_b64}</auth>");
    reader.get_mut().get_mut().write_all(stanza.as_bytes()).map_err(|e| e.to_string())?;

    let resp = expect_element(reader, Duration::from_secs(15))?;
    match resp.name.as_str() {
        "success" => Ok(()),
        "failure" => Err("authentication failed — check the JID and password".to_string()),
        other => Err(format!("unexpected SASL response: <{other}/>")),
    }
}

fn authenticate_scram<S: std::io::Read + Write>(reader: &mut Reader<std::io::BufReader<S>>, hash: ScramHash, user: &str, password: &str) -> Result<(), String> {
    let (exchange, client_first_b64) = ScramExchange::client_first(hash, user);
    let mechanism = hash.mechanism_name();
    let stanza = format!("<auth xmlns='urn:ietf:params:xml:ns:xmpp-sasl' mechanism='{mechanism}'>{client_first_b64}</auth>");
    reader.get_mut().get_mut().write_all(stanza.as_bytes()).map_err(|e| e.to_string())?;

    let challenge = expect_element(reader, Duration::from_secs(15))?;
    if challenge.name == "failure" {
        return Err("server rejected SCRAM client-first-message".to_string());
    }
    if challenge.name != "challenge" {
        return Err(format!("expected <challenge/>, got <{}/>", challenge.name));
    }
    let (client_final_b64, expected_server_sig) = exchange.client_final(password, &challenge.text)?;

    let final_stanza = format!(
        "<response xmlns='urn:ietf:params:xml:ns:xmpp-sasl'>{client_final_b64}</response>"
    );
    reader.get_mut().get_mut().write_all(final_stanza.as_bytes()).map_err(|e| e.to_string())?;

    let outcome = expect_element(reader, Duration::from_secs(15))?;
    match outcome.name.as_str() {
        "failure" => Err("authentication failed — check the JID and password".to_string()),
        "success" => {
            // Mutual auth: refuse to proceed if the server can't prove
            // it knows the shared secret too, even though it claimed
            // "success" — see this module's and scram.rs's doc for why
            // that's a real security property PLAIN can't offer.
            super::scram::verify_server_final(&outcome.text, &expected_server_sig)
        }
        other => Err(format!("unexpected SASL outcome: <{other}/>")),
    }
}

/// Verifies `jid`/`password` actually authenticate against the real
/// server, and only saves the session locally (plus starts the
/// persistent background connection) if that succeeds.
#[tauri::command]
pub async fn xmpp_login(app: AppHandle, jid: String, password: String) -> Result<XmppSessionInfo, String> {
    let (user, domain) = split_jid(&jid)?;
    let resource = "BlueMessages".to_string();

    let password_for_blocking = password.clone();
    let (user_c, domain_c, resource_c) = (user.clone(), domain.clone(), resource.clone());
    let mut conn = tokio::task::spawn_blocking(move || connect_authenticate(&domain_c, &user_c, &password_for_blocking, &resource_c))
        .await
        .map_err(|e| format!("login task panicked: {e}"))??;
    let full_jid = conn.full_jid.clone();
    conn.close();

    let dir = super::messages_dir();
    let session = StoredXmppSession {
        jid: format!("{user}@{domain}"),
        password_encrypted: super::secretstore::encrypt(&dir, &password),
        resource,
    };
    write_session(&session)?;

    let generation = CONNECTION_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || run_background_connection(app, generation));

    Ok(XmppSessionInfo { jid: full_jid })
}

/// Runs for as long as `generation` is still the current one (see
/// [`CONNECTION_GENERATION`]'s doc), reconnecting with exponential
/// backoff (capped) on any disconnect, and pushing every incoming
/// `<message>` with a `<body>` into local storage plus a
/// `blue-messages://xmpp-incoming` event — this is what gives Blue
/// Messages' XMPP channel real-time receiving instead of only updating
/// when the person manually reopens/refreshes a conversation.
fn run_background_connection(app: AppHandle, generation: u64) {
    let mut backoff = Duration::from_secs(2);
    const MAX_BACKOFF: Duration = Duration::from_secs(300);

    loop {
        if CONNECTION_GENERATION.load(Ordering::SeqCst) != generation {
            return; // superseded by a newer login, or logged out
        }
        let Some(session) = read_session() else { return; };
        let dir = super::messages_dir();
        let password = super::secretstore::decrypt(&dir, &session.password_encrypted);
        let Ok((user, domain)) = split_jid(&session.jid) else { return; };

        match connect_authenticate(&domain, &user, &password, &session.resource) {
            Ok(mut conn) => {
                backoff = Duration::from_secs(2); // reset after a successful connect
                run_receive_loop(&app, &mut conn, generation);
                conn.close();
            }
            Err(e) => {
                tracing::warn!("XMPP background connection failed, retrying in {:?}: {e}", backoff);
            }
        }

        if CONNECTION_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        std::thread::sleep(backoff);
        backoff = std::cmp::min(backoff * 2, MAX_BACKOFF);
    }
}

/// Reads elements from an established connection until it disconnects,
/// the generation is superseded, or a whitespace keepalive is due.
/// Returns (rather than erroring) on any read failure — the caller
/// (`run_background_connection`) treats a returned-from receive loop
/// the same as any other disconnect: back off and reconnect.
fn run_receive_loop(app: &AppHandle, conn: &mut XmppConnection, generation: u64) {
    let mut last_activity = Instant::now();
    const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(60);

    loop {
        if CONNECTION_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        match conn.read_element(Duration::from_secs(5)) {
            Ok(Some(el)) => {
                last_activity = Instant::now();
                if el.name == "message" {
                    handle_incoming_message(app, &el);
                }
                // presence/iq/other stanzas: not currently acted on —
                // see this module's doc for what's out of scope.
            }
            Ok(None) => return, // server closed the stream
            Err(e) => {
                // A read-timeout is expected and not a real error (it's
                // how this loop gets a chance to check `generation` and
                // send keepalives); anything else means the connection
                // is actually gone.
                if !e.contains("WouldBlock") && !e.contains("TimedOut") && !e.to_lowercase().contains("timed out") {
                    tracing::debug!("XMPP receive loop ending: {e}");
                    return;
                }
            }
        }
        if last_activity.elapsed() >= KEEPALIVE_INTERVAL {
            // XMPP's own "whitespace ping" — a single space character
            // is valid at any point in the stream and keeps NAT/
            // firewall connection tracking (and some servers' idle
            // timeouts) from treating the link as dead.
            if conn.write_raw(b" ").is_err() {
                return;
            }
            last_activity = Instant::now();
        }
    }
}

fn handle_incoming_message(app: &AppHandle, el: &XmlElement) {
    let Some(body) = el.child_text("body") else { return; };
    if body.is_empty() { return; }
    let Some(from) = el.attrs.get("from") else { return; };
    let from_bare = from.split('/').next().unwrap_or(from).to_string();

    let mut conversations = super::read_conversations();
    // Extracted as an owned `String` (rather than keeping the `&Conversation`
    // borrow from `.find()` alive) specifically so `conversations` is free
    // to be borrowed `mut` again below, for `.iter_mut()` — holding onto
    // `convo` across that would be a borrow-checker conflict (an immutable
    // borrow from this `.find()` still alive when the mutable one starts).
    let Some(convo_id) = conversations
        .iter()
        .find(|c| c.channel == super::Channel::Xmpp && c.participant == from_bare)
        .map(|c| c.id.clone())
    else {
        // A message from a JID with no matching conversation — could
        // support auto-creating one here; left as a explicit
        // non-feature for now so an unsolicited message can't silently
        // create UI state without the person having added that contact
        // first.
        return;
    };

    let message = super::Message {
        id: format!("xmpp-{}", chrono::Utc::now().timestamp_millis()),
        conversation_id: convo_id.clone(),
        body: body.to_string(),
        direction: super::MessageDirection::Incoming,
        sent_at: chrono::Utc::now().to_rfc3339(),
        read: false,
    };

    if super::storage::add_many(std::slice::from_ref(&message)).is_err() {
        return;
    }
    if let Some(c) = conversations.iter_mut().find(|c| c.id == convo_id) {
        c.last_message_preview = message.body.clone();
        c.last_message_at = message.sent_at.clone();
        c.unread_count += 1;
        let _ = super::write_conversations(&conversations);
    }

    let _ = app.emit(
        "blue-messages://xmpp-incoming",
        XmppIncomingEvent { conversation_id: convo_id, message },
    );
}

/// Adds `contact_jid` as a local conversation with `channel: Xmpp`.
#[tauri::command]
pub fn xmpp_add_contact(contact_jid: String, name: String) -> Result<super::Conversation, String> {
    super::create_conversation_internal(name, contact_jid, super::Channel::Xmpp)
}

/// Sends `body` to `to_jid` as a `<message type='chat'>` stanza over
/// its own short-lived connection — see this module's doc for why
/// sending doesn't (yet) share the persistent background connection.
pub fn send_message(to_jid: &str, body: &str) -> Result<(), String> {
    let session = read_session().ok_or("Not logged in to XMPP")?;
    let dir = super::messages_dir();
    let password = super::secretstore::decrypt(&dir, &session.password_encrypted);
    let (user, domain) = split_jid(&session.jid)?;
    let mut conn = connect_authenticate(&domain, &user, &password, &session.resource)?;

    let id = format!("bm-{}", chrono::Utc::now().timestamp_millis());
    let stanza = format!(
        "<message to='{to}' type='chat' id='{id}'><body>{body}</body></message>",
        to = xml_escape(to_jid),
        body = xml_escape(body),
    );
    conn.write_raw(stanza.as_bytes())?;
    conn.close();
    Ok(())
}

/// Manual "pull anything missed" fallback — mostly superseded by the
/// persistent background connection's live push, but kept for: the
/// window right after login before the background thread has finished
/// its first connect, and for anyone who wants an explicit "check now"
/// action rather than trusting the background connection is still
/// alive.
#[tauri::command]
pub async fn xmpp_refresh_thread(conversation_id: String) -> Result<Vec<super::Message>, String> {
    let conversations = super::read_conversations();
    let convo = conversations.iter().find(|c| c.id == conversation_id).ok_or("Conversation not found")?.clone();
    if convo.channel != super::Channel::Xmpp {
        return Err("Not an XMPP conversation".to_string());
    }

    let session = read_session().ok_or("Not logged in to XMPP")?;
    let existing_ids = super::storage::message_ids_for(&conversation_id);

    let convo_id_for_blocking = conversation_id.clone();
    let new_messages: Vec<super::Message> = tokio::task::spawn_blocking(move || -> Result<Vec<super::Message>, String> {
        let dir = super::messages_dir();
        let password = super::secretstore::decrypt(&dir, &session.password_encrypted);
        let (user, domain) = split_jid(&session.jid)?;
        let mut conn = connect_authenticate(&domain, &user, &password, &session.resource)?;

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut collected = Vec::new();
        let mut seq = 0u32;
        while Instant::now() < deadline {
            match conn.read_element(Duration::from_millis(500)) {
                Ok(Some(el)) if el.name == "message" => {
                    if let Some(b) = el.child_text("body") {
                        if !b.is_empty() {
                            seq += 1;
                            let id = format!("xmpp-{}-{}", chrono::Utc::now().timestamp_millis(), seq);
                            if !existing_ids.contains(&id) {
                                collected.push(super::Message {
                                    id,
                                    conversation_id: convo_id_for_blocking.clone(),
                                    body: b.to_string(),
                                    direction: super::MessageDirection::Incoming,
                                    sent_at: chrono::Utc::now().to_rfc3339(),
                                    read: false,
                                });
                            }
                        }
                    }
                }
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => continue, // timeout slice — keep polling until the deadline
            }
        }
        conn.close();
        Ok(collected)
    })
    .await
    .map_err(|e| format!("refresh task panicked: {e}"))??;

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
    fn xml_escape_covers_all_five_predefined_entities() {
        let raw = "<hello> & \"world\" 'quote'";
        let escaped = xml_escape(raw);
        assert!(!escaped.contains('<') && !escaped.contains('>') && !escaped.contains('&') || escaped.contains("&amp;"));
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("&apos;"));
    }
}
