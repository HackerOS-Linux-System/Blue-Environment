// BEDM IPC — Unix domain socket server at /run/bedm/bedm.sock
// Protocol: newline-delimited JSON

use crate::{session, DaemonState, SOCKET_PATH};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Mutex,
};
use tracing::{error, info, warn};

// ── Messages greeter → daemon ──────────────────────────────────────────────
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum GreeterRequest {
    /// Authenticate user with password
    Authenticate {
        username: String,
        password: String,
    },
    /// Launch a session after successful auth
    StartSession {
        username: String,
        session: String,
        env: Option<Vec<(String, String)>>,
    },
    /// Get list of available sessions
    GetSessions,
    /// Get list of available users
    GetUsers,
    /// Get daemon info
    GetInfo,
    /// Power action
    PowerAction { action: String },
    /// Cancel / close greeter
    Cancel,
}

// ── Messages daemon → greeter ──────────────────────────────────────────────
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    /// Auth success — client can now call StartSession
    AuthSuccess { username: String },
    /// Auth failure
    AuthFailure { reason: String, attempts_left: u8 },
    /// Session launched
    SessionStarted { session_id: String },
    /// Session launch failed
    SessionError { reason: String },
    /// List of sessions
    Sessions { sessions: Vec<SessionInfo> },
    /// List of users
    Users { users: Vec<UserInfo> },
    /// Daemon info
    Info {
        version: String,
        hostname: String,
        uptime: u64,
        os_name: String,
        os_version: String,
    },
    /// Power action result
    PowerResult { success: bool, message: String },
    /// Generic error
    Error { message: String },
    /// Goodbye
    Bye,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub session_type: String, // "wayland" | "x11"
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
        Err(e) => {
            error!("Failed to bind BEDM socket at {}: {}", SOCKET_PATH, e);
            return;
        }
    };

    // Make socket group-readable for greeter
    let _ = std::os::unix::fs::chown(SOCKET_PATH, Some(0), None);
    let _ = std::fs::set_permissions(
        SOCKET_PATH,
        std::os::unix::fs::PermissionsExt::from_mode(0o666),
    );

    info!("BEDM IPC listening on {}", SOCKET_PATH);

    // Handle signals
    tokio::spawn(async {
        let mut term = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )
        .unwrap();
        term.recv().await;
        info!("SIGTERM received — shutting down BEDM");
        let _ = std::fs::remove_file(SOCKET_PATH);
        std::process::exit(0);
    });

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                info!("Greeter connected");
                let state_clone = state.clone();
                tokio::spawn(async move {
                    handle_client(stream, state_clone).await;
                });
            }
            Err(e) => {
                error!("Accept error: {}", e);
            }
        }
    }
}

async fn handle_client(mut stream: UnixStream, state: Arc<Mutex<DaemonState>>) {
    let (reader, mut writer) = stream.split();
    let mut lines = BufReader::new(reader).lines();
    let mut auth_user: Option<String> = None;
    let mut fail_count: u8 = 0;

    // Send welcome info
    let info = build_info_response().await;
    send_response(&mut writer, &info).await;

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: GreeterRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = DaemonResponse::Error {
                    message: format!("Parse error: {}", e),
                };
                send_response(&mut writer, &resp).await;
                continue;
            }
        };

        match request {
            GreeterRequest::GetInfo => {
                let resp = build_info_response().await;
                send_response(&mut writer, &resp).await;
            }

            GreeterRequest::GetSessions => {
                let sessions = crate::session::list_sessions(&state).await;
                let resp = DaemonResponse::Sessions { sessions };
                send_response(&mut writer, &resp).await;
            }

            GreeterRequest::GetUsers => {
                let users = crate::users::list_users(&state).await;
                let resp = DaemonResponse::Users { users };
                send_response(&mut writer, &resp).await;
            }

            GreeterRequest::Authenticate { username, password } => {
                if fail_count >= 5 {
                    let resp = DaemonResponse::AuthFailure {
                        reason: "Too many failed attempts".to_string(),
                        attempts_left: 0,
                    };
                    send_response(&mut writer, &resp).await;
                    break;
                }

                // Check allow_root
                let allow_root = state.lock().await.config.allow_root.unwrap_or(false);
                if username == "root" && !allow_root {
                    let resp = DaemonResponse::AuthFailure {
                        reason: "Root login not permitted".to_string(),
                        attempts_left: 5 - fail_count - 1,
                    };
                    send_response(&mut writer, &resp).await;
                    fail_count += 1;
                    continue;
                }

                info!("Auth attempt for: {}", username);
                match crate::pam_auth::authenticate(&username, &password) {
                    Ok(()) => {
                        info!("Auth success: {}", username);
                        auth_user = Some(username.clone());
                        fail_count = 0;
                        let resp = DaemonResponse::AuthSuccess { username };
                        send_response(&mut writer, &resp).await;
                    }
                    Err(reason) => {
                        warn!("Auth failure for {}: {}", username, reason);
                        fail_count += 1;
                        let resp = DaemonResponse::AuthFailure {
                            reason,
                            attempts_left: 5u8.saturating_sub(fail_count),
                        };
                        send_response(&mut writer, &resp).await;
                    }
                }
            }

            GreeterRequest::StartSession {
                username,
                session,
                env,
            } => {
                // Must authenticate first (or autologin)
                let authed = auth_user.as_deref() == Some(username.as_str());
                if !authed {
                    let resp = DaemonResponse::Error {
                        message: "Not authenticated".to_string(),
                    };
                    send_response(&mut writer, &resp).await;
                    continue;
                }

                info!("Starting session: {} for {}", session, username);
                let session_id =
                    session::launch_session(&state, &username, &session, env).await;
                let resp = DaemonResponse::SessionStarted {
                    session_id: session_id.unwrap_or_else(|| "error".to_string()),
                };
                send_response(&mut writer, &resp).await;
            }

            GreeterRequest::PowerAction { action } => {
                info!("Power action: {}", action);
                let cmd = {
                    let st = state.lock().await;
                    let power = st.config.power.clone().unwrap_or_default();
                    match action.as_str() {
                        "shutdown" => power.shutdown,
                        "reboot" => power.reboot,
                        "suspend" => power.suspend,
                        "hibernate" => power.hibernate,
                        _ => None,
                    }
                };

                if let Some(cmd) = cmd {
                    let resp = DaemonResponse::PowerResult {
                        success: true,
                        message: format!("Executing: {}", cmd),
                    };
                    send_response(&mut writer, &resp).await;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    let _ = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .spawn();
                } else {
                    let resp = DaemonResponse::PowerResult {
                        success: false,
                        message: "Unknown power action".to_string(),
                    };
                    send_response(&mut writer, &resp).await;
                }
            }

            GreeterRequest::Cancel => {
                send_response(&mut writer, &DaemonResponse::Bye).await;
                break;
            }
        }
    }

    info!("Greeter disconnected");
}

async fn send_response(
    writer: &mut tokio::net::unix::WriteHalf<'_>,
    response: &DaemonResponse,
) {
    if let Ok(mut json) = serde_json::to_string(response) {
        json.push('\n');
        let _ = writer.write_all(json.as_bytes()).await;
    }
}

async fn build_info_response() -> DaemonResponse {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "localhost".to_string())
        .trim()
        .to_string();

    let uptime = std::fs::read_to_string("/proc/uptime")
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;

    let (os_name, os_version) = read_os_release();

    DaemonResponse::Info {
        version: crate::BEDM_VERSION.to_string(),
        hostname,
        uptime,
        os_name,
        os_version,
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
            match k {
                "PRETTY_NAME" => name = v,
                "VERSION_ID" => version = v,
                _ => {}
            }
        }
    }

    (name, version)
}

// ── Permissions helper ─────────────────────────────────────────────────────
use std::os::unix::fs::PermissionsExt;
