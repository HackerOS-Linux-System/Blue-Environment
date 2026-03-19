mod state;
mod input;
mod render;
mod ipc;
mod xwayland;

use std::time::Duration;
use smithay::backend::session::Session;
use tracing::{info, error, warn};
use tracing_subscriber::{EnvFilter, fmt};

fn main() {
    fmt()
    .with_env_filter(
        EnvFilter::try_from_env("BLUE_LOG")
        .unwrap_or_else(|_| EnvFilter::new("blue_compositor=info,smithay=warn")),
    )
    .init();

    info!("Blue Compositor starting...");

    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        warn!("Existing display session detected — running nested (winit)");
        run_winit();
    } else {
        info!("TTY mode — using DRM/KMS backend");
        run_udev();
    }
}

fn run_udev() {
    use smithay::backend::session::libseat::LibSeatSession;

    let (session, _notifier) = match LibSeatSession::new() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create libseat session: {}", e);
            error!("Make sure seatd is running: sudo systemctl start seatd");
            error!("And add yourself to seat group: sudo usermod -aG seat $USER");
            std::process::exit(1);
        }
    };

    info!("Seat: {}", session.seat());

    let event_loop: calloop::EventLoop<'static, state::BlueState> =
    calloop::EventLoop::try_new().expect("Failed to create event loop");
    let display: wayland_server::Display<state::BlueState> =
    wayland_server::Display::new().expect("Failed to create Wayland display");

    let loop_handle = event_loop.handle();
    let mut st = state::BlueState::new(&loop_handle, display);
    st.init_udev(session, &loop_handle);
    st.init_xwayland(&loop_handle);
    st.init_ipc(&loop_handle);

    info!("Compositor ready — WAYLAND_DISPLAY={}", st.socket_name());
    std::env::set_var("WAYLAND_DISPLAY", st.socket_name());
    std::env::set_var("DISPLAY", ":0");

    run_loop(event_loop, st);
}

fn run_winit() {
    use smithay::backend::winit;

    let event_loop: calloop::EventLoop<'static, state::BlueState> =
    calloop::EventLoop::try_new().expect("Failed to create event loop");
    let display: wayland_server::Display<state::BlueState> =
    wayland_server::Display::new().expect("Failed to create Wayland display");

    let loop_handle = event_loop.handle();
    let mut st = state::BlueState::new(&loop_handle, display);

    let (winit_backend, winit_evt) =
    winit::init::<smithay::backend::renderer::gles::GlesRenderer>()
    .expect("Failed to init winit backend");

    st.init_winit(winit_backend, winit_evt, &loop_handle);
    st.init_xwayland(&loop_handle);
    st.init_ipc(&loop_handle);

    info!("Nested compositor ready — WAYLAND_DISPLAY={}", st.socket_name());
    std::env::set_var("WAYLAND_DISPLAY", st.socket_name());

    run_loop(event_loop, st);
}

fn run_loop(
    mut event_loop: calloop::EventLoop<'static, state::BlueState>,
    mut state: state::BlueState,
) {
    loop {
        if let Err(e) = event_loop.dispatch(Some(Duration::from_millis(16)), &mut state) {
            error!("Event loop error: {}", e);
            break;
        }
        state.refresh();
        if state.should_exit() {
            info!("Compositor exiting");
            break;
        }
    }
}
