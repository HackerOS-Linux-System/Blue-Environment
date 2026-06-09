use crate::{session, DaemonState, SOCKET_PATH};
use serde::{Deserialize, Serialize};
use std::{os::unix::fs::PermissionsExt, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Mutex,
};
use tracing::{error, info, warn};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum GreeterRequest {
    Authenticate { username: String, password: String },
    StartSession  { username: String, session: String, env: Option<Vec<(String, String)>> },
    GetSessions,
    GetUsers,
    GetInfo,
    PowerAction   { action: String },
    Cancel,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    AuthSuccess   { username: String },
    AuthFailure   { reason: String, attempts_left: u8 },
    SessionStarted{ session_id: String },
    SessionError  { reason: String },
    Sessions      { sessions: Vec<SessionInfo> },
    Users         { users: Vec<UserInfo> },
    Info          { version: String, hostname: String, uptime: u64, os_name: String, os_version: String },
    PowerResult   { success: bool, message: String },
    Error         { message: String },
    Bye,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub session_type: String,
    pub desktop_names: Vec<String>,
    pub icon: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserInfo {
    pub username: String,
    pub realname: String,
    pub uid: u32,
    pub home: String,
    pub shell: String,
    pub icon_path: Option<String>,
    pub last_session: Option<String>,
}

pub async fn run_server(state: Arc<Mutex<DaemonState>>) {
    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => { error!("Failed to bind BEDM socket at {}: {}", SOCKET_PATH, e); return; }
    };

    std::fs::set_permissions(SOCKET_PATH, std::fs::Permissions::from_mode(0o666)).ok();
    info!("BEDM IPC listening on {}", SOCKET_PATH);

    tokio::spawn(async {
        let mut term = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()).unwrap();
        term.recv().await;
        info!("SIGTERM received — shutting down BEDM");
        let _ = std::fs::remove_file(SOCKET_PATH);
        std::process::exit(0);
    });

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                info!("Greeter client connected");
                let state_clone = state.clone();
                tokio::spawn(async move { handle_client(stream, state_clone).await; });
            }
            Err(e) => { error!("Accept error: {}", e); }
        }
    }
}

async fn handle_client(mut stream: UnixStream, state: Arc<Mutex<DaemonState>>) {
    let (reader, mut writer) = stream.split();
    let mut lines = BufReader::new(reader).lines();
    let mut auth_user: Option<String> = None;
    let mut fail_count: u8 = 0;

    // Send welcome Info on connect
    send_response(&mut writer, &build_info_response().await).await;

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let request: GreeterRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                send_response(&mut writer, &DaemonResponse::Error {
                    message: format!("Parse error: {}", e) }).await;
                continue;
            }
        };

        match request {
            GreeterRequest::GetInfo => {
                send_response(&mut writer, &build_info_response().await).await;
            }
            GreeterRequest::GetSessions => {
                let sessions = crate::session::list_sessions(&state).await;
                send_response(&mut writer, &DaemonResponse::Sessions { sessions }).await;
            }
            GreeterRequest::GetUsers => {
                let users = crate::users::list_users(&state).await;
                send_response(&mut writer, &DaemonResponse::Users { users }).await;
            }
            GreeterRequest::Authenticate { username, password } => {
                if fail_count >= 5 {
                    send_response(&mut writer, &DaemonResponse::AuthFailure {
                        reason: "Too many failed attempts".to_string(),
                        attempts_left: 0 }).await;
                    break;
                }
                let allow_root = state.lock().await.config.allow_root.unwrap_or(false);
                if username == "root" && !allow_root {
                    fail_count += 1;
                    send_response(&mut writer, &DaemonResponse::AuthFailure {
                        reason: "Root login not permitted".to_string(),
                        attempts_left: 5 - fail_count }).await;
                    continue;
                }
                info!("Auth attempt for: {}", username);
                match crate::pam_auth::authenticate(&username, &password) {
                    Ok(()) => {
                        info!("Auth success: {}", username);
                        auth_user = Some(username.clone());
                        fail_count = 0;
                        send_response(&mut writer, &DaemonResponse::AuthSuccess { username }).await;
                    }
                    Err(reason) => {
                        warn!("Auth failure for {}: {}", username, reason);
                        fail_count += 1;
                        send_response(&mut writer, &DaemonResponse::AuthFailure {
                            reason,
                            attempts_left: 5u8.saturating_sub(fail_count) }).await;
                    }
                }
            }
            GreeterRequest::StartSession { username, session, env } => {
                let authed = auth_user.as_deref() == Some(username.as_str());
                if !authed {
                    send_response(&mut writer, &DaemonResponse::Error {
                        message: "Not authenticated".to_string() }).await;
                    continue;
                }
                info!("Starting session '{}' for '{}'", session, username);
                let sid = session::launch_session(&state, &username, &session, env).await;
                send_response(&mut writer, &DaemonResponse::SessionStarted {
                    session_id: sid.unwrap_or_else(|| "error".to_string()) }).await;
            }
            GreeterRequest::PowerAction { action } => {
                let cmd = {
                    let st = state.lock().await;
                    let power = st.config.power.clone().unwrap_or_default();
                    match action.as_str() {
                        "shutdown"  => power.shutdown,
                        "reboot"    => power.reboot,
                        "suspend"   => power.suspend,
                        "hibernate" => power.hibernate,
                        _ => None,
                    }
                };
                if let Some(cmd) = cmd {
                    send_response(&mut writer, &DaemonResponse::PowerResult {
                        success: true, message: format!("Executing: {}", cmd) }).await;
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    let _ = tokio::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
                } else {
                    send_response(&mut writer, &DaemonResponse::PowerResult {
                        success: false, message: "Unknown power action".to_string() }).await;
                }
            }
            GreeterRequest::Cancel => {
                send_response(&mut writer, &DaemonResponse::Bye).await;
                break;
            }
        }
    }
    info!("Greeter client disconnected");
}

async fn send_response(writer: &mut tokio::net::unix::WriteHalf<'_>, response: &DaemonResponse) {
    if let Ok(mut json) = serde_json::to_string(response) {
        json.push('\n');
        let _ = writer.write_all(json.as_bytes()).await;
    }
}

async fn build_info_response() -> DaemonResponse {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "localhost".to_string()).trim().to_string();
    let uptime = std::fs::read_to_string("/proc/uptime").unwrap_or_default()
        .split_whitespace().next()
        .and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) as u64;
    let (os_name, os_version) = read_os_release();
    DaemonResponse::Info {
        version: crate::BEDM_VERSION.to_string(),
        hostname, uptime, os_name, os_version,
    }
}

fn read_os_release() -> (String, String) {
    let content = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    let mut name = "Linux".to_string();
    let mut version = String::new();
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim_matches('"').to_string();
            match k { "PRETTY_NAME" => name = v, "VERSION_ID" => version = v, _ => {} }
        }
    }
    (name, version)
}
