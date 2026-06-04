use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::{DrmDevice, DrmDeviceFd, DrmNode},
        renderer::{
            damage::OutputDamageTracker,
            element::{
                surface::WaylandSurfaceRenderElement,
            },
            gles::{GlesRenderer, GlesTarget},
        },
        session::{Session, libseat::LibSeatSession},
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
        winit::{self, WinitGraphicsBackend, WinitEventLoop, WinitEvent},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{LoopHandle, timer::{Timer, TimeoutAction}},
        rustix::fs::OFlags,
        drm::control::{connector, Device as DrmControlDevice, ModeTypeFlags},
    },
    utils::{DeviceFd, Point, Size, Transform},
};
use std::{collections::HashMap, os::unix::io::OwnedFd, time::Duration};
use tracing::{error, info, warn};

use crate::state::{BackendData, BlueState, GpuDevice, UdevData, WinitData};

// ── Winit (nested/dev mode) ───────────────────────────────────────────────

pub fn init_winit(
    state: &mut BlueState,
    backend: WinitGraphicsBackend<GlesRenderer>,
    events: WinitEventLoop,
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
    let mode = OutputMode {
        size: Size::from((size.w as i32, size.h as i32)),
        refresh: 60_000,
    };
    output.change_current_state(Some(mode), Some(Transform::Normal), Some(Scale::Integer(1)), Some(Point::from((0, 0))));
    output.set_preferred(mode);
    state.space.map_output(&output, Point::from((0, 0)));
    let damage_tracker = OutputDamageTracker::from_output(&output);
    state.outputs.push(output.clone());
    state.backend_data = BackendData::Winit(Box::new(WinitData { backend, output, damage_tracker }));
    state.seat.add_keyboard(smithay::input::keyboard::XkbConfig::default(), 400, 30).expect("keyboard");
    let _ = state.seat.add_pointer();

    loop_handle.insert_source(events, |event, _, state| {
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
                if let BackendData::Winit(ref d) = state.backend_data {
                    let output = d.output.clone();
                    drop(d);
                    render_winit(state, &output);
                }
            }
            WinitEvent::Focus(_) => {}
        }
    }).expect("winit source");
}

pub fn render_winit(state: &mut BlueState, output: &Output) {
    // Phase 1: collect render elements (borrow ends before render)
    let elements = {
        let BackendData::Winit(ref mut d) = state.backend_data else { return };
        let (renderer, _) = match d.backend.bind() {
            Ok(r) => r,
            Err(e) => { error!("bind: {}", e); return; }
        };
        // SpaceRenderElements wraps WaylandSurfaceRenderElement - use the correct type
        use smithay::desktop::space::SpaceRenderElements;
        let elems: Vec<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>> =
            state.space.render_elements_for_output(renderer, output, 1.0, 1.0);
        elems
    };

    // Phase 2: render with fresh borrow
    let BackendData::Winit(ref mut d) = state.backend_data else { return };
    let (renderer, mut frame) = match d.backend.bind() {
        Ok(r) => r,
        Err(e) => { error!("bind2: {}", e); return; }
    };
    if let Err(e) = d.damage_tracker.render_output(renderer, &mut frame, 0, &elements, [0.08_f32, 0.10, 0.15, 1.0]) {
        warn!("render_output: {:?}", e);
    }
    drop(frame);
    if let Err(e) = d.backend.submit(None) { warn!("submit: {:?}", e); }
    d.backend.window().request_redraw();
}

// ── Udev/DRM (TTY mode) ────────────────────────────────────────────────────

pub fn init_udev(
    state: &mut BlueState,
    mut session: LibSeatSession,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    // Session trait must be in scope for .seat()
    let seat_name = session.seat();
    info!("udev backend, seat: {}", seat_name);

    // primary_gpu returns PathBuf — convert to DrmNode
    let primary_path = primary_gpu(&seat_name)
        .ok().flatten()
        .or_else(|| all_gpus(&seat_name).ok().and_then(|v| v.into_iter().next()))
        .expect("No GPU found");
    let primary_node = DrmNode::from_path(&primary_path).expect("DrmNode");
    info!("Primary GPU: {:?}", primary_node);

    let udev_backend = UdevBackend::new(&seat_name).expect("udev backend");
    let mut devices: HashMap<DrmNode, GpuDevice> = HashMap::new();

    if let Ok(gpu) = open_gpu(&primary_node, &mut session) {
        scan_drm_outputs(state, &gpu.drm, primary_node, loop_handle);
        devices.insert(primary_node, gpu);
    }

    state.backend_data = BackendData::Udev(Box::new(UdevData {
        session, primary_gpu: primary_node, devices,
    }));
    state.seat.add_keyboard(smithay::input::keyboard::XkbConfig::default(), 400, 30).expect("kb");
    let _ = state.seat.add_pointer();

    loop_handle.insert_source(udev_backend, |event, _, state| {
        match event {
            UdevEvent::Added { path, .. } => {
                if let Ok(node) = DrmNode::from_path(&path) {
                    if let BackendData::Udev(ref mut data) = state.backend_data {
                        let mut sess = data.session.clone();
                        if let Ok(gpu) = open_gpu(&node, &mut sess) {
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

    // Render timer
    loop_handle.insert_source(
        Timer::from_duration(Duration::from_millis(16)),
        |_, _, state| { state.refresh(); TimeoutAction::ToDuration(Duration::from_millis(16)) },
    ).expect("render timer");
}

fn open_gpu(node: &DrmNode, session: &mut LibSeatSession) -> Result<GpuDevice, Box<dyn std::error::Error>> {
    // Session trait in scope via import above
    let path = node.dev_path().ok_or("no dev path")?;
    let owned_fd: OwnedFd = session.open(&path, OFlags::empty())
        .map_err(|e| format!("session.open: {}", e))?;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(owned_fd));
    // DrmDevice::new returns (DrmDevice, DrmDeviceNotifier)
    let (drm, _notifier) = DrmDevice::new(drm_fd.clone(), true)
        .map_err(|e| format!("DrmDevice: {}", e))?;
    let gbm = GbmDevice::new(drm_fd)
        .map_err(|e| format!("GbmDevice: {}", e))?;
    Ok(GpuDevice { drm, gbm })
}

fn scan_drm_outputs(
    state: &mut BlueState,
    drm: &DrmDevice,
    node: DrmNode,
    _loop_handle: &LoopHandle<'static, BlueState>,
) {
    let Ok(resources) = drm.resource_handles() else { return };
    let mut x_off = 0i32;
    for conn_handle in resources.connectors() {
        let Ok(conn) = drm.get_connector(*conn_handle, false) else { continue };
        if conn.state() != connector::State::Connected { continue; }
        let mode = conn.modes().iter()
            .filter(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
            .max_by_key(|m| m.vrefresh())
            .or_else(|| conn.modes().first())
            .copied();
        let Some(mode) = mode else { continue };
        let (w, h) = mode.size();
        // connector::Handle doesn't have  — use u32 from handle
        let conn_id = u32::from(*conn_handle);
        let name = format!("{}-{}", conn.interface() as u8, conn_id);
        let phys = conn.size().unwrap_or((0, 0));
        let output = Output::new(name.clone(), PhysicalProperties {
            size: Size::from((phys.0 as i32, phys.1 as i32)),
            subpixel: Subpixel::Unknown,
            make: "Unknown".to_string(),
            model: name,
            serial_number: String::new(),
        });
        let sm = OutputMode { size: Size::from((w as i32, h as i32)), refresh: mode.vrefresh() as i32 * 1000 };
        output.change_current_state(Some(sm), Some(Transform::Normal), Some(Scale::Integer(1)), Some(Point::from((x_off, 0))));
        output.set_preferred(sm);
        state.space.map_output(&output, Point::from((x_off, 0)));
        state.outputs.push(output);
        x_off += w as i32;
    }
}
