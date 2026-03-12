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
    // Compositor → Panel / AppMenu (broadcast)
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
// IPC Server (hub — działa w compositorze)
// ──────────────────────────────────────────────────────────

type Callback = Box<dyn Fn(IpcMessage) + Send + 'static>;

/// Aktywne połączenie klienta (panel, app_menu, etc.)
struct Client {
    writer: UnixStream,
}

pub struct IpcServer {
    pub path:    PathBuf,
    /// Callback wywoływany gdy compositor dostanie wiadomość od klienta
    listeners:   Arc<Mutex<Vec<Callback>>>,
    /// Aktywne połączenia klientów — do broadcastu
    clients:     Arc<Mutex<Vec<Client>>>,
}

impl IpcServer {
    pub fn new() -> Result<Self> {
        let path = socket_path();

        if path.exists() { std::fs::remove_file(&path).ok(); }
        if let Some(p) = path.parent() { std::fs::create_dir_all(p).ok(); }

        let listeners: Arc<Mutex<Vec<Callback>>> = Arc::new(Mutex::new(Vec::new()));
        let clients:   Arc<Mutex<Vec<Client>>>   = Arc::new(Mutex::new(Vec::new()));

        let listeners_bg = listeners.clone();
        let clients_bg   = clients.clone();
        let path_bg      = path.clone();

        thread::spawn(move || {
            match UnixListener::bind(&path_bg) {
                Ok(listener) => {
                    info!("IPC server listening on {:?}", path_bg);
                    for stream in listener.incoming() {
                        match stream {
                            Ok(s) => {
                                // Zapisz klon strumienia do listy klientów (do broadcastu)
                                if let Ok(writer) = s.try_clone() {
                                    clients_bg.lock().unwrap()
                                    .push(Client { writer });
                                }
                                let ls = listeners_bg.clone();
                                let cs = clients_bg.clone();
                                thread::spawn(move || handle_conn(s, ls, cs));
                            }
                            Err(e) => error!("IPC accept: {}", e),
                        }
                    }
                }
                Err(e) => error!("IPC bind {:?}: {}", path_bg, e),
            }
        });

        Ok(Self { path, listeners, clients })
    }

    /// Zarejestruj handler dla wiadomości od klientów
    pub fn on_message<F: Fn(IpcMessage) + Send + 'static>(&self, f: F) {
        self.listeners.lock().unwrap().push(Box::new(f));
    }

    /// Wyślij wiadomość do WSZYSTKICH połączonych klientów (panel, app_menu, etc.)
    pub fn broadcast(&self, msg: &IpcMessage) {
        if let Ok(json) = serde_json::to_string(msg) {
            let payload = format!("{}\n", json);
            let mut clients = self.clients.lock().unwrap();
            clients.retain_mut(|c| {
                c.writer.write_all(payload.as_bytes()).is_ok()
            });
        }
    }
}

fn handle_conn(
    stream:    UnixStream,
    listeners: Arc<Mutex<Vec<Callback>>>,
    clients:   Arc<Mutex<Vec<Client>>>,
) {
    let mut reader = BufReader::new(stream);
    let mut buf    = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if let Ok(msg) = serde_json::from_str::<IpcMessage>(buf.trim()) {
                    debug!("IPC recv: {:?}", msg);

                    // 1. Powiadom lokalne handlery (compositor logic)
                    let ls = listeners.lock().unwrap();
                    for cb in ls.iter() { cb(msg.clone()); }
                    drop(ls);

                    // 2. Broadcast do wszystkich klientów (panel, app_menu)
                    //    — dzięki temu panel może wysłać ToggleAppMenu a app_menu to dostanie
                    if let Ok(json) = serde_json::to_string(&msg) {
                        let payload = format!("{}\n", json);
                        let mut cs = clients.lock().unwrap();
                        cs.retain_mut(|c| {
                            c.writer.write_all(payload.as_bytes()).is_ok()
                        });
                    }
                }
            }
            Err(e) => { debug!("IPC read: {}", e); break; }
        }
    }
}

// ──────────────────────────────────────────────────────────
// Client helpers (panel, app_menu, settings)
// ──────────────────────────────────────────────────────────

/// Wyślij jedną wiadomość do hub (compositor)
pub fn send(msg: &IpcMessage) {
    let path = socket_path();
    match UnixStream::connect(&path) {
        Ok(mut s) => {
            if let Ok(j) = serde_json::to_string(msg) {
                s.write_all(j.as_bytes()).ok();
                s.write_all(b"\n").ok();
            }
        }
        Err(e) => warn!("IPC send failed: {}", e),
    }
}

/// Persistent listener — łączy się do hubu i czeka na broadcast wiadomości.
/// Uruchom w osobnym wątku. Automatycznie reconnectuje po utracie połączenia.
pub fn listen<F: Fn(IpcMessage) + Send + 'static>(callback: F) {
    let path = socket_path();
    loop {
        match UnixStream::connect(&path) {
            Ok(s) => {
                debug!("IPC listener connected");
                let mut reader = BufReader::new(s);
                let mut buf    = String::new();
                loop {
                    buf.clear();
                    match reader.read_line(&mut buf) {
                        Ok(0) => break,  // serwer zamknął połączenie
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
                debug!("IPC listener disconnected — reconnecting...");
            }
            Err(_) => {
                // Compositor jeszcze nie gotowy — poczekaj
                thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }
}
