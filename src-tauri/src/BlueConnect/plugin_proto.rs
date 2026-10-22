use serde::Serialize;
use serde_json::{json, Value};

pub const TYPE_BATTERY_REQUEST: &str = "kdeconnect.battery.request";
pub const TYPE_BATTERY: &str = "kdeconnect.battery";
pub const TYPE_FIND_MY_PHONE: &str = "kdeconnect.findmyphone.request";
pub const TYPE_CLIPBOARD: &str = "kdeconnect.clipboard";
pub const TYPE_CLIPBOARD_REQUEST: &str = "kdeconnect.clipboard.request";
pub const TYPE_SHARE: &str = "kdeconnect.share.request";
pub const TYPE_NOTIFICATION_REQUEST: &str = "kdeconnect.notification.request";
pub const TYPE_NOTIFICATION_LIST: &str = "kdeconnect.notification.list";

pub const MAX_CLIPBOARD_BYTES: usize = 256 * 1024;
pub const MAX_SHARED_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_URL_LENGTH: usize = 2048;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryInfo {
    pub percent: u8,
    pub charging: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationInfo {
    pub id: String,
    pub app_name: String,
    pub title: String,
    pub text: String,
    pub time: i64,
}

/// One framed packet: compact JSON + `\n`.
pub fn frame(packet_type: &str, body: Value) -> Vec<u8> {
    let mut v = serde_json::to_vec(&json!({ "type": packet_type, "body": body })).unwrap_or_default();
    v.push(b'\n');
    v
}

/// Truncates to at most `max_bytes` of UTF-8 without cutting a character in half.
pub fn fit_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Only plain web / mail / phone links may be opened on request of this computer.
pub fn is_safe_url(url: &str) -> bool {
    if url.is_empty() || url.chars().count() > MAX_URL_LENGTH {
        return false;
    }
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once(':') else { return false };
    if rest.is_empty() || scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return false;
    }
    matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https" | "mailto" | "tel")
}

pub fn battery_request() -> Vec<u8> {
    frame(TYPE_BATTERY_REQUEST, json!({ "request": true }))
}
pub fn find_my_phone() -> Vec<u8> {
    frame(TYPE_FIND_MY_PHONE, json!({}))
}
pub fn clipboard_request() -> Vec<u8> {
    frame(TYPE_CLIPBOARD_REQUEST, json!({}))
}
pub fn notification_request() -> Vec<u8> {
    frame(TYPE_NOTIFICATION_REQUEST, json!({ "request": true }))
}
pub fn clipboard(content: &str) -> Vec<u8> {
    frame(TYPE_CLIPBOARD, json!({ "content": fit_utf8(content, MAX_CLIPBOARD_BYTES) }))
}
/// A safe URL is sent as `url` (opened on the other device), anything else as plain `text`.
pub fn share(text_or_url: &str) -> Vec<u8> {
    if is_safe_url(text_or_url) {
        frame(TYPE_SHARE, json!({ "url": text_or_url }))
    } else {
        frame(TYPE_SHARE, json!({ "text": fit_utf8(text_or_url, MAX_SHARED_TEXT_BYTES) }))
    }
}

/// Parses the first line of `buf` as a packet and checks its type.
pub fn parse_reply(buf: &[u8], expected_type: &str) -> Result<Value, String> {
    let end = buf.iter().position(|b| *b == b'\n').unwrap_or(buf.len());
    let v: Value = serde_json::from_slice(&buf[..end]).map_err(|e| format!("malformed reply: {e}"))?;
    match v.get("type").and_then(|t| t.as_str()) {
        Some(t) if t == expected_type => Ok(v),
        Some(other) => Err(format!("unexpected reply type: {other}")),
        None => Err("reply has no type".into()),
    }
}

/// `currentCharge < 0` means the other device has no battery.
pub fn parse_battery(packet: &Value) -> Option<BatteryInfo> {
    let body = packet.get("body")?;
    let charge = body.get("currentCharge")?.as_i64()?;
    if charge < 0 {
        return None;
    }
    Some(BatteryInfo { percent: charge.min(100) as u8, charging: body.get("isCharging").and_then(|c| c.as_bool()).unwrap_or(false) })
}

pub fn parse_clipboard(packet: &Value) -> String {
    packet.get("body").and_then(|b| b.get("content")).and_then(|c| c.as_str()).unwrap_or("").to_string()
}

pub fn parse_notifications(packet: &Value) -> Vec<NotificationInfo> {
    let Some(arr) = packet.get("body").and_then(|b| b.get("notifications")).and_then(|n| n.as_array()) else { return Vec::new() };
    let s = |o: &Value, k: &str| o.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    arr.iter()
        .filter(|o| o.is_object())
        .map(|o| NotificationInfo {
            id: o.get("id").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default(),
            app_name: s(o, "appName"),
            title: s(o, "title"),
            text: s(o, "text"),
            time: o.get("time").and_then(|t| t.as_i64()).unwrap_or(0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(frame: &[u8]) -> Value {
        serde_json::from_slice(&frame[..frame.len() - 1]).unwrap()
    }

    #[test]
    fn frames_end_with_newline_and_carry_type() {
        let f = battery_request();
        assert_eq!(*f.last().unwrap(), b'\n');
        assert_eq!(parsed(&f)["type"], TYPE_BATTERY_REQUEST);
        assert_eq!(parsed(&f)["body"]["request"], true);
    }

    #[test]
    fn url_safety() {
        assert!(is_safe_url("https://example.org/a?b=c"));
        assert!(is_safe_url("mailto:a@b.c"));
        assert!(is_safe_url("tel:+48123456789"));
        assert!(!is_safe_url("javascript:alert(1)"));
        assert!(!is_safe_url("file:///etc/passwd"));
        assert!(!is_safe_url("ms-settings:"));
        assert!(!is_safe_url("smb://x/y"));
        assert!(!is_safe_url("https://a b"));
        assert!(!is_safe_url("https://x/\n"));
        assert!(!is_safe_url(""));
        assert!(!is_safe_url("nocolon"));
        assert!(!is_safe_url("https:"));
        assert!(!is_safe_url(&format!("https://x/{}", "a".repeat(3000))));
    }

    #[test]
    fn share_picks_url_or_text() {
        assert_eq!(parsed(&share("https://example.org"))["body"]["url"], "https://example.org");
        let t = parsed(&share("javascript:alert(1)"));
        assert_eq!(t["body"]["text"], "javascript:alert(1)");
        assert!(t["body"].get("url").is_none());
    }

    #[test]
    fn fit_utf8_keeps_whole_characters() {
        assert_eq!(fit_utf8("ą-----------", 2), "ą");
        assert_eq!(fit_utf8("🙂🙂", 5), "🙂");
        assert_eq!(fit_utf8("short", 100), "short");
        assert_eq!(fit_utf8("ą", 1), "");
    }

    #[test]
    fn clipboard_is_capped() {
        let big = "x".repeat(MAX_CLIPBOARD_BYTES + 100);
        let f = parsed(&clipboard(&big));
        assert_eq!(f["body"]["content"].as_str().unwrap().len(), MAX_CLIPBOARD_BYTES);
    }

    #[test]
    fn parses_battery_replies() {
        let ok = json!({ "type": TYPE_BATTERY, "body": { "currentCharge": 73, "isCharging": true } });
        assert_eq!(parse_battery(&ok), Some(BatteryInfo { percent: 73, charging: true }));
        assert_eq!(parse_battery(&json!({ "body": { "currentCharge": -1 } })), None);
        assert_eq!(parse_battery(&json!({ "body": { "currentCharge": 250 } })).unwrap().percent, 100);
        assert_eq!(parse_battery(&json!({})), None);
    }

    #[test]
    fn parse_reply_checks_type_and_takes_first_line() {
        let buf = b"{\"type\":\"kdeconnect.clipboard\",\"body\":{\"content\":\"hi\"}}\n{\"type\":\"x\"}\n";
        let v = parse_reply(buf, TYPE_CLIPBOARD).unwrap();
        assert_eq!(parse_clipboard(&v), "hi");
        assert!(parse_reply(buf, TYPE_BATTERY).unwrap_err().contains("unexpected reply type"));
        assert!(parse_reply(b"not json", TYPE_BATTERY).is_err());
        assert!(parse_reply(b"{}", TYPE_BATTERY).is_err());
    }

    #[test]
    fn parses_notification_lists() {
        let v = json!({ "type": TYPE_NOTIFICATION_LIST, "body": { "notifications": [
            { "id": "a", "appName": "Mail", "title": "Hi", "text": "Body", "time": 5 },
            { "id": 7, "appName": "Chat" },
            "junk"
        ] } });
        let n = parse_notifications(&v);
        assert_eq!(n.len(), 2);
        assert_eq!(n[0], NotificationInfo { id: "a".into(), app_name: "Mail".into(), title: "Hi".into(), text: "Body".into(), time: 5 });
        assert_eq!(n[1].id, "7");
        assert!(parse_notifications(&json!({})).is_empty());
    }
}
