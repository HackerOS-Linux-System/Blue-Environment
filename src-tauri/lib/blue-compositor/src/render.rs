use calloop::LoopHandle;
use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmNode},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::OutputDamageTracker,
            gles::GlesRenderer,
        },
        session::{libseat::LibSeatSession, Session},
        udev::{primary_gpu, UdevBackend, UdevEvent},
        winit::{self, WinitGraphicsBackend},
    },
    input::keyboard::XkbConfig,
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::timer::{TimeoutAction, Timer},
        drm::control::{connector, Device as DrmControlDevice, ModeTypeFlags},
        input::Libinput,
    },
    utils::{DeviceFd, Point, Size, Transform},
};
use std::{collections::HashMap, os::unix::io::FromRawFd, time::Duration};
use tracing::{error, info, warn};

use crate::state::BlueState;

pub struct UdevData {
    pub session: LibSeatSession,
    pub primary_gpu: DrmNode,
    pub devices: HashMap<DrmNode, GpuDevice>,
}

pub struct GpuDevice {
    pub drm: DrmDevice,
    pub gbm: GbmDevice<DrmDeviceFd>,
}

pub struct WinitData {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub output: Output,
    pub damage_tracker: OutputDamageTracker,
}

// ── DRM/KMS udev backend ───────────────────────────────────────────────────

pub fn init_udev(
    state: &mut BlueState,
    session: LibSeatSession,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    let seat_name = session.seat();
    info!("Initializing udev backend on seat: {}", seat_name);

    let primary_gpu_path = match primary_gpu(&seat_name) {
        Ok(Some(p)) => p,
        Ok(None) => {
            error!("No primary GPU found");
            return;
        }
        Err(e) => {
            error!("GPU detection error: {}", e);
            return;
        }
    };

    let primary_gpu = match DrmNode::from_path(&primary_gpu_path) {
        Ok(n) => n,
        Err(e) => {
            error!("Failed to get DRM node: {}", e);
            return;
        }
    };

    info!("Primary GPU: {:?}", primary_gpu);

    state.backend_data = crate::state::BackendData::Udev(Box::new(UdevData {
        session: session.clone(),
                                                                  primary_gpu,
                                                                  devices: HashMap::new(),
    }));

    let udev_backend = match UdevBackend::new(&seat_name) {
        Ok(b) => b,
        Err(e) => {
            error!("Failed to create udev backend: {}", e);
            return;
        }
    };

    for (_, path) in udev_backend.device_list() {
        if let Ok(node) = DrmNode::from_path(&path) {
            add_gpu_device(state, node, &path, loop_handle);
        }
    }

    let lh = loop_handle.clone();
    loop_handle
    .insert_source(udev_backend, move |event, _, state| match event {
        UdevEvent::Added { path, .. } => {
            if let Ok(node) = DrmNode::from_path(&path) {
                add_gpu_device(state, node, &path, &lh);
            }
        }
        _ => {}
    })
    .expect("Failed to insert udev source");

    // libinput – źródło zdarzeń wejścia
    let mut libinput_ctx = Libinput::new_with_udev(LibinputSessionInterface::from(session));
    libinput_ctx.udev_assign_seat(&seat_name).unwrap();
    loop_handle
    .insert_source(LibinputInputBackend::new(libinput_ctx), |event, _, state| {
        crate::input::handle_input(state, event);
    })
    .expect("Failed to insert libinput source");

    // Dodanie urządzeń wejścia do Seatu
    state
    .seat
    .add_keyboard(XkbConfig::default(), 600, 25)
    .expect("Failed to add keyboard");
    // add_pointer() zwraca PointerHandle, a nie Result – usuwamy .expect()
    let _ = state.seat.add_pointer();

    loop_handle
    .insert_source(Timer::from_duration(Duration::from_millis(16)), |_, _, state| {
        let outputs = state.outputs.clone();
        for out in outputs {
            render_output(state, &out);
        }
        TimeoutAction::ToDuration(Duration::from_millis(16))
    })
    .expect("Failed to insert render timer");
}

fn add_gpu_device(
    state: &mut BlueState,
    node: DrmNode,
    path: &std::path::Path,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    let session = match &mut state.backend_data {
        crate::state::BackendData::Udev(d) => &mut d.session,
        _ => return,
    };

    let owned_fd = match session.open(
        path,
        smithay::reexports::rustix::fs::OFlags::RDWR
        | smithay::reexports::rustix::fs::OFlags::CLOEXEC
        | smithay::reexports::rustix::fs::OFlags::NONBLOCK,
    ) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open DRM device: {}", e);
            return;
        }
    };

    let raw_fd = std::os::unix::io::IntoRawFd::into_raw_fd(owned_fd);
    let drm_fd = unsafe { DrmDeviceFd::new(DeviceFd::from_raw_fd(raw_fd)) };

    let (drm, notifier) = match DrmDevice::new(drm_fd.clone(), true) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to create DRM device: {}", e);
            return;
        }
    };
    let gbm = match GbmDevice::new(drm_fd) {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to create GBM device: {}", e);
            return;
        }
    };

    loop_handle
    .insert_source(notifier, move |event, _, _state| {
        if let DrmEvent::VBlank(_) = event {}
    })
    .expect("Failed to insert DRM source");

    init_drm_outputs(state, &drm);

    if let crate::state::BackendData::Udev(d) = &mut state.backend_data {
        d.devices.insert(node, GpuDevice { drm, gbm });
    }
}

fn init_drm_outputs(state: &mut BlueState, drm: &DrmDevice) {
    let res: smithay::reexports::drm::control::ResourceHandles = match drm.resource_handles() {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to get DRM resources: {}", e);
            return;
        }
    };

    for connector in res.connectors() {
        let conn_info: smithay::reexports::drm::control::connector::Info =
        match drm.get_connector(*connector, false) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if conn_info.state() != connector::State::Connected {
            continue;
        }

        let mode: Option<&smithay::reexports::drm::control::Mode> = conn_info
        .modes()
        .iter()
        .max_by_key(|m| {
            let preferred = m.mode_type().contains(ModeTypeFlags::PREFERRED) as u64;
            let area = m.size().0 as u64 * m.size().1 as u64;
            (preferred << 32) | (area * m.vrefresh() as u64)
        });

        let mode = match mode {
            Some(m) => m,
            None => continue,
        };
        let (w, h) = mode.size();
        info!("Connector {:?}: {}x{}@{}Hz", connector, w, h, mode.vrefresh());

        let output = Output::new(
            format!("{:?}", conn_info.interface()),
                PhysicalProperties {
                    size: conn_info
                    .size()
                    .map(|(pw, ph)| Size::from((pw as i32, ph as i32)))
                    .unwrap_or_default(),
                                 subpixel: Subpixel::Unknown,
                                 make: "Blue".to_string(),
                                 model: "Compositor".to_string(),
                                 serial_number: String::new(),
                },
        );

        let smithay_mode = OutputMode {
            size: Size::from((w as i32, h as i32)),
            refresh: mode.vrefresh() as i32 * 1000,
        };
        output.change_current_state(
            Some(smithay_mode),
                                    Some(Transform::Normal),
                                    Some(Scale::Integer(1)),
                                    Some(Point::from((0, 0))),
        );
        output.set_preferred(smithay_mode);
        state.space.map_output(&output, Point::from((0, 0)));
        state.outputs.push(output);
        break; // one output for now
    }
}

// ── Winit backend ──────────────────────────────────────────────────────────

pub fn init_winit(
    state: &mut BlueState,
    backend: WinitGraphicsBackend<GlesRenderer>,
    events: winit::WinitEventLoop,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    let size = backend.window_size();
    info!("Winit window: {}x{}", size.w, size.h);

    let output = Output::new(
        "winit".to_string(),
                             PhysicalProperties {
                                 size: Size::from((0, 0)),
                             subpixel: Subpixel::Unknown,
                             make: "Blue".to_string(),
                             model: "Winit".to_string(),
                             serial_number: String::new(),
                             },
    );

    let mode = OutputMode {
        size: Size::from((size.w as i32, size.h as i32)),
        refresh: 60_000,
    };
    output.change_current_state(
        Some(mode),
                                Some(Transform::Normal),
                                Some(Scale::Integer(1)),
                                Some(Point::from((0, 0))),
    );
    output.set_preferred(mode);
    state.space.map_output(&output, Point::from((0, 0)));

    let damage_tracker = OutputDamageTracker::from_output(&output);
    state.backend_data = crate::state::BackendData::Winit(Box::new(WinitData {
        backend,
        output: output.clone(),
                                                                   damage_tracker,
    }));
    state.outputs.push(output);

    // Dodanie urządzeń wejścia do Seatu
    state
    .seat
    .add_keyboard(XkbConfig::default(), 600, 25)
    .expect("Failed to add keyboard");
    // add_pointer() zwraca PointerHandle, a nie Result – usuwamy .expect()
    let _ = state.seat.add_pointer();

    loop_handle
    .insert_source(events, |event, _, state| {
        use winit::WinitEvent;
        match event {
            WinitEvent::Resized { size, .. } => {
                if let crate::state::BackendData::Winit(d) = &mut state.backend_data {
                    let m = OutputMode {
                        size: Size::from((size.w as i32, size.h as i32)),
                   refresh: 60_000,
                    };
                    d.output.change_current_state(Some(m), None, None, None);
                    d.damage_tracker = OutputDamageTracker::from_output(&d.output);
                }
            }
            WinitEvent::Input(event) => crate::input::handle_input(state, event),
                   WinitEvent::CloseRequested => state.should_exit = true,
                   WinitEvent::Redraw => {
                       let output = match &state.backend_data {
                           crate::state::BackendData::Winit(d) => d.output.clone(),
                   _ => return,
                       };
                       render_output(state, &output);
                   }
                   _ => {}
        }
    })
    .expect("Failed to insert winit source");
}

// ── Common render path ─────────────────────────────────────────────────────

pub fn render_output(state: &mut BlueState, output: &Output) {
    if let crate::state::BackendData::Winit(ref mut d) = state.backend_data {
        let _elements = {
            let (renderer, mut target) = d.backend.bind().expect("Failed to bind winit target");
            let elements = state
            .space
            .render_elements_for_output(renderer, output, 1.0)
            .unwrap_or_default();

            let _ = d.damage_tracker.render_output(
                renderer,
                &mut target,
                0,
                &elements,
                [0.08, 0.10, 0.15, 1.0],
            );
            elements
        };
        d.backend.submit(None).ok();
        d.backend.window().request_redraw();
    }
}
