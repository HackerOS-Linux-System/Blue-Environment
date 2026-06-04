mod state;
mod input;
mod render;
mod ipc;
mod xwayland;

use std::time::Duration;
use smithay::backend::session::Session;
use tracing::{info, error, warn};
use tracing_subscriber::{EnvFilter, fmt};
use std::fs;

fn init_logging() {
    let home = dirs::home_dir().expect("Home directory not found");
    let log_dir = home.join(".cache/Blue-Environment/compositor/logs");
    fs::create_dir_all(&log_dir).ok();

    let file_name = format!(
        "compositor_{}.log",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );

    let file_appender = tracing_appender::rolling::never(&log_dir, file_name);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("blue_compositor=info,smithay=warn")),
        )
        .with_writer(non_blocking)
        .with_ansi(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).ok();
}

fn write_desktop_file() -> std::io::Result<()> {
    let exe_path = std::env::current_exe()?;
    let desktop_path = dirs::home_dir()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "No home dir")
        })?
        .join(".local/share/wayland-sessions/blue-environment.desktop");

    fs::create_dir_all(desktop_path.parent().unwrap())?;
    let content = format!(
        "[Desktop Entry]\nName=Blue Environment\nComment=Blue Wayland Compositor\nExec={}\nType=Application\nCategories=System;\n",
        exe_path.display()
    );
    fs::write(desktop_path, content)
}

fn main() {
    // Stderr logging for startup
    fmt()
        .with_env_filter(
            EnvFilter::try_from_env("BLUE_LOG")
                .unwrap_or_else(|_| EnvFilter::new("blue_compositor=info,smithay=warn")),
        )
        .init();

    init_logging();
    info!("Blue Compositor v0.2 starting...");

    if let Err(e) = write_desktop_file() {
        warn!("Could not write desktop file: {}", e);
    }

    // Detect if we already have a display server
    let has_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let has_x11 = std::env::var("DISPLAY").is_ok();

    if has_wayland || has_x11 {
        warn!(
            "Existing display session detected (wayland={}, x11={}) — running nested (winit)",
            has_wayland, has_x11
        );
        run_winit();
    } else {
        info!("No display server found — using DRM/KMS backend (TTY mode)");
        run_udev();
    }
}

// ── DRM/KMS backend (production, bare-metal / TTY) ─────────────────────────

fn run_udev() {
    use smithay::backend::session::libseat::LibSeatSession;

    let (session, notifier) = match LibSeatSession::new() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create libseat session: {}", e);
            error!("Make sure seatd is running: sudo systemctl enable --now seatd");
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

    // Register session notifier
    loop_handle
        .insert_source(notifier, |event, _, state| {

            match event {}
                }", e);
                            }
                        }
                    }
                }
            }
        })
        .expect("Failed to insert session notifier");

    st.init_udev(session, &loop_handle);

    // Start XWayland
    if let Err(e) = st.init_xwayland(&loop_handle) {
        error!("XWayland failed to start: {} — X11 apps will not work", e);
    }

    st.init_ipc(&loop_handle);

    let socket = st.socket_name().to_string();
    info!("Compositor ready — WAYLAND_DISPLAY={}", socket);

    // Export environment so spawned apps can find us
    std::env::set_var("WAYLAND_DISPLAY", &socket);
    if let Some(xdisp) = st.x11_display {
        std::env::set_var("DISPLAY", format!(":{}", xdisp));
        info!("XWayland on DISPLAY=:{}", xdisp);
    }

    // Set XDG_SESSION_TYPE so apps use Wayland protocols
    std::env::set_var("XDG_SESSION_TYPE", "wayland");
    std::env::set_var("XDG_CURRENT_DESKTOP", "Blue");

    run_loop(event_loop, st);
}

// ── Winit backend (nested, dev/VM) ─────────────────────────────────────────

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

    if let Err(e) = st.init_xwayland(&loop_handle) {
        error!("XWayland failed: {} — X11 apps unavailable", e);
    }

    st.init_ipc(&loop_handle);

    let socket = st.socket_name().to_string();
    info!("Nested compositor ready — WAYLAND_DISPLAY={}", socket);
    std::env::set_var("WAYLAND_DISPLAY", &socket);
    std::env::set_var("XDG_SESSION_TYPE", "wayland");
    std::env::set_var("XDG_CURRENT_DESKTOP", "Blue");

    if let Some(xdisp) = st.x11_display {
        std::env::set_var("DISPLAY", format!(":{}", xdisp));
    }

    run_loop(event_loop, st);
}

// ── Main event loop ────────────────────────────────────────────────────────

fn run_loop(
    mut event_loop: calloop::EventLoop<'static, state::BlueState>,
    mut state: state::BlueState,
) {
    loop {
        // 16ms ≈ 60 fps budget for the compositor tick
        if let Err(e) = event_loop.dispatch(Some(Duration::from_millis(16)), &mut state) {
            error!("Event loop dispatch error: {}", e);
            break;
        }

        state.refresh();

        if state.should_exit() {
            info!("Compositor exit requested");
            break;
        }
    }

    info!("Blue Compositor stopped");
}
