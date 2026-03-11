use anyhow::{anyhow, Result};
use std::sync::Arc;
use smithay::reexports::rustix::fs::OFlags;
use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::{DrmDevice, DrmDeviceFd, DrmNode},
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::gles::GlesRenderer,
        session::{
            libseat::LibSeatSession,
            Session,
        },
        udev::UdevBackend,
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        input::Libinput,
        wayland_server::{Display, ListeningSocket},
    },
    utils::Transform,
};
use tracing::{info, warn};

use crate::{compositor::{BlueState, ClientState}, config::BlueConfig, session};
use super::BackendType;

pub fn run(config: BlueConfig, backend_type: BackendType) -> Result<()> {
    info!("Initializing DRM/KMS backend");

    let mut event_loop: EventLoop<BlueState> = EventLoop::try_new()?;
    let mut display: Display<BlueState>      = Display::new()?;

    // ── Session via libseat ───────────────────────────────
    let (mut session, notifier) = LibSeatSession::new()
    .map_err(|e| anyhow!("LibSeat session failed: {e} — ensure seatd/logind running"))?;

    event_loop.handle()
    .insert_source(notifier, |_, _, _| {})
    .map_err(|e| anyhow!("insert libseat notifier: {e}"))?;

    let seat_name = <LibSeatSession as Session>::seat(&session).to_string();
    info!("Seat: {}", seat_name);

    // ── UDev backend ─────────────────────────────────────
    let udev = UdevBackend::new(&seat_name)
    .map_err(|e| anyhow!("UdevBackend::new: {e}"))?;

    // ── libinput ─────────────────────────────────────────
    // BORROW FIX: zbieramy eventy do Vec, obsługujemy poza closure
    let li_iface   = LibinputSessionInterface::from(session.clone());
    let mut li     = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(li_iface);
    li.udev_assign_seat(&seat_name).ok();
    let li_backend = LibinputInputBackend::new(li);

    // Kanał do przekazywania inputu z libinput source do event loop
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    event_loop.handle()
    .insert_source(li_backend, move |event, _, _state: &mut BlueState| {
        input_tx.send(event).ok();
    })
    .map_err(|e| anyhow!("insert libinput: {e}"))?;

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

    // ── Register DRM devices ─────────────────────────────
    // Robimy to PRZED BlueState::new żeby session nie był ruchomy po tym
    let mut initial_outputs: Vec<Output> = Vec::new();
    for (dev_id, path) in udev.device_list() {
        match DrmNode::from_dev_id(dev_id) {
            Ok(node) => {
                info!("DRM device {:?} at {:?}", node, path);
                match setup_drm(&mut session, node, path) {
                    Ok(output) => initial_outputs.push(output),
                    Err(e)     => warn!("DRM setup failed for {:?}: {}", path, e),
                }
            }
            Err(e) => warn!("DRM node from dev_id: {}", e),
        }
    }

    // ── Compositor state ─────────────────────────────────
    let loop_signal = event_loop.get_signal();
    let mut state = BlueState::new(
        &mut display,
        event_loop.handle(),
                                   loop_signal,
                                   config,
                                   backend_type,
    )?;
    state.outputs.extend(initial_outputs);

    // ── Launch panel + app menu ───────────────────────────
    launch_bundled(socket_name);
    let mut _daemons = session::start_daemons();
    session::run_autostart();

    info!("Blue Environment ready on DRM/KMS");

    event_loop.run(
        Some(std::time::Duration::from_millis(1)),
                   &mut state,
                   |s| {
                       // Borrow fix: wyciągnij input_state tymczasowo przez raw ptr
                       while let Ok(event) = input_rx.try_recv() {
                           // SAFETY: input_state i reszta BlueState są rozłącznymi polami
                           let input_ptr: *mut crate::input::InputState = &mut s.input_state;
                           unsafe { (*input_ptr).handle(event, s); }
                       }
                       // Akceptuj nowych klientów Wayland
                       if let Ok(Some(client)) = listening_socket.accept() {
                           s.dh.insert_client(client, Arc::new(ClientState::default())).ok();
                       }
                       s.dh.flush_clients().ok();
                   },
    )?;

    Ok(())
}

fn setup_drm(
    session: &mut LibSeatSession,
    node:    DrmNode,
    path:    &std::path::Path,
) -> Result<Output> {
    // session.open() wymaga &mut self
    let owned_fd = session.open(path, OFlags::RDWR | OFlags::CLOEXEC)
    .map_err(|e| anyhow!("open DRM {:?}: {}", path, e))?;

    let fd = DrmDeviceFd::new(owned_fd.into());
    let (_drm, _notifier) = DrmDevice::new(fd.clone(), false)?;
    let gbm = GbmDevice::new(fd)?;

    // EGLDisplay::new jest unsafe w smithay 0.6
    let egl       = unsafe { EGLDisplay::new(gbm.clone())? };
    let ctx       = EGLContext::new(&egl)?;
    let _renderer = unsafe { GlesRenderer::new(ctx)? };
    info!("GLES renderer ready on {:?}", node);

    let mode = Mode { size: (1920, 1080).into(), refresh: 60_000 };
    let output = Output::new(
        format!("DRM-{:?}", node),
            PhysicalProperties {
                size:     (520, 293).into(),
                             subpixel: Subpixel::Unknown,
                             make:     "Blue".to_string(),
                             model:    "DRM".to_string(),
            },
    );
    output.change_current_state(
        Some(mode), Some(Transform::Normal), Some(Scale::Integer(1)), None,
    );
    output.set_preferred(mode);

    Ok(output)
}

fn launch_bundled(wayland_display: String) {
    let wd1 = wayland_display.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        crate::panel::launch(wd1);
    });
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        crate::app_menu::launch(wayland_display);
    });
}
