use serde::{Deserialize, Serialize};
use smithay::reexports::{
    calloop::LoopHandle,
    wayland_protocols::xdg::shell::server::xdg_toplevel,
};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::{error, info, warn};

use crate::state::{BlueState, WindowInfo};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompositorMessage {
    Ready { socket: String },
    WindowList { windows: Vec<WindowInfo> },
    WorkspaceSwitched { index: usize },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShellMessage {
    FocusWindow { id: u64 },
    CloseWindow { id: u64 },
    SwitchWorkspace { index: usize },
    GetWindowList,
    MoveWindowToWorkspace { id: u64, workspace: usize },
    ToggleMaximize { id: u64 },
    MinimizeWindow { id: u64 },
}

pub fn ipc_socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    PathBuf::from(runtime).join("blue-compositor.sock")
}

type Clients = Arc<Mutex<Vec<UnixStream>>>;

pub fn init_ipc(state: &mut BlueState, loop_handle: &LoopHandle<'static, BlueState>) {
    let path = ipc_socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => { error!("IPC bind failed: {}", e); return }
    };
    listener.set_nonblocking(true).unwrap();
    info!("IPC socket: {:?}", path);

    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let clients_accept = clients.clone();
    let clients_poll = clients.clone();

    // Accept new connections
    loop_handle.insert_source(
        smithay::reexports::calloop::generic::Generic::new(
            listener,
            smithay::reexports::calloop::Interest::READ,
            smithay::reexports::calloop::Mode::Level,
        ),
        move |_, listener, state: &mut BlueState| {
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(true).ok();
                        send_msg(&stream, &CompositorMessage::Ready { socket: state.socket_name().to_string() });
                        clients_accept.lock().unwrap().push(stream);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => { warn!("IPC accept: {}", e); break }
                }
            }
            Ok(smithay::reexports::calloop::PostAction::Continue)
        },
    ).expect("IPC accept source");

    // Poll loop
    loop_handle.insert_source(
        smithay::reexports::calloop::timer::Timer::from_duration(Duration::from_millis(50)),
        move |_, _, state: &mut BlueState| {
            poll_clients(state, &clients_poll);
            broadcast_windows(state, &clients_poll);
            smithay::reexports::calloop::timer::TimeoutAction::ToDuration(Duration::from_millis(50))
        },
    ).expect("IPC poll timer");
}

fn poll_clients(state: &mut BlueState, clients: &Clients) {
    let mut to_remove = Vec::new();
    let mut lock = clients.lock().unwrap();
    for (i, stream) in lock.iter_mut().enumerate() {
        let clone = match stream.try_clone() { Ok(c) => c, Err(_) => { to_remove.push(i); continue } };
        let mut reader = BufReader::new(clone);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => { to_remove.push(i); break }
                Ok(_) => {
                    let t = line.trim();
                    if t.is_empty() { continue }
                    match serde_json::from_str::<ShellMessage>(t) {
                        Ok(msg) => handle_message(state, msg),
                        Err(e) => warn!("IPC parse: {} ({})", e, t),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => { to_remove.push(i); break }
            }
        }
    }
    for i in to_remove.into_iter().rev() { lock.swap_remove(i); }
}

fn handle_message(state: &mut BlueState, msg: ShellMessage) {
    use smithay::wayland::seat::WaylandFocus;
    match msg {
        ShellMessage::FocusWindow { id } => {
            if let Some(win) = state.window_by_id(id) {
                state.space.raise_element(&win, true);
                if let Some(surf) = win.wl_surface() {
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    if let Some(kb) = state.seat.get_keyboard() {
                        kb.set_focus(state, Some(surf.into_owned()), serial);
                    }
                }
                if let Some(m) = state.window_meta.get_mut(&id) { m.is_minimized = false; }
            }
        }
        ShellMessage::CloseWindow { id } => {
            if let Some(win) = state.window_by_id(id) {
                if let Some(t) = win.toplevel() { t.send_close(); }
            }
        }
        ShellMessage::SwitchWorkspace { index } => state.switch_workspace(index),
        ShellMessage::MoveWindowToWorkspace { id, workspace } => {
            if let Some(m) = state.window_meta.get_mut(&id) { m.workspace = workspace; }
        }
        ShellMessage::ToggleMaximize { id } => {
            if let Some(win) = state.window_by_id(id) {
                if let Some(t) = win.toplevel() {
                    t.with_pending_state(|s| {
                        if s.states.contains(xdg_toplevel::State::Maximized) {
                            s.states.unset(xdg_toplevel::State::Maximized);
                        } else {
                            s.states.set(xdg_toplevel::State::Maximized);
                        }
                    });
                    t.send_configure();
                }
            }
        }
        ShellMessage::MinimizeWindow { id } => {
            if let Some(m) = state.window_meta.get_mut(&id) { m.is_minimized = true; }
        }
        ShellMessage::GetWindowList => {}
    }
}

fn broadcast_windows(state: &BlueState, clients: &Clients) {
    let windows = state.ipc_windows.lock().unwrap().clone();
    if windows.is_empty() { return }
    let msg = CompositorMessage::WindowList { windows };
    let mut lock = clients.lock().unwrap();
    lock.retain(|s| send_msg(s, &msg));
}

fn send_msg(stream: &UnixStream, msg: &CompositorMessage) -> bool {
    let Ok(mut json) = serde_json::to_string(msg) else { return false };
    json.push('\n');
    stream.try_clone().map(|mut s| s.write_all(json.as_bytes()).is_ok()).unwrap_or(false)
}
