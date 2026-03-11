use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, error, info, warn};

// ──────────────────────────────────────────────────────────
// Message types
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IpcMessage {
    // Compositor → Panel / AppMenu
    ToggleAppMenu,
    ToggleFullscreenLauncher,
    ShowWindowSwitcher,
    ShowLogoutDialog,
    WorkspaceChanged { active: usize, total: usize },
    WindowListUpdate { windows: Vec<WindowInfo> },

    // Panel / AppMenu → Compositor
    SetWallpaper { path: String },
    SwitchWorkspace { index: usize },
    FocusWindow { id: u64 },
    CloseWindow { id: u64 },
    LogOut,
    Suspend,
    Reboot,
    PowerOff,

    // Settings → Compositor (live reload)
    ConfigReload,

    // Notification center
    ToggleNotificationCenter,

    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace: usize,
    pub focused: bool,
    pub minimized: bool,
}

// ──────────────────────────────────────────────────────────
// Socket path
// ──────────────────────────────────────────────────────────

pub fn socket_path() -> PathBuf {
    let uid = nix::unistd::getuid().as_raw();
    PathBuf::from(format!("/run/user/{}/blue-environment.sock", uid))
}

// ──────────────────────────────────────────────────────────
// IPC Server (runs inside compositor thread)
// ──────────────────────────────────────────────────────────

type Callback = Box<dyn Fn(IpcMessage) + Send + 'static>;

pub struct IpcServer {
    pub path: PathBuf,
    listeners: Arc<Mutex<Vec<Callback>>>,
}

impl IpcServer {
    pub fn new() -> Result<Self> {
        let path = socket_path();

        // Remove stale socket
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).ok();
        }

        let listeners: Arc<Mutex<Vec<Callback>>> = Arc::new(Mutex::new(Vec::new()));
        let listeners_bg = listeners.clone();
        let path_bg = path.clone();

        thread::spawn(move || {
            match UnixListener::bind(&path_bg) {
                Ok(listener) => {
                    info!("IPC server listening on {:?}", path_bg);
                    for stream in listener.incoming() {
                        match stream {
                            Ok(s) => {
                                let ls = listeners_bg.clone();
                                thread::spawn(move || handle_conn(s, ls));
                            }
                            Err(e) => error!("IPC accept: {}", e),
                        }
                    }
                }
                Err(e) => error!("IPC bind {:?}: {}", path_bg, e),
            }
        });

        Ok(Self { path, listeners })
    }

    /// Register a handler for messages received from external clients
    pub fn on_message<F: Fn(IpcMessage) + Send + 'static>(&self, f: F) {
        self.listeners.lock().unwrap().push(Box::new(f));
    }

    /// Broadcast a message to all clients currently connected to the socket
    pub fn broadcast(&self, msg: &IpcMessage) {
        send_to(&self.path, msg);
    }
}

fn handle_conn(stream: UnixStream, listeners: Arc<Mutex<Vec<Callback>>>) {
    let mut reader = BufReader::new(stream);
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if let Ok(msg) = serde_json::from_str::<IpcMessage>(buf.trim()) {
                    debug!("IPC recv: {:?}", msg);
                    let ls = listeners.lock().unwrap();
                    for cb in ls.iter() {
                        cb(msg.clone());
                    }
                }
            }
            Err(e) => {
                debug!("IPC read error: {}", e);
                break;
            }
        }
    }
}

// ──────────────────────────────────────────────────────────
// Client helpers (used by panel/app_menu threads)
// ──────────────────────────────────────────────────────────

/// Send one message to the compositor IPC socket
pub fn send(msg: &IpcMessage) {
    send_to(&socket_path(), msg);
}

fn send_to(path: &PathBuf, msg: &IpcMessage) {
    match UnixStream::connect(path) {
        Ok(mut s) => {
            if let Ok(j) = serde_json::to_string(msg) {
                s.write_all(j.as_bytes()).ok();
                s.write_all(b"\n").ok();
            }
        }
        Err(e) => warn!("IPC send failed: {}", e),
    }
}

/// Blocking listener — run this in a dedicated thread
pub fn listen<F: Fn(IpcMessage) + Send + 'static>(callback: F) {
    let path = socket_path();
    loop {
        match UnixStream::connect(&path) {
            Ok(s) => {
                let mut reader = BufReader::new(s);
                let mut buf = String::new();
                loop {
                    buf.clear();
                    match reader.read_line(&mut buf) {
                        Ok(0) => break,
                        Ok(_) => {
                            if let Ok(msg) =
                                serde_json::from_str::<IpcMessage>(buf.trim())
                                {
                                    callback(msg);
                                }
                        }
                        Err(_) => break,
                    }
                }
            }
            Err(_) => {
                thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
}
