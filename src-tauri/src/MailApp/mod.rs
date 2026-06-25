use serde::{Deserialize, Serialize};
use std::process::Command;
use std::fs;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MailAccount {
    pub id: String,
    pub name: String,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub use_ssl: bool,
}

#[derive(Serialize, Clone)]
pub struct RemoteEmail {
    pub uid: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub date: String,
    pub body: String,
    pub read: bool,
}

#[derive(Serialize, Clone)]
pub struct SendResult { pub success: bool, pub error: Option<String> }

fn config_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("Blue-Environment")
        .join("mail")
}

/// Returns stored mail accounts (passwords are not serialised in responses).
#[tauri::command]
pub fn mail_get_accounts() -> Vec<MailAccount> {
    let path = config_dir().join("accounts.json");
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// Saves an account (overwrites by id if it already exists).
#[tauri::command]
pub fn mail_save_account(account: MailAccount) -> bool {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_err() { return false; }
    let path = dir.join("accounts.json");
    let mut accounts: Vec<MailAccount> = fs::read_to_string(&path)
        .ok()
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    accounts.retain(|a| a.id != account.id);
    accounts.push(account);
    fs::write(path, serde_json::to_string_pretty(&accounts).unwrap_or_default()).is_ok()
}

/// Deletes an account by id.
#[tauri::command]
pub fn mail_delete_account(account_id: String) -> bool {
    let path = config_dir().join("accounts.json");
    let mut accounts: Vec<MailAccount> = fs::read_to_string(&path)
        .ok()
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    let before = accounts.len();
    accounts.retain(|a| a.id != account_id);
    if accounts.len() == before { return false; }
    fs::write(path, serde_json::to_string_pretty(&accounts).unwrap_or_default()).is_ok()
}

/// Fetches recent messages from an IMAP mailbox using curl.
/// Returns an empty list (not an error) when curl or credentials are unavailable
/// so the frontend stays in its graceful demo-data mode.
#[tauri::command]
pub fn mail_fetch_inbox(account_id: String, folder: Option<String>, limit: Option<u32>) -> Vec<RemoteEmail> {
    let accounts: Vec<MailAccount> = fs::read_to_string(config_dir().join("accounts.json"))
        .ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    let Some(acc) = accounts.iter().find(|a| a.id == account_id) else { return vec![]; };

    let scheme = if acc.use_ssl { "imaps" } else { "imap" };
    let mbox   = folder.as_deref().unwrap_or("INBOX");
    let n      = limit.unwrap_or(20);
    let url    = format!("{}://{}:{}/{}", scheme, acc.imap_host, acc.imap_port, mbox);

    // curl --list-only fetches UIDs; real message bodies need separate fetch calls.
    // For now we fetch the combined FETCH overview (RFC822.HEADER) for the last N.
    let output = Command::new("curl")
        .args([
            "--silent", "--ssl-reqd",
            "-u", &format!("{}:{}", acc.username, acc.password),
            "--url", &url,
            "-X", &format!("FETCH 1:{n} (RFC822.HEADER UID FLAGS)"),
        ])
        .output();

    let Ok(out) = output else { return vec![]; };
    if !out.status.success() { return vec![]; }

    // Parse raw IMAP FETCH response lines into simple structs.
    parse_imap_fetch_response(&String::from_utf8_lossy(&out.stdout))
}

fn parse_imap_fetch_response(raw: &str) -> Vec<RemoteEmail> {
    let mut results = Vec::new();
    let mut uid = String::new();
    let mut from = String::new();
    let mut to = String::new();
    let mut subject = String::new();
    let mut date = String::new();
    let mut seen = false;

    for line in raw.lines() {
        if line.starts_with("* ") && line.contains("FETCH") {
            // Commit previous
            if !uid.is_empty() {
                results.push(RemoteEmail { uid: uid.clone(), from: from.clone(), to: to.clone(), subject: subject.clone(), date: date.clone(), body: String::new(), read: seen });
            }
            uid = line.split_whitespace().nth(1).unwrap_or("0").to_string();
            from.clear(); to.clear(); subject.clear(); date.clear(); seen = false;
        } else if let Some(v) = line.strip_prefix("From: ") {
            from = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("To: ") {
            to = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Subject: ") {
            subject = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Date: ") {
            date = v.trim().to_string();
        } else if line.contains("\\Seen") {
            seen = true;
        }
    }
    if !uid.is_empty() {
        results.push(RemoteEmail { uid, from, to, subject, date, body: String::new(), read: seen });
    }
    results
}

/// Sends an email via msmtp (a lightweight, widely-available SMTP client).
/// Falls back to curl --smtp if msmtp is not installed.
#[tauri::command]
pub fn mail_send(
    account_id: String,
    to: String,
    cc: Option<String>,
    subject: String,
    body: String,
) -> SendResult {
    let accounts: Vec<MailAccount> = fs::read_to_string(config_dir().join("accounts.json"))
        .ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    let Some(acc) = accounts.iter().find(|a| a.id == account_id) else {
        return SendResult { success: false, error: Some("Account not found".to_string()) };
    };

    // Build RFC 2822 message
    let cc_header = cc.as_deref().filter(|s| !s.is_empty())
        .map(|c| format!("Cc: {}\r\n", c)).unwrap_or_default();
    let message = format!(
        "From: {} <{}>\r\nTo: {}\r\n{}Subject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
        acc.name, acc.email, to, cc_header, subject, body
    );

    // Try msmtp first (most common on modern Fedora/Ubuntu desktops)
    if which_exists("msmtp") {
        let conf_dir = config_dir();
        let _ = fs::create_dir_all(&conf_dir);
        let cfg_path = conf_dir.join("msmtp.conf");
        let tls_val = if acc.use_ssl { "on" } else { "off" };
        let cfg = format!(
            "account blue\nhost {}\nport {}\nauth on\nuser {}\npassword {}\ntls {}\ntls_starttls {}\nfrom {}\n",
            acc.smtp_host, acc.smtp_port, acc.username, acc.password,
            if acc.smtp_port == 465 { "on" } else { tls_val },
            if acc.smtp_port == 587 { "on" } else { "off" },
            acc.email,
        );
        if fs::write(&cfg_path, &cfg).is_ok() {
            let output = Command::new("msmtp")
                .args(["--file", cfg_path.to_str().unwrap_or(""), "--account=blue", "-t"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    child.stdin.take().map(|mut s| { let _ = s.write_all(message.as_bytes()); });
                    child.wait()
                });
            fs::remove_file(&cfg_path).ok();
            return match output {
                Ok(s) if s.success() => SendResult { success: true, error: None },
                Ok(s) => SendResult { success: false, error: Some(format!("msmtp exited {}", s.code().unwrap_or(-1))) },
                Err(e) => SendResult { success: false, error: Some(e.to_string()) },
            };
        }
    }

    // Fallback: curl --smtp
    if which_exists("curl") {
        let scheme = if acc.smtp_port == 465 { "smtps" } else { "smtp" };
        let url    = format!("{}://{}:{}", scheme, acc.smtp_host, acc.smtp_port);
        let output = Command::new("curl")
            .args([
                "--silent", "--ssl-reqd",
                "-u", &format!("{}:{}", acc.username, acc.password),
                "--url", &url,
                "--mail-from", &acc.email,
                "--mail-rcpt", &to,
                "--upload-file", "-",
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.take().map(|mut s| { let _ = s.write_all(message.as_bytes()); });
                child.wait()
            });
        return match output {
            Ok(s) if s.success() => SendResult { success: true, error: None },
            Ok(_) => SendResult { success: false, error: Some("curl SMTP failed".to_string()) },
            Err(e) => SendResult { success: false, error: Some(e.to_string()) },
        };
    }

    SendResult { success: false, error: Some("No mail sender available (install msmtp or curl)".to_string()) }
}

fn which_exists(cmd: &str) -> bool {
    Command::new("which").arg(cmd).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Marks a message as read/unread via IMAP STORE.
#[tauri::command]
pub fn mail_mark_read(account_id: String, uid: String, read: bool) -> bool {
    let accounts: Vec<MailAccount> = std::fs::read_to_string(config_dir().join("accounts.json"))
        .ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    let Some(acc) = accounts.iter().find(|a| a.id == account_id) else { return false; };
    let scheme = if acc.use_ssl { "imaps" } else { "imap" };
    let url    = format!("{}://{}:{}/INBOX", scheme, acc.imap_host, acc.imap_port);
    let cmd    = if read { format!("UID STORE {} +FLAGS (\\Seen)", uid) } else { format!("UID STORE {} -FLAGS (\\Seen)", uid) };
    Command::new("curl")
        .args(["--silent", "--ssl-reqd", "-u", &format!("{}:{}", acc.username, acc.password), "--url", &url, "-X", &cmd])
        .status().map(|s| s.success()).unwrap_or(false)
}

/// Moves a message to a different IMAP folder using COPY + STORE \\Deleted + EXPUNGE.
#[tauri::command]
pub fn mail_move_message(account_id: String, uid: String, dest_folder: String) -> bool {
    let accounts: Vec<MailAccount> = std::fs::read_to_string(config_dir().join("accounts.json"))
        .ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    let Some(acc) = accounts.iter().find(|a| a.id == account_id) else { return false; };
    let scheme = if acc.use_ssl { "imaps" } else { "imap" };
    let base   = format!("{}://{}:{}/INBOX", scheme, acc.imap_host, acc.imap_port);
    let creds  = format!("{}:{}", acc.username, acc.password);
    let copy   = Command::new("curl").args(["--silent", "--ssl-reqd", "-u", &creds, "--url", &base, "-X", &format!("UID COPY {} {}", uid, dest_folder)]).status();
    if !copy.map(|s| s.success()).unwrap_or(false) { return false; }
    let _ = Command::new("curl").args(["--silent", "--ssl-reqd", "-u", &creds, "--url", &base, "-X", &format!("UID STORE {} +FLAGS (\\Deleted)", uid)]).status();
    Command::new("curl").args(["--silent", "--ssl-reqd", "-u", &creds, "--url", &base, "-X", "EXPUNGE"]).status().map(|s| s.success()).unwrap_or(false)
}
