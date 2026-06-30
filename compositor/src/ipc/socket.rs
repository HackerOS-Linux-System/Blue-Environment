use calloop::LoopHandle;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::{error, info, warn};

use crate::state::BlueState;
use super::messages::{CompositorMessage, ShellMessage};
use super::handler::handle_shell_message;

pub type Clients = Arc<Mutex<Vec<UnixStream>>>;

pub fn ipc_socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    PathBuf::from(runtime_dir).join("blue-compositor.sock")
}

pub fn init_ipc(state: &mut BlueState, loop_handle: &LoopHandle<'static, BlueState>) {
    let socket_path = ipc_socket_path();
    let _ = std::fs::remove_file(&socket_path);

    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => { error!("IPC bind failed at {:?}: {}", socket_path, e); return; }
    };
    listener.set_nonblocking(true).unwrap();
    info!("IPC socket: {:?}", socket_path);

    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let clients_accept = clients.clone();
    state.clients = clients;

    // Accept new connections
    loop_handle.insert_source(
        calloop::generic::Generic::new(listener, calloop::Interest::READ, calloop::Mode::Level),
        move |_, listener, state: &mut BlueState| {
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(true).ok();
                        let msg = CompositorMessage::Ready { socket: state.socket_name().to_string() };
                        send_to_stream(&stream, &msg);
                        clients_accept.lock().unwrap().push(stream);
                        info!("IPC client connected");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => { warn!("IPC accept: {}", e); break; }
                }
            }
            Ok(calloop::PostAction::Continue)
        },
    ).expect("IPC accept source");

    // Poll clients every 33ms
    loop_handle.insert_source(
        calloop::timer::Timer::from_duration(Duration::from_millis(33)),
        |_, _, state: &mut BlueState| {
            let clients = state.clients.clone();
            poll_clients(state, &clients);
            broadcast_windows(state, &clients);
            calloop::timer::TimeoutAction::ToDuration(Duration::from_millis(33))
        },
    ).expect("IPC poll timer");
}

fn poll_clients(state: &mut BlueState, clients: &Clients) {
    let mut to_remove = Vec::new();
    let mut messages  = Vec::new();

    {
        let mut lock = clients.lock().unwrap();
        for (i, stream) in lock.iter_mut().enumerate() {
            let Ok(clone) = stream.try_clone() else { to_remove.push(i); continue; };
            let mut reader = BufReader::new(clone);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => { to_remove.push(i); break; }
                    Ok(_) => {
                        let t = line.trim();
                        if t.is_empty() { continue; }
                        match serde_json::from_str::<ShellMessage>(t) {
                            Ok(msg) => messages.push(msg),
                            Err(e)  => warn!("IPC parse: {} ({})", e, t),
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => { to_remove.push(i); break; }
                }
            }
        }
        for i in to_remove.into_iter().rev() {
            info!("IPC client disconnected");
            lock.swap_remove(i);
        }
    }

    for msg in messages {
        handle_shell_message(state, msg);
    }
}

fn broadcast_windows(state: &BlueState, clients: &Clients) {
    let windows = state.ipc_windows.lock().unwrap().clone();
    if windows.is_empty() { return; }
    broadcast(clients, &CompositorMessage::WindowList { windows });
}

pub fn broadcast(clients: &Clients, msg: &CompositorMessage) {
    let mut lock = clients.lock().unwrap();
    lock.retain(|s| send_to_stream(s, msg));
}

fn send_to_stream(stream: &UnixStream, msg: &CompositorMessage) -> bool {
    let Ok(mut json) = serde_json::to_string(msg) else { return false; };
    json.push('\n');
    stream.try_clone().map(|mut s| s.write_all(json.as_bytes()).is_ok()).unwrap_or(false)
}

// Public broadcast helpers
pub fn broadcast_workspace_switch(clients: &Clients, index: usize, count: usize) {
    broadcast(clients, &CompositorMessage::WorkspaceSwitched { index, count });
}
pub fn broadcast_start_menu_toggle(clients: &Clients) {
    broadcast(clients, &CompositorMessage::ToggleStartMenu);
}
pub fn broadcast_window_opened(clients: &Clients, window: crate::state::WindowInfo) {
    broadcast(clients, &CompositorMessage::WindowOpened { window });
}
pub fn broadcast_window_closed(clients: &Clients, id: u64) {
    broadcast(clients, &CompositorMessage::WindowClosed { id });
}
pub fn broadcast_idle_changed(clients: &Clients, idle: bool) {
    broadcast(clients, &CompositorMessage::IdleChanged { idle });
}
