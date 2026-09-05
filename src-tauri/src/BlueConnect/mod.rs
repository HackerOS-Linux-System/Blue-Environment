pub(crate) mod tls;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tauri::{AppHandle, Emitter};

/// A pairing request currently waiting on this device's own person to
/// accept/reject it — set by `bc_listen_for_pairing` right after it
/// reads the peer's request and computes the SAS (see `tls.rs`'s
/// `compute_sas`), consumed by `bc_confirm_incoming_pairing` once the
/// person responds. `oneshot` rather than a general channel since
/// there's ever exactly one decision to deliver.
static PENDING_INCOMING_PAIRING: std::sync::OnceLock<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>> = std::sync::OnceLock::new();
fn pending_incoming_pairing() -> &'static Mutex<Option<tokio::sync::oneshot::Sender<bool>>> {
    PENDING_INCOMING_PAIRING.get_or_init(|| Mutex::new(None))
}

/// KDE Connect's real discovery port — deliberately the same value so
/// this can at least *see* real KDE Connect/GSConnect devices on the
/// same LAN (this project's own pairing handshake now uses real TLS —
/// see `tls.rs` — but it's still Blue Connect's own certificate/trust
/// scheme, not KDE Connect's exact protocol, so pairing still only
/// meaningfully succeeds device-to-device between two Blue Connect
/// instances).
pub const DISCOVERY_PORT: u16 = 1716;
/// This device's own advertised TCP port for the pairing handshake —
/// separate from the UDP discovery port, matching KDE Connect's own
/// split (UDP broadcast for "I exist", TCP for anything after that).
pub const PAIRING_TCP_PORT: u16 = 1717;
const PROTOCOL_VERSION: u32 = 7; // matches KDE Connect's own current identity protocolVersion

fn connect_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-connect")
}
fn devices_path() -> PathBuf {
    connect_dir().join("devices.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Phone,
    Tablet,
    Desktop,
    Laptop,
    Tv,
    Unknown,
}

impl DeviceType {
    fn from_str_loose(s: &str) -> Self {
        match s {
            "phone" => Self::Phone,
            "tablet" => Self::Tablet,
            "desktop" => Self::Desktop,
            "laptop" => Self::Laptop,
            "tv" => Self::Tv,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub address: String,
    pub tcp_port: u16,
    pub paired: bool,
    /// SHA-256 fingerprint of the TLS certificate this device presented
    /// the moment it was paired — see `tls.rs`'s module doc. `None` for
    /// devices discovered but never paired, or devices paired before
    /// this field existed (in which case pairing was plaintext and
    /// there's nothing to pin retroactively; re-pairing populates it).
    #[serde(default)]
    pub pinned_cert_sha256: Option<String>,
}

/// The identity packet's shape on the wire — matches KDE Connect's
/// real `kdeconnect.identity` body fields closely enough to parse
/// theirs (`deviceId`/`deviceName`/`deviceType`/`tcpPort` all match
/// their naming). Certificate identity is no longer advertised in this
/// packet at all — it's exchanged as part of the TLS handshake itself
/// (see `tls.rs`) once a pairing connection actually opens, the same
/// place KDE Connect's own protocol carries it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IdentityBody {
    #[serde(rename = "deviceId")]
    device_id: String,
    #[serde(rename = "deviceName")]
    device_name: String,
    #[serde(rename = "deviceType")]
    device_type: String,
    #[serde(rename = "protocolVersion")]
    protocol_version: u32,
    #[serde(rename = "tcpPort")]
    tcp_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IdentityPacket {
    #[serde(rename = "type")]
    packet_type: String,
    body: IdentityBody,
}

fn this_device_id() -> String {
    // Stable per-install id, generated once and cached — mirrors what
    // KDE Connect itself does (a random id persisted alongside its
    // config, not derived from hardware, so it survives a hostname
    // change but not a fresh install).
    let path = connect_dir().join("device_id");
    if let Ok(existing) = fs::read_to_string(&path) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    let id = format!("blue-connect-{}", uuid_like());
    let _ = fs::create_dir_all(connect_dir());
    let _ = fs::write(&path, &id);
    id
}

/// A short, sufficiently-unique id without pulling in a full `uuid`
/// crate for one call site — timestamp plus a bit of process/thread
/// entropy is plenty for "identify this install among a handful of
/// devices on someone's LAN," which is this id's only job.
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:x}", nanos)
}

fn this_device_name() -> String {
    hostname::get().ok().and_then(|h| h.into_string().ok()).unwrap_or_else(|| "Blue Environment".to_string())
}

static KNOWN_DEVICES: Mutex<Option<HashMap<String, DiscoveredDevice>>> = Mutex::new(None);

fn with_known_devices<T>(f: impl FnOnce(&mut HashMap<String, DiscoveredDevice>) -> T) -> T {
    let mut guard = KNOWN_DEVICES.lock().unwrap();
    if guard.is_none() {
        *guard = Some(load_devices());
    }
    f(guard.as_mut().unwrap())
}

fn load_devices() -> HashMap<String, DiscoveredDevice> {
    fs::read_to_string(devices_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_devices(devices: &HashMap<String, DiscoveredDevice>) {
    let _ = fs::create_dir_all(connect_dir());
    if let Ok(json) = serde_json::to_string_pretty(devices) {
        let _ = fs::write(devices_path(), json);
    }
}

// ── Public API for other modules to talk to an already-paired device ──
//
// Blue Messages' phone-relayed SMS channel (`BlueMessagesApp/sms.rs`)
// is the first consumer of this, but it's deliberately generic — any
// future KDE-Connect-style "plugin" (clipboard sync, find-my-device,
// notification mirroring, ...) that needs to send/receive its own
// packets to a paired device over the same mutual-TLS transport
// pairing already established can reuse this instead of re-implementing
// connect+TLS+fingerprint-pinning-check itself.

/// The subset of a paired device's info another module actually needs
/// to open its own connection to it — deliberately not the full
/// `DiscoveredDevice` (which includes discovery-only fields like
/// `device_type` that a packet-relay caller has no use for).
pub struct PairedDeviceHandle {
    pub device_id: String,
    pub device_name: String,
    pub address: String,
    pub tcp_port: u16,
    pinned_cert_sha256: String,
}

/// Serializable, frontend-facing view of a paired device — just enough
/// for a "send SMS through..." picker to show a name and pass the id
/// back. Deliberately excludes `address`/`tcp_port`/the pinned
/// fingerprint, which are only meaningful to backend code that's about
/// to open a connection.
#[derive(Serialize, Clone)]
pub struct PairedDeviceHandlePublic {
    pub device_id: String,
    pub device_name: String,
}

/// Looks up `device_id` and returns a handle iff it's both marked
/// `paired` and has a pinned certificate fingerprint on file (i.e. it
/// went through the real SAS-confirmed pairing flow — see
/// `bc_request_pairing`/`bc_listen_for_pairing`). Returns `None`
/// otherwise, including for devices merely *discovered* but never
/// paired — callers should treat that the same as "not available",
/// same as `sms.rs`'s `sms_modem_available` treats a missing modem.
pub fn get_paired_device(device_id: &str) -> Option<PairedDeviceHandle> {
    with_known_devices(|devices| {
        let dev = devices.get(device_id)?;
        if !dev.paired {
            return None;
        }
        let fp = dev.pinned_cert_sha256.clone()?;
        Some(PairedDeviceHandle {
            device_id: dev.id.clone(),
            device_name: dev.name.clone(),
            address: dev.address.clone(),
            tcp_port: dev.tcp_port,
            pinned_cert_sha256: fp,
        })
    })
}

/// Lists every currently paired device — used by other modules to
/// offer a "relay through..." picker (e.g. Blue Messages' "new SMS
/// conversation" flow choosing which paired phone to send through)
/// without reaching into this module's private device registry.
pub fn list_paired_devices() -> Vec<PairedDeviceHandle> {
    with_known_devices(|devices| {
        devices
            .values()
            .filter(|d| d.paired)
            .filter_map(|d| {
                d.pinned_cert_sha256.clone().map(|fp| PairedDeviceHandle {
                    device_id: d.id.clone(),
                    device_name: d.name.clone(),
                    address: d.address.clone(),
                    tcp_port: d.tcp_port,
                    pinned_cert_sha256: fp,
                })
            })
            .collect()
    })
}

/// Opens a fresh mutual-TLS connection to an already-paired device,
/// verifying its certificate matches the fingerprint pinned when it
/// was paired — the same check `bc_request_pairing` does before
/// re-pairing, reused here for every ordinary (non-pairing) packet
/// exchange too, since an already-paired relationship should never
/// silently start trusting a different key.
///
/// Returns the live stream for the caller to write/read whatever
/// packets its own plugin protocol needs (e.g. `sms.rs`'s
/// `kdeconnect.sms.*` packets) — this function only owns the
/// transport/trust layer, not any particular packet format.
pub async fn open_authenticated_connection(handle: &PairedDeviceHandle) -> Result<tokio_rustls::client::TlsStream<TcpStream>, String> {
    let addr = format!("{}:{}", handle.address, handle.tcp_port);
    let tcp = TcpStream::connect(&addr).await.map_err(|e| format!("failed to connect to {addr}: {e}"))?;

    let captured = Arc::new(Mutex::new(None));
    let config = tls::client_config(captured.clone())?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let server_name = rustls::pki_types::ServerName::IpAddress(
        handle.address.parse::<std::net::IpAddr>()
            .map_err(|_| format!("invalid device address: {}", handle.address))?
            .into(),
    );
    let stream = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| format!("TLS handshake with {addr} failed: {e}"))?;

    let peer_fingerprint = captured.lock().unwrap().clone().ok_or("TLS handshake completed without a peer certificate")?;
    if peer_fingerprint != handle.pinned_cert_sha256 {
        return Err(format!(
            "Refusing to talk to {}: it presented a different certificate than the one pinned when it was paired. \
             Use \"Forget device\" and re-pair only if you're sure it's really {0}.",
            handle.device_name
        ));
    }

    Ok(stream)
}

/// Broadcasts one `kdeconnect.identity` UDP packet announcing this
/// device, then listens for up to `timeout_secs` seconds for replies
/// from other devices on the LAN, merging anything heard into the
/// known-devices list (persisted — see [`bc_get_devices`]). Real
/// network I/O: this genuinely sends/receives UDP broadcast packets,
/// not a simulated device list.
#[tauri::command]
pub async fn bc_start_discovery(timeout_secs: u64) -> Result<Vec<DiscoveredDevice>, String> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).await.map_err(|e| format!("failed to bind UDP socket: {e}"))?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;

    let identity = IdentityPacket {
        packet_type: "kdeconnect.identity".to_string(),
        body: IdentityBody {
            device_id: this_device_id(),
            device_name: this_device_name(),
            device_type: "desktop".to_string(),
            protocol_version: PROTOCOL_VERSION,
            tcp_port: PAIRING_TCP_PORT,
        },
    };
    let payload = serde_json::to_vec(&identity).map_err(|e| e.to_string())?;

    let broadcast_addr: SocketAddr = format!("255.255.255.255:{DISCOVERY_PORT}").parse().unwrap();
    socket.send_to(&payload, broadcast_addr).await.map_err(|e| format!("failed to send broadcast: {e}"))?;

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs.max(1));
    let mut buf = [0u8; 4096];
    let mut found = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, from))) => {
                if let Ok(packet) = serde_json::from_slice::<IdentityPacket>(&buf[..n]) {
                    if packet.packet_type != "kdeconnect.identity" || packet.body.device_id == this_device_id() {
                        continue; // ignore our own broadcast echoing back, and anything not an identity packet
                    }
                    let device = DiscoveredDevice {
                        id: packet.body.device_id.clone(),
                        name: packet.body.device_name,
                        device_type: DeviceType::from_str_loose(&packet.body.device_type),
                        address: from.ip().to_string(),
                        tcp_port: packet.body.tcp_port,
                        paired: with_known_devices(|d| d.get(&packet.body.device_id).map(|e| e.paired).unwrap_or(false)),
                        pinned_cert_sha256: with_known_devices(|d| d.get(&packet.body.device_id).and_then(|e| e.pinned_cert_sha256.clone())),
                    };
                    found.push(device);
                }
            }
            Ok(Err(_)) | Err(_) => break, // socket error or timeout elapsed
        }
    }

    with_known_devices(|devices| {
        for device in &found {
            devices.insert(device.id.clone(), device.clone());
        }
        save_devices(devices);
    });

    Ok(found)
}

/// Every device this install has ever discovered or paired with,
/// most-recently-discovered devices included — persisted across
/// restarts (see `devices.json` under `connect_dir()`), so a phone
/// that was paired yesterday still shows up (as paired, but possibly
/// offline) without needing a fresh discovery broadcast first.
#[tauri::command]
pub fn bc_get_devices() -> Vec<DiscoveredDevice> {
    with_known_devices(|devices| {
        let mut list: Vec<DiscoveredDevice> = devices.values().cloned().collect();
        list.sort_by(|a, b| b.paired.cmp(&a.paired).then(a.name.cmp(&b.name)));
        list
    })
}

#[tauri::command]
pub fn bc_forget_device(device_id: String) {
    with_known_devices(|devices| {
        devices.remove(&device_id);
        save_devices(devices);
    });
}

/// Opens a **TLS** connection (mutual: both sides present their
/// persistent self-signed certificate — see `tls.rs`) to `device_id`,
/// computes a Short Authentication String (SAS) from both sides'
/// certificate fingerprints, emits it to the frontend as
/// `blue-connect://pairing-sas` so the person can see/compare it, sends
/// the pairing request, and then **waits for the other device's own
/// person to explicitly accept or reject** (see `bc_listen_for_pairing`
/// / `bc_confirm_incoming_pairing`) before finalizing anything. Nothing
/// is marked `paired` or pinned unless that explicit acceptance comes
/// back — this is the fix for what used to be "trust-on-first-network-
/// use": a stranger's device sending the right bytes over the network
/// used to be sufficient; now a human has to actually say yes.
#[tauri::command]
pub async fn bc_request_pairing(app: AppHandle, device_id: String) -> Result<(), String> {
    let device = with_known_devices(|d| d.get(&device_id).cloned()).ok_or("Unknown device — run discovery first")?;
    let addr = format!("{}:{}", device.address, device.tcp_port);
    let tcp = TcpStream::connect(&addr).await.map_err(|e| format!("failed to connect to {addr}: {e}"))?;

    let captured = Arc::new(Mutex::new(None));
    let config = tls::client_config(captured.clone())?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let server_name = rustls::pki_types::ServerName::IpAddress(
        device.address.parse::<std::net::IpAddr>()
            .map_err(|_| format!("invalid device address: {}", device.address))?
            .into(),
    );
    let mut stream = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| format!("TLS handshake with {addr} failed: {e}"))?;

    let peer_fingerprint = captured.lock().unwrap().clone().ok_or("TLS handshake completed without a peer certificate")?;
    if device.paired {
        if let Some(pinned) = &device.pinned_cert_sha256 {
            if pinned != &peer_fingerprint {
                return Err(format!(
                    "Refusing to pair: {} presented a different certificate than the one pinned when it was last paired. \
                     This can mean the device was reset, or that something is impersonating it on the network — \
                     use \"Forget device\" and re-pair only if you're sure it's really {0}.",
                    device.name
                ));
            }
        }
    }

    let (my_cert, _) = tls::load_or_create_identity()?;
    let my_fingerprint = tls::fingerprint(&my_cert);
    let sas = tls::compute_sas(&my_fingerprint, &peer_fingerprint);
    let _ = app.emit("blue-connect://pairing-sas", serde_json::json!({
        "deviceId": device_id,
        "deviceName": device.name,
        "sas": sas,
        "role": "initiator",
    }));

    let pair_packet = serde_json::json!({
        "type": "kdeconnect.pair",
        "body": { "pair": true, "deviceId": this_device_id() },
    });
    let mut payload = serde_json::to_vec(&pair_packet).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    stream.write_all(&payload).await.map_err(|e| format!("failed to send pairing request: {e}"))?;
    stream.flush().await.map_err(|e| e.to_string())?;

    // Wait for the other side's own person to accept/reject (see
    // `bc_listen_for_pairing`) — up to 2 minutes, generous enough for a
    // person to actually notice a notification/dialog and respond, but
    // bounded so a device that never answers doesn't hang this command
    // forever.
    let mut buf = vec![0u8; 4096];
    let response = tokio::time::timeout(std::time::Duration::from_secs(120), stream.read(&mut buf)).await;
    let n = match response {
        Ok(Ok(0)) => return Err(format!("{} closed the connection without responding", device.name)),
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(format!("connection error while waiting for a response: {e}")),
        Err(_) => return Err(format!("{} did not respond to the pairing request in time", device.name)),
    };
    let json: serde_json::Value = serde_json::from_slice(&buf[..n]).map_err(|e| e.to_string())?;
    let accepted = json.get("body").and_then(|b| b.get("pair")).and_then(|v| v.as_bool()).unwrap_or(false);

    if !accepted {
        return Err(format!("Pairing was declined on {}", device.name));
    }

    with_known_devices(|devices| {
        if let Some(entry) = devices.get_mut(&device_id) {
            entry.paired = true;
            entry.pinned_cert_sha256 = Some(peer_fingerprint);
            save_devices(devices);
        }
    });

    Ok(())
}

/// Listens once for an incoming **TLS** pairing connection on
/// [`PAIRING_TCP_PORT`]. On receiving a request, computes the same SAS
/// [`bc_request_pairing`] computed (same two fingerprints, same
/// canonical ordering — see `tls.rs::compute_sas`) and emits it as
/// `blue-connect://pairing-request` for the frontend to show an
/// accept/reject dialog with that code, **then blocks waiting for
/// [`bc_confirm_incoming_pairing`]** to be called with the person's
/// decision before sending any response or touching the paired/pinned
/// state — replacing what used to be an unconditional auto-accept.
#[tauri::command]
pub async fn bc_listen_for_pairing(app: AppHandle, timeout_secs: u64) -> Result<Option<String>, String> {
    let listener = TcpListener::bind(("0.0.0.0", PAIRING_TCP_PORT))
        .await
        .map_err(|e| format!("failed to bind TCP {PAIRING_TCP_PORT}: {e}"))?;

    let accept_fut = listener.accept();
    let (tcp, peer_addr) = match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs.max(1)), accept_fut).await {
        Ok(Ok(pair)) => pair,
        Ok(Err(e)) => return Err(format!("accept failed: {e}")),
        Err(_) => return Ok(None), // timed out with no incoming connection — not an error
    };

    let captured = Arc::new(Mutex::new(None));
    let config = tls::server_config(captured.clone())?;
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let mut stream = acceptor
        .accept(tcp)
        .await
        .map_err(|e| format!("TLS handshake with {} failed: {e}", peer_addr.ip()))?;
    let peer_fingerprint = captured.lock().unwrap().clone().ok_or("TLS handshake completed without a peer certificate")?;

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_slice(&buf[..n]).map_err(|e| e.to_string())?;

    let device_id = json
        .get("body")
        .and_then(|b| b.get("deviceId"))
        .and_then(|v| v.as_str())
        .ok_or("pairing packet missing deviceId")?
        .to_string();

    // If this device was already paired under a different pinned
    // certificate, refuse rather than silently re-pinning — same
    // "something's wrong, don't just trust the new key" stance as the
    // client side takes in `bc_request_pairing`.
    let existing = with_known_devices(|d| d.get(&device_id).cloned());
    if let Some(existing) = &existing {
        if existing.paired {
            if let Some(pinned) = &existing.pinned_cert_sha256 {
                if pinned != &peer_fingerprint {
                    return Err(format!(
                        "Refusing to accept pairing from {}: it presented a different certificate than the one pinned last time. \
                         Use \"Forget device\" first if you're certain this is intentional (e.g. the device was reset).",
                        existing.name
                    ));
                }
            }
        }
    }

    let device_name = existing.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| format!("Device @ {}", peer_addr.ip()));

    let (my_cert, _) = tls::load_or_create_identity()?;
    let my_fingerprint = tls::fingerprint(&my_cert);
    let sas = tls::compute_sas(&my_fingerprint, &peer_fingerprint);

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    *pending_incoming_pairing().lock().unwrap() = Some(tx);
    let _ = app.emit("blue-connect://pairing-request", serde_json::json!({
        "deviceId": device_id,
        "deviceName": device_name,
        "sas": sas,
    }));

    // Wait up to 2 minutes for `bc_confirm_incoming_pairing` — same
    // budget as the initiator side waits, since both are waiting on the
    // same human action.
    let accepted = match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(decision)) => decision,
        Ok(Err(_)) => false, // sender dropped without deciding — treat as reject
        Err(_) => false,     // timed out waiting for a decision — treat as reject
    };
    *pending_incoming_pairing().lock().unwrap() = None;

    let response_packet = serde_json::json!({
        "type": "kdeconnect.pair",
        "body": { "pair": accepted },
    });
    let mut payload = serde_json::to_vec(&response_packet).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    let _ = stream.write_all(&payload).await;
    let _ = stream.flush().await;

    if !accepted {
        return Err(format!("Pairing request from {device_name} was declined or timed out"));
    }

    with_known_devices(|devices| {
        let entry = devices.entry(device_id.clone()).or_insert_with(|| DiscoveredDevice {
            id: device_id.clone(),
            name: device_name,
            device_type: DeviceType::Unknown,
            address: peer_addr.ip().to_string(),
            tcp_port: PAIRING_TCP_PORT,
            paired: false,
            pinned_cert_sha256: None,
        });
        entry.paired = true;
        entry.pinned_cert_sha256 = Some(peer_fingerprint);
        save_devices(devices);
    });

    Ok(Some(device_id))
}

/// Delivers the person's accept/reject decision for whichever pairing
/// request is currently waiting inside a running [`bc_listen_for_pairing`]
/// call (there can only be one at a time — see
/// [`PENDING_INCOMING_PAIRING`]). Returns an error if nothing is
/// actually pending, e.g. the dialog was shown but the request already
/// timed out before the person clicked anything.
#[tauri::command]
pub fn bc_confirm_incoming_pairing(accept: bool) -> Result<(), String> {
    let sender = pending_incoming_pairing().lock().unwrap().take();
    match sender {
        Some(tx) => tx.send(accept).map_err(|_| "The pairing request is no longer waiting (it may have already timed out)".to_string()),
        None => Err("No pairing request is currently waiting for a decision".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_packet_round_trips_through_json_with_kdeconnect_field_names() {
        let packet = IdentityPacket {
            packet_type: "kdeconnect.identity".to_string(),
            body: IdentityBody {
                device_id: "abc123".to_string(),
                device_name: "Test Desktop".to_string(),
                device_type: "desktop".to_string(),
                protocol_version: PROTOCOL_VERSION,
                tcp_port: PAIRING_TCP_PORT,
            },
        };
        let json = serde_json::to_value(&packet).unwrap();
        // Field names must match KDE Connect's real wire format exactly
        // (camelCase, not snake_case) — this is what lets a real KDE
        // Connect/GSConnect install parse a packet we send, and vice
        // versa.
        assert_eq!(json["body"]["deviceId"], "abc123");
        assert_eq!(json["body"]["deviceName"], "Test Desktop");
        assert_eq!(json["body"]["tcpPort"], PAIRING_TCP_PORT);

        let back: IdentityPacket = serde_json::from_value(json).unwrap();
        assert_eq!(back.body.device_id, "abc123");
    }

    #[test]
    fn device_type_parses_known_kdeconnect_type_strings() {
        assert_eq!(DeviceType::from_str_loose("phone"), DeviceType::Phone);
        assert_eq!(DeviceType::from_str_loose("desktop"), DeviceType::Desktop);
        assert_eq!(DeviceType::from_str_loose("something-unexpected"), DeviceType::Unknown);
    }

    #[test]
    fn discovery_port_matches_kdeconnects_real_port() {
        // Not an arbitrary choice — this is the actual UDP port real
        // KDE Connect/GSConnect installs broadcast identity packets on.
        assert_eq!(DISCOVERY_PORT, 1716);
    }
}
