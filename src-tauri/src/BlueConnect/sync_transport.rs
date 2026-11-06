#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use super::{get_paired_device, list_paired_devices, local_device_id, open_authenticated_connection_on, paired_device_for_fingerprint, tls};
use crate::lock_ext::LockExt;
use crate::settings_sync::{self as sync, Snapshot};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

pub const SYNC_TCP_PORT: u16 = 1718;
pub const TYPE_EXCHANGE: &str = "blue.sync.exchange";
pub const TYPE_REPLY: &str = "blue.sync.exchange.reply";
const MAX_LINE: u64 = 3 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

pub type Emitter = Arc<dyn Fn(&str, Value) + Send + Sync>;

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static EMITTER: Mutex<Option<Emitter>> = Mutex::new(None);

/// `{"type":…,"body":{"snapshot":…}}\n`
pub fn frame(packet_type: &str, snapshot: &Snapshot) -> Result<Vec<u8>, String> {
    let mut v = serde_json::to_vec(&json!({ "type": packet_type, "body": { "snapshot": snapshot } })).map_err(|e| e.to_string())?;
    v.push(b'\n');
    Ok(v)
}

/// Parses a frame, checking its type.
pub fn parse_frame(line: &[u8], expected_type: &str) -> Result<Snapshot, String> {
    let v: Value = serde_json::from_slice(line).map_err(|e| format!("malformed sync packet: {e}"))?;
    if v.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err("unexpected sync packet type".into());
    }
    serde_json::from_value(v.get("body").and_then(|b| b.get("snapshot")).cloned().ok_or("sync packet has no snapshot")?).map_err(|e| format!("malformed snapshot: {e}"))
}

async fn read_line<R: tokio::io::AsyncRead + Unpin>(r: R) -> Result<Vec<u8>, String> {
    let mut reader = BufReader::new(r.take(MAX_LINE));
    let mut line = Vec::new();
    match tokio::time::timeout(IO_TIMEOUT, reader.read_until(b'\n', &mut line)).await {
        Ok(Ok(0)) => Err("connection closed".into()),
        Ok(Ok(_)) => Ok(line),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("timed out".into()),
    }
}

fn emit(name: &str, payload: Value) {
    if let Some(e) = EMITTER.lock_safe().clone() {
        e(name, payload);
    }
}

async fn handle_connection(tcp: tokio::net::TcpStream) -> Result<(), String> {
    let captured = Arc::new(Mutex::new(None));
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls::server_config(captured.clone())?));
    let mut stream = tokio::time::timeout(IO_TIMEOUT, acceptor.accept(tcp)).await.map_err(|_| "TLS handshake timed out".to_string())?.map_err(|e| format!("TLS handshake failed: {e}"))?;
    let fingerprint = captured.lock_safe().clone().ok_or("no client certificate")?;
    // Who is this? Only a device we paired with can be anybody.
    let (peer_id, peer_name) = paired_device_for_fingerprint(&fingerprint).ok_or("peer is not a paired device")?;
    if !sync::is_enabled(&peer_id) {
        return Err(format!("sync is not enabled for {peer_name}"));
    }
    let line = read_line(&mut stream).await?;
    let theirs = parse_frame(&line, TYPE_EXCHANGE)?;
    let reply = sync::handle_exchange(&peer_id, theirs, &local_device_id())?;
    stream.write_all(&frame(TYPE_REPLY, &reply)?).await.map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())?;
    emit("blue-sync://remote-changes", json!({ "deviceId": peer_id, "deviceName": peer_name }));
    Ok(())
}

async fn serve() {
    let listener = match TcpListener::bind(("0.0.0.0", SYNC_TCP_PORT)).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("settings sync: cannot listen on {SYNC_TCP_PORT}: {e}");
            SERVER_STARTED.store(false, Ordering::SeqCst);
            return;
        }
    };
    tracing::info!("settings sync: listening on {SYNC_TCP_PORT}");
    loop {
        let Ok((tcp, addr)) = listener.accept().await else { continue };
        tokio::spawn(async move {
            if let Err(e) = handle_connection(tcp).await {
                tracing::debug!("settings sync: dropped connection from {}: {e}", addr.ip());
            }
        });
    }
}

/// Starts the listener once (idempotent). Called when sync is enabled for a device and at start-up if any is.
pub fn ensure_server(emitter: Emitter) {
    *EMITTER.lock_safe() = Some(emitter);
    if SERVER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(serve());
}

/// Start-up hook.
pub fn start_if_enabled(emitter: Emitter) {
    if sync::any_enabled() {
        ensure_server(emitter);
    } else {
        *EMITTER.lock_safe() = Some(emitter);
    }
}

// ── client side ────────────────────────────────────────────────────────────

pub async fn exchange(device_id: &str) -> Result<sync::Applied, String> {
    if !sync::is_enabled(device_id) {
        return Err("Sync is not enabled for this device".into());
    }
    let handle = get_paired_device(device_id).ok_or("That device is no longer paired")?;
    let snapshot = sync::load().store.snapshot(&local_device_id(), sync::now_ms());
    let result = async {
        let mut stream = open_authenticated_connection_on(&handle, SYNC_TCP_PORT).await?;
        stream.write_all(&frame(TYPE_EXCHANGE, &snapshot)?).await.map_err(|e| format!("failed to send to {}: {e}", handle.device_name))?;
        stream.flush().await.map_err(|e| e.to_string())?;
        let line = read_line(&mut stream).await.map_err(|e| format!("{} did not answer: {e} — is sync enabled on it too?", handle.device_name))?;
        let theirs = parse_frame(&line, TYPE_REPLY)?;
        sync::apply_remote(device_id, theirs, &local_device_id())
    }
    .await;
    if let Err(e) = &result {
        sync::note_error(device_id, e);
    }
    result
}

// ── commands ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPeer {
    pub device_id: String,
    pub name: String,
    pub enabled: bool,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub device_id: String,
    pub fields: Vec<&'static str>,
    pub peers: Vec<SyncPeer>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingChange {
    pub key: String,
    pub value: Value,
    pub deleted: bool,
}

/// Paired devices and their sync state.
#[tauri::command]
pub fn sync_status() -> SyncStatus {
    let state = sync::load();
    SyncStatus {
        device_id: local_device_id(),
        fields: sync::SYNC_FIELDS.to_vec(),
        peers: list_paired_devices()
            .into_iter()
            .map(|d| {
                let p = state.peers.get(&d.device_id).cloned().unwrap_or_default();
                SyncPeer { device_id: d.device_id, name: d.device_name, enabled: p.enabled, last_sync: p.last_sync, last_error: p.last_error }
            })
            .collect(),
    }
}

#[tauri::command]
pub fn sync_set_device_enabled(app: tauri::AppHandle, device_id: String, enabled: bool) -> Result<(), String> {
    use tauri::Emitter as _;
    if get_paired_device(&device_id).is_none() {
        return Err("That device is not paired".into());
    }
    sync::set_enabled(&device_id, enabled)?;
    if enabled {
        ensure_server(Arc::new(move |name, payload| {
            let _ = app.emit(name, payload);
        }));
    }
    Ok(())
}

/// The shell reports settings that the person changed *on this computer*.
#[tauri::command]
pub fn sync_report_local(changes: BTreeMap<String, Option<Value>>) -> Result<usize, String> {
    sync::report_local(&local_device_id(), changes)
}

/// Exchanges settings with one device now; returns what must be applied locally.
#[tauri::command]
pub async fn sync_now(device_id: String) -> Result<Vec<PendingChange>, String> {
    exchange(&device_id).await?;
    Ok(sync_take_pending())
}

/// Remote changes waiting to be applied (drained).
#[tauri::command]
pub fn sync_take_pending() -> Vec<PendingChange> {
    sync::take_pending().into_iter().map(|(key, e)| PendingChange { key, value: e.value, deleted: e.deleted }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_sync::{Entry, SCHEMA_VERSION};

    fn snap() -> Snapshot {
        Snapshot { schema: SCHEMA_VERSION, device_id: "d".into(), generated_at: 1, entries: [("theme".to_string(), Entry { value: json!("dark"), updated_at: 5, origin: "d".into(), deleted: false })].into_iter().collect() }
    }

    #[test]
    fn frames_round_trip_and_check_their_type() {
        let f = frame(TYPE_EXCHANGE, &snap()).unwrap();
        assert!(f.ends_with(b"\n"));
        assert_eq!(parse_frame(&f, TYPE_EXCHANGE).unwrap(), snap());
        assert!(parse_frame(&f, TYPE_REPLY).is_err(), "a reply must not be accepted as a request");
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_frame(b"not json", TYPE_EXCHANGE).is_err());
        assert!(parse_frame(br#"{"type":"blue.sync.exchange","body":{}}"#, TYPE_EXCHANGE).is_err());
        assert!(parse_frame(br#"{"type":"blue.sync.exchange","body":{"snapshot":{"schema":"x"}}}"#, TYPE_EXCHANGE).is_err());
    }

    #[tokio::test]
    async fn oversize_lines_are_cut_off() {
        let big = vec![b'a'; (MAX_LINE as usize) + 10];
        let r = read_line(&big[..]).await.unwrap();
        assert_eq!(r.len() as u64, MAX_LINE, "reading stops at the cap instead of buffering unbounded input");
    }
}
