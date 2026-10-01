use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex as StdMutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;
use tauri::{Emitter, Window};
use lazy_static::lazy_static;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

// ── Simple line-buffered terminal (fallback backend) ────────────────────────

struct TerminalProcess {
    stdin: tokio::process::ChildStdin,
}

lazy_static! {
    static ref TERMINALS: std::sync::Arc<Mutex<HashMap<String, TerminalProcess>>> =
    std::sync::Arc::new(Mutex::new(HashMap::new()));
}

#[derive(Serialize)]
pub struct SpawnTerminalResult {
    pub success: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn spawn_terminal(window: Window, window_id: String) -> Result<SpawnTerminalResult, String> {
    let child_result = TokioCommand::new("bash")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn();

    let mut child = match child_result {
        Ok(c) => c,
        Err(e) => return Ok(SpawnTerminalResult { success: false, error: Some(e.to_string()) }),
    };

    let stdin = match child.stdin.take() {
        Some(s) => s,
        None => return Ok(SpawnTerminalResult { success: false, error: Some("Failed to open stdin".to_string()) }),
    };
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return Ok(SpawnTerminalResult { success: false, error: Some("Failed to open stdout".to_string()) }),
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => return Ok(SpawnTerminalResult { success: false, error: Some("Failed to open stderr".to_string()) }),
    };

    let window_clone = window.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = window_clone.emit("terminal-output", serde_json::json!({"type":"stdout","data":line+"\r\n"}));
        }
    });

    let window_clone = window.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = window_clone.emit("terminal-output", serde_json::json!({"type":"stderr","data":line+"\r\n"}));
        }
    });

    TERMINALS.lock().await.insert(window_id, TerminalProcess { stdin });
    Ok(SpawnTerminalResult { success: true, error: None })
}

#[tauri::command]
pub async fn write_to_terminal(window_id: String, command: String) -> Result<(), String> {
    let mut terminals = TERMINALS.lock().await;
    if let Some(term) = terminals.get_mut(&window_id) {
        let cmd = command + "\n";
        term.stdin.write_all(cmd.as_bytes()).await.map_err(|e| e.to_string())?;
        term.stdin.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Terminal not found".to_string())
    }
}

// ── Real PTY backend ─────────────────────────────────────────────────────────

pub struct PtySession {
    writer: Box<dyn std::io::Write + Send>,
    pid: u32,
}
pub type PtySessions = std::sync::Arc<StdMutex<HashMap<String, PtySession>>>;

/// Constructs the empty session map handed to `tauri::Builder::manage()`
/// at startup — kept here so main.rs doesn't need to know `PtySession`'s
/// internal shape.
pub fn new_pty_sessions() -> PtySessions {
    std::sync::Arc::new(StdMutex::new(HashMap::new()))
}

#[tauri::command(async)]
pub fn pty_create(
    id: String,
    shell: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
    window: tauri::Window,
    sessions: tauri::State<PtySessions>,
) -> Result<(), String> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: rows.unwrap_or(24),
                                  cols: cols.unwrap_or(80),
                                  pixel_width: 0,
                                  pixel_height: 0,
    }).map_err(|e| e.to_string())?;

    let shell_path = shell.unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()));
    let mut cmd = CommandBuilder::new(&shell_path);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if let Ok(home) = std::env::var("HOME") { cmd.env("HOME", &home); }
    if let Ok(user) = std::env::var("USER") { cmd.env("USER", &user); }
    cmd.cwd(std::env::var("HOME").unwrap_or_else(|_| "/".to_string()));

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    let id_clone = id.clone();
    let win = window.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = win.emit(&format!("pty-exit-{}", id_clone), ());
                    break;
                }
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = win.emit(&format!("pty-data-{}", id_clone), data);
                }
            }
        }
    });

    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    let pid = child.process_id().unwrap_or(0);
    map.insert(id, PtySession { writer: Box::new(writer), pid });
    Ok(())
}

#[tauri::command(async)]
pub fn pty_write(id: String, data: String, sessions: tauri::State<PtySessions>) -> Result<(), String> {
    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    if let Some(session) = map.get_mut(&id) {
        session.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        session.writer.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command(async)]
pub fn pty_resize(_id: String, _cols: u16, _rows: u16, _sessions: tauri::State<PtySessions>) -> Result<(), String> {
    Ok(())
}

#[tauri::command(async)]
pub fn pty_close(id: String, sessions: tauri::State<PtySessions>) -> Result<(), String> {
    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    if let Some(session) = map.remove(&id) {
        if session.pid > 0 {
            unsafe { libc::kill(session.pid as i32, libc::SIGTERM); }
        }
    }
    Ok(())
}
