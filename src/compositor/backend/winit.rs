use anyhow::{anyhow, Result};
use std::sync::Arc;
use smithay::{
    backend::{
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_server::{Display, ListeningSocket},
    },
    utils::Transform,
};
use tracing::info;

use crate::{compositor::{BlueState, ClientState}, config::BlueConfig, session};
use super::BackendType;

pub fn run(config: BlueConfig, backend_type: BackendType) -> Result<()> {
    info!("Initializing Winit backend (nested mode)");

    let mut event_loop: EventLoop<BlueState> = EventLoop::try_new()?;
    let mut display: Display<BlueState>      = Display::new()?;

    let (mut backend, mut winit_loop) = winit::init::<GlesRenderer>()
    .map_err(|e| anyhow!("winit init failed: {}", e))?;

    let mode = Mode { size: (1920, 1080).into(), refresh: 60_000 };
    let output = Output::new(
        "Blue-Virtual".to_string(),
                             PhysicalProperties {
                                 size:     (520, 293).into(),
                             subpixel: Subpixel::Unknown,
                             make:     "Blue Environment".to_string(),
                             model:    "Virtual".to_string(),
                             },
    );
    output.change_current_state(
        Some(mode), Some(Transform::Normal), Some(Scale::Integer(1)), None,
    );
    output.set_preferred(mode);

    // ── Wayland socket ────────────────────────────────────
    let listening_socket =
    ListeningSocket::bind_auto("wayland", 0usize..=255)
    .map_err(|e| anyhow!("Failed to create Wayland socket: {e}"))?;

    let socket_name = listening_socket
    .socket_name()
    .map(|n| n.to_string_lossy().to_string())
    .unwrap_or_else(|| "wayland-0".to_string());

    std::env::set_var("WAYLAND_DISPLAY", &socket_name);
    info!("Wayland socket: {}", socket_name);

    let loop_signal = event_loop.get_signal();
    let mut state = BlueState::new(
        &mut display,
        event_loop.handle(),
                                   loop_signal,
                                   config,
                                   backend_type,
    )?;
    state.outputs.push(output);

    // Kanał inputu — fix dla borrow checker (jak w udev.rs)
    let (input_tx, input_rx) = std::sync::mpsc::channel();

    let wd1 = socket_name.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        crate::panel::launch(wd1);
    });
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        crate::app_menu::launch(socket_name);
    });

    let mut _daemons = session::start_daemons();
    session::run_autostart();

    info!("Blue Environment ready (nested)");

    // Flaga zamknięcia okna
    let mut should_exit = false;

    event_loop.run(
        Some(std::time::Duration::from_millis(16)),
                   &mut state,
                   |state| {
                       if should_exit {
                           state.loop_signal.stop();
                           return;
                       }

                       // Akceptuj nowych klientów Wayland
                       if let Ok(Some(client)) = listening_socket.accept() {
                           state.dh.insert_client(client, Arc::new(ClientState::default())).ok();
                       }

                       // Borrow fix: wyciągnij input_state przez raw ptr
                       while let Ok(event) = input_rx.try_recv() {
                           // SAFETY: input_state i reszta BlueState są rozłącznymi polami
                           let input_ptr: *mut crate::input::InputState = &mut state.input_state;
                           unsafe { (*input_ptr).handle(event, state); }
                       }

                       backend.bind().ok();

                       // dispatch_new_events — nie matchujemy na prywatny PumpStatus
                       // Zamiast tego zbieramy WinitEvent::CloseRequested jeśli wystąpi
                       winit_loop.dispatch_new_events(|event| {
                           match event {
                               WinitEvent::Input(input) => { input_tx.send(input).ok(); }
                               _ => {}
                           }
                       });

                       backend.submit(None).ok();
                       state.dh.flush_clients().ok();
                   },
    )?;

    Ok(())
}
