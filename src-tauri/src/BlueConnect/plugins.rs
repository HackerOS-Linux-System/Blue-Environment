use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::plugin_proto as proto;
use super::{get_paired_device, open_authenticated_connection};

const REPLY_TIMEOUT: Duration = Duration::from_secs(8);

/// Sends one packet to a paired device; when `expect` is set, waits for one reply of that type.
async fn request(device_id: &str, packet: Vec<u8>, expect: Option<&str>) -> Result<Option<serde_json::Value>, String> {
    let handle = get_paired_device(device_id).ok_or("That device is no longer paired — re-pair it in Blue Connect and try again")?;
    let mut stream = open_authenticated_connection(&handle).await?;
    stream.write_all(&packet).await.map_err(|e| format!("failed to send to {}: {e}", handle.device_name))?;
    stream.flush().await.map_err(|e| e.to_string())?;

    let Some(expected) = expect else { return Ok(None) };
    let mut buf = vec![0u8; 65536];
    let n = match tokio::time::timeout(REPLY_TIMEOUT, stream.read(&mut buf)).await {
        Ok(Ok(0)) => return Err(format!("{} closed the connection without replying", handle.device_name)),
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(format!("connection error while waiting for {}: {e}", handle.device_name)),
        Err(_) => return Err(format!("{} did not reply in time", handle.device_name)),
    };
    proto::parse_reply(&buf[..n], expected).map(Some)
}

/// Battery of the other device; `None` when it has no battery.
#[tauri::command]
pub async fn bc_device_battery(device_id: String) -> Result<Option<proto::BatteryInfo>, String> {
    let reply = request(&device_id, proto::battery_request(), Some(proto::TYPE_BATTERY)).await?;
    Ok(reply.as_ref().and_then(proto::parse_battery))
}

/// Makes the other device ring ("Find my phone").
#[tauri::command]
pub async fn bc_find_device(device_id: String) -> Result<(), String> {
    request(&device_id, proto::find_my_phone(), None).await.map(|_| ())
}

/// Puts `text` on the other device's clipboard.
#[tauri::command]
pub async fn bc_send_clipboard(device_id: String, text: String) -> Result<(), String> {
    request(&device_id, proto::clipboard(&text), None).await.map(|_| ())
}

/// Reads the other device's clipboard text (Android may answer empty while the app is in the background).
#[tauri::command]
pub async fn bc_get_device_clipboard(device_id: String) -> Result<String, String> {
    let reply = request(&device_id, proto::clipboard_request(), Some(proto::TYPE_CLIPBOARD)).await?;
    Ok(reply.as_ref().map(proto::parse_clipboard).unwrap_or_default())
}

/// Pushes a link (opened — or offered as a tap-to-open notification — on the other device) or plain text.
#[tauri::command]
pub async fn bc_share(device_id: String, text_or_url: String) -> Result<(), String> {
    if text_or_url.trim().is_empty() {
        return Err("Nothing to send".into());
    }
    request(&device_id, proto::share(text_or_url.trim()), None).await.map(|_| ())
}

/// The other device's current notifications (Android: needs "Notification access" enabled there).
#[tauri::command]
pub async fn bc_device_notifications(device_id: String) -> Result<Vec<proto::NotificationInfo>, String> {
    let reply = request(&device_id, proto::notification_request(), Some(proto::TYPE_NOTIFICATION_LIST)).await?;
    Ok(reply.as_ref().map(proto::parse_notifications).unwrap_or_default())
}
