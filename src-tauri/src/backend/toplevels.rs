use serde::Serialize;
use std::os::fd::AsRawFd;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self as handle_ev, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self as manager_ev, ZwlrForeignToplevelManagerV1},
};

/// One window as reported by the compositor.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Toplevel {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub minimized: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub activated: bool,
}

/// Commands the rest of the shell can send to the compositor.
#[derive(Debug, Clone, Copy)]
pub enum Cmd {
    Activate(u64),
    Close(u64),
    SetMinimized(u64, bool),
    SetMaximized(u64, bool),
    SetFullscreen(u64, bool),
}

#[derive(Default)]
struct Shared {
    list: Vec<Toplevel>,
    running: bool,
}

static SHARED: OnceLock<Mutex<Shared>> = OnceLock::new();
static TX: OnceLock<Mutex<Option<Sender<Cmd>>>> = OnceLock::new();

fn shared() -> &'static Mutex<Shared> {
    SHARED.get_or_init(|| Mutex::new(Shared::default()))
}

fn tx() -> &'static Mutex<Option<Sender<Cmd>>> {
    TX.get_or_init(|| Mutex::new(None))
}

/// Starts the tracker thread (idempotent). Returns immediately; use
/// [`is_running`] to see whether the compositor really offered the
/// protocol.
pub fn start() {
    let mut guard = tx().lock().unwrap();
    if guard.is_some() {
        return;
    }
    let (sender, receiver) = channel::<Cmd>();
    *guard = Some(sender);
    drop(guard);
    let _ = std::thread::Builder::new()
        .name("blue-toplevels".into())
        .spawn(move || {
            // Reconnect loop: if the compositor restarts (or the first
            // attempt races its startup) we simply try again.
            let mut attempts = 0u32;
            loop {
                match run(&receiver) {
                    Ok(()) => break,
                    Err(e) => {
                        shared().lock().unwrap().running = false;
                        attempts += 1;
                        if attempts == 1 || attempts % 30 == 0 {
                            eprintln!("[blue-toplevels] {e} (retrying)");
                        }
                        if attempts > 300 {
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(2));
                    }
                }
            }
        });
}

pub fn is_running() -> bool {
    shared().lock().map(|s| s.running).unwrap_or(false)
}

/// Snapshot of every currently-mapped window.
pub fn list() -> Vec<Toplevel> {
    shared().lock().map(|s| s.list.clone()).unwrap_or_default()
}

pub fn send(cmd: Cmd) {
    if let Some(sender) = tx().lock().unwrap().as_ref() {
        let _ = sender.send(cmd);
    }
}

/// True for the Blue Environment shell's own window — it must never show
/// up in its own taskbar / Alt-Tab list.
pub fn is_shell_window(t: &Toplevel) -> bool {
    let app = t.app_id.to_lowercase();
    app.contains("blue-environment") || app.contains("blue_environment") || t.title == "Blue Environment"
}

// ── Wayland state ───────────────────────────────────────────────────────

struct Entry {
    id: u64,
    handle: ZwlrForeignToplevelHandleV1,
    title: String,
    app_id: String,
    minimized: bool,
    maximized: bool,
    fullscreen: bool,
    activated: bool,
    /// Received its first `done` — before that the data is incomplete.
    ready: bool,
}

struct State {
    seat: Option<wl_seat::WlSeat>,
    entries: Vec<Entry>,
    next_id: u64,
    dirty: bool,
}

impl State {
    fn entry_mut(&mut self, handle: &ZwlrForeignToplevelHandleV1) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.handle.id() == handle.id())
    }

    fn find(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Activate(id) => {
                if let (Some(e), Some(seat)) = (self.find(id), self.seat.as_ref()) {
                    if e.minimized {
                        e.handle.unset_minimized();
                    }
                    e.handle.activate(seat);
                }
            }
            Cmd::Close(id) => {
                if let Some(e) = self.find(id) {
                    e.handle.close();
                }
            }
            Cmd::SetMinimized(id, on) => {
                if let Some(e) = self.find(id) {
                    if on {
                        e.handle.set_minimized();
                    } else {
                        e.handle.unset_minimized();
                    }
                }
            }
            Cmd::SetMaximized(id, on) => {
                if let Some(e) = self.find(id) {
                    if on {
                        e.handle.set_maximized();
                    } else {
                        e.handle.unset_maximized();
                    }
                }
            }
            Cmd::SetFullscreen(id, on) => {
                if let Some(e) = self.find(id) {
                    if on {
                        e.handle.set_fullscreen(None);
                    } else {
                        e.handle.unset_fullscreen();
                    }
                }
            }
        }
    }

    fn snapshot(&self) -> Vec<Toplevel> {
        self.entries
            .iter()
            .filter(|e| e.ready)
            .map(|e| Toplevel {
                id: e.id,
                title: e.title.clone(),
                app_id: e.app_id.clone(),
                minimized: e.minimized,
                maximized: e.maximized,
                fullscreen: e.fullscreen,
                activated: e.activated,
            })
            .collect()
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(state: &mut Self, _: &ZwlrForeignToplevelManagerV1, event: manager_ev::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let manager_ev::Event::Toplevel { toplevel } = event {
            state.next_id += 1;
            state.entries.push(Entry {
                id: state.next_id,
                handle: toplevel,
                title: String::new(),
                app_id: String::new(),
                minimized: false,
                maximized: false,
                fullscreen: false,
                activated: false,
                ready: false,
            });
        }
    }

    event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        manager_ev::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(state: &mut Self, handle: &ZwlrForeignToplevelHandleV1, event: handle_ev::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            handle_ev::Event::Title { title } => {
                if let Some(e) = state.entry_mut(handle) {
                    e.title = title;
                }
            }
            handle_ev::Event::AppId { app_id } => {
                if let Some(e) = state.entry_mut(handle) {
                    e.app_id = app_id;
                }
            }
            handle_ev::Event::State { state: raw } => {
                if let Some(e) = state.entry_mut(handle) {
                    e.minimized = false;
                    e.maximized = false;
                    e.fullscreen = false;
                    e.activated = false;
                    for chunk in raw.chunks_exact(4) {
                        // enum state: maximized=0, minimized=1, activated=2, fullscreen=3
                        match u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) {
                            0 => e.maximized = true,
                            1 => e.minimized = true,
                            2 => e.activated = true,
                            3 => e.fullscreen = true,
                            _ => {}
                        }
                    }
                }
            }
            handle_ev::Event::Done => {
                if let Some(e) = state.entry_mut(handle) {
                    e.ready = true;
                }
                state.dirty = true;
            }
            handle_ev::Event::Closed => {
                if let Some(pos) = state.entries.iter().position(|e| e.handle.id() == handle.id()) {
                    let removed = state.entries.remove(pos);
                    removed.handle.destroy();
                }
                state.dirty = true;
            }
            _ => {}
        }
    }
}

fn run(rx: &Receiver<Cmd>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("wayland connect: {e}"))?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| format!("registry: {e}"))?;
    let qh = queue.handle();

    let _manager: ZwlrForeignToplevelManagerV1 = globals
        .bind(&qh, 1..=3, ())
        .map_err(|_| "compositor does not offer wlr-foreign-toplevel-management".to_string())?;
    let seat: Option<wl_seat::WlSeat> = globals.bind(&qh, 1..=7, ()).ok();

    let mut state = State { seat, entries: Vec::new(), next_id: 0, dirty: false };
    shared().lock().unwrap().running = true;

    let mut last_list: Vec<Toplevel> = Vec::new();
    let mut last_active: Option<u64> = None;

    loop {
        while let Ok(cmd) = rx.try_recv() {
            state.apply(cmd);
        }
        conn.flush().map_err(|e| format!("flush: {e}"))?;
        queue.dispatch_pending(&mut state).map_err(|e| format!("dispatch: {e}"))?;

        if let Some(guard) = conn.prepare_read() {
            let mut pfd = libc::pollfd { fd: guard.connection_fd().as_raw_fd(), events: libc::POLLIN, revents: 0 };
            // Short timeout so queued commands (activate/close…) are
            // picked up promptly even when the compositor is quiet.
            let n = unsafe { libc::poll(&mut pfd, 1, 50) };
            if n > 0 {
                if let Err(e) = guard.read() {
                    if !matches!(&e, wayland_client::backend::WaylandError::Io(io) if io.kind() == std::io::ErrorKind::WouldBlock) {
                        return Err(format!("read: {e}"));
                    }
                }
            } else {
                drop(guard);
            }
        }
        queue.dispatch_pending(&mut state).map_err(|e| format!("dispatch: {e}"))?;

        if state.dirty {
            state.dirty = false;
            let snap = state.snapshot();
            if snap != last_list {
                last_list = snap.clone();
                shared().lock().unwrap().list = snap.clone();
                publish(&snap, &mut last_active);
            }
        }
    }
}

/// Converts to the exact JSON shape HackerOS-Comp uses for its
/// `window_list` message so the frontend needs no special casing.
fn publish(list: &[Toplevel], last_active: &mut Option<u64>) {
    let windows: Vec<serde_json::Value> = list
        .iter()
        .filter(|t| !is_shell_window(t))
        .map(|t| {
            serde_json::json!({
                "id": t.id, "title": t.title, "app_id": t.app_id,
                "x": 0, "y": 0, "width": 0, "height": 0,
                "is_fullscreen": t.fullscreen, "is_minimized": t.minimized,
                "workspace": 0,
            })
        })
        .collect();
    super::emit("compositor:window-list", serde_json::json!({ "windows": windows }));

    let active = list.iter().find(|t| t.activated && !is_shell_window(t)).map(|t| t.id);
    if active != *last_active {
        *last_active = active;
        if let Some(id) = active {
            super::emit("compositor:window-focused", serde_json::json!({ "id": id }));
        }
    }
}
