// BEDM IPC Client — used by greeter to talk to daemon

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use tracing::{error, info};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonInfoResponse {
    pub version: String,
    pub hostname: String,
    pub uptime: u64,
    pub os_name: String,
    pub os_version: String,
}

// ── Request types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request<'a> {
    GetInfo,
    GetSessions,
    GetUsers,
    Authenticate {
        username: &'a str,
        password: &'a str,
    },
    StartSession {
        username: &'a str,
        session: &'a str,
        env: Option<Vec<(String, String)>>,
    },
    PowerAction {
        action: &'a str,
    },
    Cancel,
}

// ── Response types ─────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    AuthSuccess { username: String },
    AuthFailure { reason: String, attempts_left: u8 },
    SessionStarted { session_id: String },
    SessionError { reason: String },
    Sessions { sessions: Vec<SessionInfo> },
    Users { users: Vec<UserInfo> },
    Info {
        version: String,
        hostname: String,
        uptime: u64,
        os_name: String,
        os_version: String,
    },
    PowerResult {
        success: bool,
        message: String,
    },
    Error {
        message: String,
    },
    Bye,
}

// ── Client ─────────────────────────────────────────────────────────────────

pub struct BedmClient {
    stream: UnixStream,
}

impl BedmClient {
    pub async fn connect(socket_path: &str) -> Result<(Self, DaemonInfoResponse), String> {
        let stream = UnixStream::connect(socket_path)
            .await
            .map_err(|e| format!("Socket connect failed: {}", e))?;

        let mut client = Self { stream };

        // Read welcome Info message
        let resp = client.read_response().await?;
        match resp {
            Response::Info {
                version,
                hostname,
                uptime,
                os_name,
                os_version,
            } => Ok((
                client,
                DaemonInfoResponse {
                    version,
                    hostname,
                    uptime,
                    os_name,
                    os_version,
                },
            )),
            _ => Err("Unexpected welcome message from daemon".to_string()),
        }
    }

    pub async fn get_sessions(&mut self) -> Result<Vec<SessionInfo>, String> {
        self.send_request(&Request::GetSessions).await?;
        match self.read_response().await? {
            Response::Sessions { sessions } => Ok(sessions),
            Response::Error { message } => Err(message),
            _ => Err("Unexpected response".to_string()),
        }
    }

    pub async fn get_users(&mut self) -> Result<Vec<UserInfo>, String> {
        self.send_request(&Request::GetUsers).await?;
        match self.read_response().await? {
            Response::Users { users } => Ok(users),
            Response::Error { message } => Err(message),
            _ => Err("Unexpected response".to_string()),
        }
    }

    pub async fn authenticate(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<crate::AuthResult, String> {
        self.send_request(&Request::Authenticate { username, password })
            .await?;

        match self.read_response().await? {
            Response::AuthSuccess { username } => Ok(crate::AuthResult {
                success: true,
                username: Some(username),
                error: None,
                attempts_left: 5,
            }),
            Response::AuthFailure {
                reason,
                attempts_left,
            } => Ok(crate::AuthResult {
                success: false,
                username: None,
                error: Some(reason),
                attempts_left,
            }),
            Response::Error { message } => Err(message),
            _ => Err("Unexpected auth response".to_string()),
        }
    }

    pub async fn start_session(
        &mut self,
        username: &str,
        session: &str,
    ) -> Result<String, String> {
        self.send_request(&Request::StartSession {
            username,
            session,
            env: None,
        })
        .await?;

        match self.read_response().await? {
            Response::SessionStarted { session_id } => Ok(session_id),
            Response::SessionError { reason } => Err(reason),
            Response::Error { message } => Err(message),
            _ => Err("Unexpected session response".to_string()),
        }
    }

    pub async fn power_action(&mut self, action: &str) -> Result<bool, String> {
        self.send_request(&Request::PowerAction { action }).await?;
        match self.read_response().await? {
            Response::PowerResult { success, message } => {
                info!("Power: {} — {}", action, message);
                Ok(success)
            }
            Response::Error { message } => Err(message),
            _ => Err("Unexpected power response".to_string()),
        }
    }

    async fn send_request<S: Serialize>(&mut self, req: &S) -> Result<(), String> {
        let mut json = serde_json::to_string(req)
            .map_err(|e| format!("Serialize error: {}", e))?;
        json.push('\n');
        self.stream
            .write_all(json.as_bytes())
            .await
            .map_err(|e| format!("Write error: {}", e))
    }

    async fn read_response(&mut self) -> Result<Response, String> {
        // Read one line from stream
        let mut buf = String::new();
        let mut reader = BufReader::new(&mut self.stream);
        reader
            .read_line(&mut buf)
            .await
            .map_err(|e| format!("Read error: {}", e))?;

        serde_json::from_str(buf.trim()).map_err(|e| format!("Parse error: {} ({})", e, buf.trim()))
    }
}
