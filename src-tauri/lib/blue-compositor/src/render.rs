use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::{DrmDevice, DrmDeviceFd, DrmNode},
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
        session::{libseat::LibSeatSession, Session},
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
        winit::{self, WinitGraphicsBackend},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::LoopHandle,
        drm::control::{connector, Device as DrmControlDevice, ModeTypeFlags},
        rustix::fs::OFlags,
    },
    utils::{DeviceFd, Point, Size, Transform},
};
use std::{collections::HashMap, os::unix::io::OwnedFd, time::Duration};
use tracing::{error, info, warn};

use crate::state::{BackendData, BlueState, GpuDevice, UdevData, WinitData};

// ── Winit ─────────────────────────────────────────────────────────────────

pub fn init_winit(
    state: &mut BlueState,
    backend: WinitGraphicsBackend<GlesRenderer>,
    events: winit::WinitEventLoop,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    let size = backend.window_size();
    let output = Output::new("winit".to_string(), PhysicalProperties {
        size: Size::from((0, 0)),
        subpixel: Subpixel::Unknown,
        make: "Blue".to_string(),
        model: "Winit".to_string(),
        serial_number: String::new(),
    });
    let mode = OutputMode { size: Size::from((size.w as i32, size.h as i32)), refresh: 60_000 };
    output.change_current_state(Some(mode), Some(Transform::Normal), Some(Scale::Integer(1)), Some(Point::from((0, 0))));
    output.set_preferred(mode);
    state.space.map_output(&output, Point::from((0, 0)));
    let damage_tracker = OutputDamageTracker::from_output(&output);
    state.outputs.push(output.clone());
    state.backend_data = BackendData::Winit(Box::new(WinitData { backend, output, damage_tracker }));
    state.seat.add_keyboard(smithay::input::keyboard::XkbConfig::default(), 400, 30).expect("keyboard");
    let _ = state.seat.add_pointer();

    loop_handle.insert_source(events, |event, _, state| {
        use winit::WinitEvent;
        match event {
            WinitEvent::Resized { size, .. } => {
                if let BackendData::Winit(ref mut d) = state.backend_data {
                    let m = OutputMode { size: Size::from((size.w as i32, size.h as i32)), refresh: 60_000 };
                    d.output.change_current_state(Some(m), None, None, None);
                    d.damage_tracker = OutputDamageTracker::from_output(&d.output);
                }
            }
            WinitEvent::Input(ev) => crate::input::handle_input(state, ev),
            WinitEvent::CloseRequested => { state.should_exit = true; }
            WinitEvent::Redraw => {
                let output = match &state.backend_data { BackendData::Winit(d) => d.output.clone(), _ => return };
                render_winit(state, &output);
            }
            WinitEvent::Focus(_) => {}
        }
    }).expect("winit source");
}

pub fn render_winit(state: &mut BlueState, output: &Output) {
    // Phase 1: collect elements (needs renderer) — do in block so borrow ends
    let elements = {
        let BackendData::Winit(ref mut d) = state.backend_data else { return };
        let (renderer, _frame) = match d.backend.bind() {
            Ok(r) => r,
            Err(e) => { error!("bind: {}", e); return }
        };
        // collect() to owned Vec — ends borrow on renderer
        state.space.render_elements_for_output(renderer, output, 1.0).unwrap_or_default()
    };
    // Phase 2: render + submit — fresh borrow
    let BackendData::Winit(ref mut d) = state.backend_data else { return };
    let (renderer, mut frame) = match d.backend.bind() {
        Ok(r) => r,
        Err(e) => { error!("bind2: {}", e); return }
    };
    if let Err(e) = d.damage_tracker.render_output(renderer, &mut frame, 0, &elements, [0.08, 0.10, 0.15, 1.0]) {
        warn!("render: {:?}", e);
    }
    drop(frame); // release borrow before submit
    if let Err(e) = d.backend.submit(None) { warn!("submit: {:?}", e); }
    d.backend.window().request_redraw();
}

// ── Udev/DRM ──────────────────────────────────────────────────────────────

pub fn init_udev(
    state: &mut BlueState,
    mut session: LibSeatSession,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    let seat_name = session.seat();
    info!("udev backend, seat: {}", seat_name);

    // primary_gpu returns PathBuf in this Smithay revision
    let primary_path = primary_gpu(&seat_name).ok().flatten()
        .unwrap_or_else(|| all_gpus(&seat_name).ok().and_then(|v| v.into_iter().next()).expect("No GPU"));
    let primary_node = DrmNode::from_path(&primary_path).expect("DrmNode from GPU path");
    info!("Primary GPU: {:?}", primary_node);

    let udev_backend = UdevBackend::new(&seat_name).expect("udev backend");
    let mut devices: HashMap<DrmNode, GpuDevice> = HashMap::new();

    if let Ok(gpu) = open_gpu(&primary_node, &mut session) {
        scan_outputs(state, &gpu);
        devices.insert(primary_node, gpu);
    }

    state.backend_data = BackendData::Udev(Box::new(UdevData { session, primary_gpu: primary_node, devices }));
    state.seat.add_keyboard(smithay::input::keyboard::XkbConfig::default(), 400, 30).expect("keyboard");
    let _ = state.seat.add_pointer();

    loop_handle.insert_source(udev_backend, |event, _, state| {
        match event {
            UdevEvent::Added { path, .. } => {
                if let Ok(node) = DrmNode::from_path(&path) {
                    if let BackendData::Udev(ref mut data) = state.backend_data {
                        let mut sess = data.session.clone();
                        if let Ok(gpu) = open_gpu(&node, &mut sess) {
                            scan_outputs(state, &gpu);
                            if let BackendData::Udev(ref mut d) = state.backend_data {
                                d.devices.insert(node, gpu);
                            }
                        }
                    }
                }
            }
            UdevEvent::Changed { .. } | UdevEvent::Removed { .. } => {}
        }
    }).expect("udev source");

    loop_handle.insert_source(
        smithay::reexports::calloop::timer::Timer::from_duration(Duration::from_millis(16)),
        |_, _, state| { state.refresh(); smithay::reexports::calloop::timer::TimeoutAction::ToDuration(Duration::from_millis(16)) },
    ).expect("render timer");
}

fn open_gpu(node: &DrmNode, session: &mut LibSeatSession) -> Result<GpuDevice, Box<dyn std::error::Error>> {
    let path = node.dev_path().ok_or("no dev path")?;
    let owned_fd: OwnedFd = session.open(&path, OFlags::empty()).map_err(|e| format!("open: {}", e))?;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(owned_fd));
    let (drm, _notifier) = DrmDevice::new(drm_fd.clone(), true).map_err(|e| format!("DrmDevice: {}", e))?;
    let gbm = GbmDevice::new(drm_fd).map_err(|e| format!("GbmDevice: {}", e))?;
    Ok(GpuDevice { drm, gbm })
}

fn scan_outputs(state: &mut BlueState, gpu: &GpuDevice) {
    let Ok(resources) = gpu.drm.resource_handles() else { return };
    let mut x_off = 0i32;
    for conn_handle in resources.connectors() {
        let Ok(conn) = gpu.drm.get_connector(*conn_handle, false) else { continue };
        if conn.state() != connector::State::Connected { continue; }
        let mode = conn.modes().iter().filter(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
            .max_by_key(|m| m.vrefresh()).or_else(|| conn.modes().first()).copied();
        let Some(mode) = mode else { continue };
        let (w, h) = mode.size();
        let name = format!("{}-{}", conn.interface() as u8, u32::from(*conn_handle));
        let output = Output::new(name.clone(), PhysicalProperties {
            size: Size::from((conn.size().map(|(pw,_)| pw as i32).unwrap_or(0), conn.size().map(|(_,ph)| ph as i32).unwrap_or(0))),
            subpixel: Subpixel::Unknown, make: "Unknown".to_string(), model: name, serial_number: String::new(),
        });
        let sm = OutputMode { size: Size::from((w as i32, h as i32)), refresh: mode.vrefresh() as i32 * 1000 };
        output.change_current_state(Some(sm), Some(Transform::Normal), Some(Scale::Integer(1)), Some(Point::from((x_off, 0))));
        output.set_preferred(sm);
        state.space.map_output(&output, Point::from((x_off, 0)));
        state.outputs.push(output);
        x_off += w as i32;
    }
}
