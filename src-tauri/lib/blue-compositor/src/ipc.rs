use calloop::LoopHandle;
use serde::{Deserialize, Serialize};
use smithay::wayland::seat::WaylandFocus;
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
    WindowFocused { id: u64 },
    ToggleStartMenu,
    ToggleFullscreenMenu,
    WorkspaceSwitched { index: usize, count: usize },
    SwitcherState { visible: bool, index: usize },
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

type Clients = Arc<Mutex<Vec<UnixStream>>>;

pub fn ipc_socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    PathBuf::from(runtime_dir).join("blue-compositor.sock")
}

pub fn init_ipc(_state: &mut BlueState, loop_handle: &LoopHandle<'static, BlueState>) {
    let socket_path = ipc_socket_path();
    let _ = std::fs::remove_file(&socket_path);

    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => { error!("Failed to bind IPC socket: {}", e); return; }
    };
    listener.set_nonblocking(true).unwrap();
    info!("IPC socket: {:?}", socket_path);

    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let clients_accept = clients.clone();
    let clients_poll = clients.clone();

    // Accept connections
    loop_handle
        .insert_source(
            calloop::generic::Generic::new(listener, calloop::Interest::READ, calloop::Mode::Level),
            move |_, listener, state: &mut BlueState| {
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(true).ok();
                            let msg = CompositorMessage::Ready { socket: state.socket_name().to_string() };
                            send_to_stream(&stream, &msg);
                            clients_accept.lock().unwrap().push(stream);
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => { warn!("IPC accept error: {}", e); break; }
                    }
                }
                Ok(calloop::PostAction::Continue)
            },
        )
        .expect("Failed to insert IPC accept source");

    // Poll + broadcast every 50ms
    loop_handle
        .insert_source(
            calloop::timer::Timer::from_duration(Duration::from_millis(50)),
            move |_, _, state: &mut BlueState| {
                poll_clients(state, &clients_poll);
                broadcast_windows(state, &clients_poll);
                calloop::timer::TimeoutAction::ToDuration(Duration::from_millis(50))
            },
        )
        .expect("Failed to insert IPC poll timer");
}

fn poll_clients(state: &mut BlueState, clients: &Clients) {
    let mut to_remove = Vec::new();
    let mut lock = clients.lock().unwrap();

    for (i, stream) in lock.iter_mut().enumerate() {
        let clone = match stream.try_clone() {
            Ok(c) => c,
            Err(_) => { to_remove.push(i); continue; }
        };
        let mut reader = BufReader::new(clone);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => { to_remove.push(i); break; }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() { continue; }
                    match serde_json::from_str::<ShellMessage>(trimmed) {
                        Ok(msg) => handle_shell_message(state, msg),
                        Err(e) => warn!("IPC parse error: {} ({})", e, trimmed),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => { to_remove.push(i); break; }
            }
        }
    }

    for i in to_remove.into_iter().rev() {
        lock.swap_remove(i);
    }
}

fn handle_shell_message(state: &mut BlueState, msg: ShellMessage) {
    match msg {
        ShellMessage::FocusWindow { id } => {
            if let Some(window) = state.window_by_id(id) {
                state.space.raise_element(&window, true);
                if let Some(surface) = window.wl_surface() {
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    if let Some(kb) = state.seat.get_keyboard() {
                        kb.set_focus(state, Some(surface.into_owned()), serial);
                    }
                }
                if let Some(meta) = state.window_meta.get_mut(&id) {
                    meta.is_minimized = false;
                }
            }
        }
        ShellMessage::CloseWindow { id } => {
            if let Some(window) = state.window_by_id(id) {
                if let Some(t) = window.toplevel() { t.send_close(); }
            }
        }
        ShellMessage::SwitchWorkspace { index } => {
            state.switch_workspace(index);
        }
        ShellMessage::MoveWindowToWorkspace { id, workspace } => {
            if let Some(meta) = state.window_meta.get_mut(&id) {
                meta.workspace = workspace;
            }
        }
        ShellMessage::ToggleMaximize { id } => {
            if let Some(window) = state.window_by_id(id) {
                if let Some(t) = window.toplevel() {
                    t.with_pending_state(|s| {
                                                if s.states.contains(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized) {
                            s.states.unset(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                        } else {
                            s.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                        }
                    });
                    t.send_configure();
                }
            }
        }
        ShellMessage::MinimizeWindow { id } => {
            if let Some(meta) = state.window_meta.get_mut(&id) {
                meta.is_minimized = true;
            }
        }
        ShellMessage::GetWindowList => {}
    }
}

fn broadcast_windows(state: &BlueState, clients: &Clients) {
    let windows = state.ipc_windows.lock().unwrap().clone();
    if windows.is_empty() { return; }
    let msg = CompositorMessage::WindowList { windows };
    broadcast(clients, &msg);
}

fn broadcast(clients: &Clients, msg: &CompositorMessage) {
    let mut lock = clients.lock().unwrap();
    lock.retain(|s| send_to_stream(s, msg));
}

fn send_to_stream(stream: &UnixStream, msg: &CompositorMessage) -> bool {
    let Ok(mut json) = serde_json::to_string(msg) else { return false; };
    json.push('\n');
    stream.try_clone().map(|mut s| s.write_all(json.as_bytes()).is_ok()).unwrap_or(false)
}
