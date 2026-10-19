use super::phone::run_timeout;
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KdeDevice {
    pub id: String,
    pub name: String,
    pub paired: bool,
    pub reachable: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KdeStatus {
    pub installed: bool,
    pub devices: Vec<KdeDevice>,
}

pub fn valid_device_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Parses `kdeconnect-cli -l`:
/// `- Pixel 7: 1234abcd_5678 (paired and reachable)`.
pub fn parse_list(out: &str) -> Vec<KdeDevice> {
    out.lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("- ")?;
            // the status is the last parenthesised group, the id the last `: ` field before it
            let open = rest.rfind('(')?;
            let status = rest[open + 1..].trim_end_matches(')').to_lowercase();
            let head = rest[..open].trim_end();
            let (name, id) = head.rsplit_once(": ")?;
            let id = id.trim().to_string();
            if !valid_device_id(&id) {
                return None;
            }
            Some(KdeDevice { id, name: name.trim().to_string(), paired: status.contains("paired") && !status.contains("not paired"), reachable: status.contains("reachable") })
        })
        .collect()
}

const T: Duration = Duration::from_secs(15);

fn cli(args: &[&str]) -> Result<String, String> {
    let o = run_timeout("kdeconnect-cli", args, T)?;
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    if o.ok {
        Ok(text)
    } else {
        Err(o.stderr.lines().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("kdeconnect-cli failed").to_string())
    }
}

fn with_device(id: &str, extra: &[&str]) -> Result<String, String> {
    if !valid_device_id(id) {
        return Err("Invalid device id".into());
    }
    let mut a = vec!["-d", id];
    a.extend_from_slice(extra);
    cli(&a)
}

#[tauri::command(async)]
pub fn kde_status() -> KdeStatus {
    match cli(&["--refresh"]).and_then(|_| cli(&["-l"])) {
        Ok(o) => KdeStatus { installed: true, devices: parse_list(&o) },
        Err(e) => KdeStatus { installed: !e.contains("not installed"), devices: vec![] },
    }
}

#[tauri::command(async)]
pub fn kde_pair(id: String) -> Result<(), String> {
    with_device(&id, &["--pair"]).map(|_| ())
}

#[tauri::command(async)]
pub fn kde_unpair(id: String) -> Result<(), String> {
    with_device(&id, &["--unpair"]).map(|_| ())
}

#[tauri::command(async)]
pub fn kde_ping(id: String) -> Result<(), String> {
    with_device(&id, &["--ping-msg", "Blue Environment"]).map(|_| ())
}

/// "Find my phone".
#[tauri::command(async)]
pub fn kde_ring(id: String) -> Result<(), String> {
    with_device(&id, &["--ring"]).map(|_| ())
}

#[tauri::command(async)]
pub fn kde_share_file(id: String, path: String) -> Result<(), String> {
    if !std::path::Path::new(&path).exists() {
        return Err("The file does not exist".into());
    }
    with_device(&id, &["--share", &path]).map(|_| ())
}

/// Shares plain text or a link (KDE Connect opens URLs in the phone's browser).
#[tauri::command(async)]
pub fn kde_share_text(id: String, text: String) -> Result<(), String> {
    if text.trim().is_empty() || text.len() > 4000 {
        return Err("Enter some text (up to 4000 characters)".into());
    }
    with_device(&id, &["--share-text", &text]).map(|_| ())
}

#[tauri::command(async)]
pub fn kde_send_sms(id: String, destination: String, message: String) -> Result<(), String> {
    if !super::phone::valid_phone_number(&destination) {
        return Err("That does not look like a phone number".into());
    }
    if message.trim().is_empty() || message.len() > 1000 {
        return Err("Enter a message (up to 1000 characters)".into());
    }
    with_device(&id, &["--destination", destination.trim(), "--send-sms", &message]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_list() {
        let out = "- Pixel 7: 1234abcd_5678 (paired and reachable)\n- Old phone: ffff0000 (paired)\n- Annie's: tablet: aaaa1111 (reachable)\n- Weird: bad id (paired)\n2 devices found\n";
        let d = parse_list(out);
        assert_eq!(d.len(), 3);
        assert_eq!(d[0], KdeDevice { id: "1234abcd_5678".into(), name: "Pixel 7".into(), paired: true, reachable: true });
        assert!(d[1].paired && !d[1].reachable);
        assert!(!d[2].paired && d[2].reachable);
        assert_eq!(d[2].name, "Annie's: tablet");
        assert!(parse_list("No devices found\n").is_empty());
    }

    #[test]
    fn validates_ids() {
        assert!(valid_device_id("abc_DEF-123") && !valid_device_id("a b") && !valid_device_id("a;b") && !valid_device_id(""));
    }
}
