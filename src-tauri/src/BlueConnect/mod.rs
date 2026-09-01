use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// KDE Connect's real discovery port — deliberately the same value so
/// this can at least *see* real KDE Connect/GSConnect devices on the
/// same LAN (see module doc's "What's not real yet" for why seeing
/// isn't the same as safely pairing with them).
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
}

/// The identity packet's shape on the wire — matches KDE Connect's
/// real `kdeconnect.identity` body fields closely enough to parse
/// theirs (`deviceId`/`deviceName`/`deviceType`/`tcpPort` all match
/// their naming), while only ever sending the subset this
/// implementation actually backs (see module doc: no TLS certificate
/// field, since there's no real cert here to advertise honestly).
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

/// Opens a plaintext TCP connection to `device_id` and sends the
/// pairing-request packet — see module doc's "What's not real yet" for
/// why this is a real TCP handshake with no TLS/certificate step, and
/// therefore only meaningfully pairs with another Blue Connect
/// instance rather than a real KDE Connect/GSConnect install.
#[tauri::command]
pub async fn bc_request_pairing(device_id: String) -> Result<(), String> {
    let device = with_known_devices(|d| d.get(&device_id).cloned()).ok_or("Unknown device — run discovery first")?;
    let addr = format!("{}:{}", device.address, device.tcp_port);
    let mut stream = TcpStream::connect(&addr).await.map_err(|e| format!("failed to connect to {addr}: {e}"))?;

    let pair_packet = serde_json::json!({
        "type": "kdeconnect.pair",
        "body": { "pair": true, "deviceId": this_device_id() },
    });
    let mut payload = serde_json::to_vec(&pair_packet).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    stream.write_all(&payload).await.map_err(|e| format!("failed to send pairing request: {e}"))?;

    Ok(())
}

/// Listens once for an incoming pairing request on
/// [`PAIRING_TCP_PORT`] and, if one arrives within `timeout_secs`,
/// automatically accepts it (marks that device paired). A real
/// implementation would surface this to the person for explicit
/// accept/reject (KDE Connect shows a notification) — auto-accepting
/// is a deliberate simplification for this pass, not something to
/// treat as secure trust establishment (see module doc — there's no
/// certificate verification happening regardless of who clicks
/// accept).
#[tauri::command]
pub async fn bc_listen_for_pairing(timeout_secs: u64) -> Result<Option<String>, String> {
    let listener = TcpListener::bind(("0.0.0.0", PAIRING_TCP_PORT))
        .await
        .map_err(|e| format!("failed to bind TCP {PAIRING_TCP_PORT}: {e}"))?;

    let accept_fut = listener.accept();
    let (mut stream, peer_addr) = match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs.max(1)), accept_fut).await {
        Ok(Ok(pair)) => pair,
        Ok(Err(e)) => return Err(format!("accept failed: {e}")),
        Err(_) => return Ok(None), // timed out with no incoming connection — not an error
    };

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_slice(&buf[..n]).map_err(|e| e.to_string())?;

    let device_id = json
        .get("body")
        .and_then(|b| b.get("deviceId"))
        .and_then(|v| v.as_str())
        .ok_or("pairing packet missing deviceId")?
        .to_string();

    with_known_devices(|devices| {
        let entry = devices.entry(device_id.clone()).or_insert_with(|| DiscoveredDevice {
            id: device_id.clone(),
            name: format!("Device @ {}", peer_addr.ip()),
            device_type: DeviceType::Unknown,
            address: peer_addr.ip().to_string(),
            tcp_port: PAIRING_TCP_PORT,
            paired: false,
        });
        entry.paired = true;
        save_devices(devices);
    });

    Ok(Some(device_id))
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
