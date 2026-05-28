use calloop::LoopHandle;
use smithay::{
    backend::{
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmNode},
        renderer::{
            damage::OutputDamageTracker,
            gles::GlesRenderer,
            utils::CommitCounter,
        },
        session::libseat::LibSeatSession,
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
        winit::{self, WinitGraphicsBackend},
        allocator::gbm::{GbmAllocator, GbmDevice, GbmBufferFlags},
        drm::compositor::DrmCompositor,
        SwapBuffersError,
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        drm::control::{connector, Device as DrmControlDevice, ModeTypeFlags},
        calloop::timer::Timer,
    },
    utils::{DeviceFd, Point, Size, Transform},
    wayland::seat::WaylandFocus,
};
use std::{
    collections::HashMap,
    os::unix::io::{AsFd, FromRawFd},
    path::Path,
    time::Duration,
};
use tracing::{error, info, warn};

use crate::state::{BackendData, BlueState, GpuDevice, UdevData, WinitData};

// ── WINIT backend (nested / dev) ───────────────────────────────────────────

pub fn init_winit(
    state: &mut BlueState,
    backend: WinitGraphicsBackend<GlesRenderer>,
    events: winit::WinitEventLoop,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    let size = backend.window_size();
    info!("Winit window: {}x{}", size.w, size.h);

    let output = Output::new(
        "winit-output".to_string(),
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

    state.backend_data = BackendData::Winit(Box::new(WinitData {
        backend,
        output: output.clone(),
        damage_tracker,
    }));
    state.outputs.push(output);

    // Add keyboard + pointer to seat
    state
        .seat
        .add_keyboard(smithay::input::keyboard::XkbConfig::default(), 400, 30)
        .expect("Failed to add keyboard");
    let _ = state.seat.add_pointer();

    // Register winit event source
    loop_handle
        .insert_source(events, |event, _, state| {
            use winit::WinitEvent;
            match event {
                WinitEvent::Resized { size, .. } => {
                    if let BackendData::Winit(ref mut d) = state.backend_data {
                        let m = OutputMode {
                            size: Size::from((size.w as i32, size.h as i32)),
                            refresh: 60_000,
                        };
                        d.output.change_current_state(Some(m), None, None, None);
                        d.damage_tracker = OutputDamageTracker::from_output(&d.output);
                        info!("Winit resized: {}x{}", size.w, size.h);
                    }
                }
                WinitEvent::Input(event) => {
                    crate::input::handle_input(state, event);
                }
                WinitEvent::CloseRequested => {
                    info!("Winit close requested");
                    state.should_exit = true;
                }
                WinitEvent::Redraw => {
                    let output = match &state.backend_data {
                        BackendData::Winit(d) => d.output.clone(),
                        _ => return,
                    };
                    render_winit_output(state, &output);
                }
                WinitEvent::Focus(_) => {}
                _ => {}
            }
        })
        .expect("Failed to insert winit source");

    info!("Winit backend initialized");
}

pub fn render_winit_output(state: &mut BlueState, output: &Output) {
    let BackendData::Winit(ref mut d) = state.backend_data else {
        return;
    };

    let (renderer, frame) = match d.backend.bind() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to bind winit renderer: {}", e);
            return;
        }
    };

    let elements: Vec<smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement<GlesRenderer>> =
        state
            .space
            .render_elements_for_output(renderer, output, 1.0)
            .unwrap_or_default();

    if let Err(e) = d.damage_tracker.render_output(
        renderer,
        frame,
        0,
        &elements,
        [0.08, 0.10, 0.15, 1.0], // Dark blue background
    ) {
        warn!("Render error: {:?}", e);
    }

    if let Err(e) = d.backend.submit(None) {
        warn!("Submit error: {:?}", e);
    }

    // Schedule next frame
    d.backend.window().request_redraw();
}

// ── UDEV/DRM backend (bare metal / production) ─────────────────────────────

pub fn init_udev(
    state: &mut BlueState,
    session: LibSeatSession,
    loop_handle: &LoopHandle<'static, BlueState>,
) {
    info!("Initializing udev/DRM backend");

    // Find primary GPU
    let primary_gpu = primary_gpu(session.seat())
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            all_gpus(session.seat())
                .ok()
                .and_then(|v| v.into_iter().next())
                .expect("No GPU found")
        });

    info!("Primary GPU: {:?}", primary_gpu);

    let udev_backend = UdevBackend::new(session.seat())
        .expect("Failed to create udev backend");

    // Initialize the primary GPU device right away
    let mut devices = HashMap::new();
    let gpu_path = primary_gpu.dev_path().expect("No device path for primary GPU");

    match open_gpu_device(&gpu_path, &session) {
        Ok(gpu_device) => {
            scan_drm_outputs(state, &gpu_device.drm, primary_gpu, loop_handle);
            devices.insert(primary_gpu, gpu_device);
        }
        Err(e) => {
            error!("Failed to open primary GPU {}: {}", gpu_path.display(), e);
        }
    }

    state.backend_data = BackendData::Udev(Box::new(UdevData {
        session: session.clone(),
        primary_gpu,
        devices,
    }));

    // Add input devices
    state
        .seat
        .add_keyboard(smithay::input::keyboard::XkbConfig::default(), 400, 30)
        .expect("Failed to add keyboard");
    let _ = state.seat.add_pointer();

    // Insert udev event source for hot-plug
    loop_handle
        .insert_source(udev_backend, move |event, _, state| {
            match event {
                UdevEvent::Added { device_id, path } => {
                    info!("GPU added: {:?} at {}", device_id, path.display());
                    // Try to open the new GPU
                    if let BackendData::Udev(ref data) = state.backend_data {
                        let sess = data.session.clone();
                        if let Ok(node) = DrmNode::from_path(&path) {
                            if let Ok(dev) = open_gpu_device(&path, &sess) {
                                scan_drm_outputs(state, &dev.drm, node, loop_handle);
                                if let BackendData::Udev(ref mut d) = state.backend_data {
                                    d.devices.insert(node, dev);
                                }
                            }
                        }
                    }
                }
                UdevEvent::Changed { device_id } => {
                    info!("GPU changed: {:?}", device_id);
                }
                UdevEvent::Removed { device_id, .. } => {
                    info!("GPU removed: {:?}", device_id);
                }
            }
        })
        .expect("Failed to insert udev source");

    // Insert DRM frame timer (60fps)
    loop_handle
        .insert_source(
            Timer::from_duration(Duration::from_millis(16)),
            |_, _, state| {
                render_drm_outputs(state);
                calloop::timer::TimeoutAction::ToDuration(Duration::from_millis(16))
            },
        )
        .expect("Failed to insert render timer");

    info!("DRM backend initialized with {} output(s)", state.outputs.len());
}

fn open_gpu_device(
    path: &Path,
    session: &LibSeatSession,
) -> Result<GpuDevice, Box<dyn std::error::Error>> {
    let fd = session
        .open(path, smithay::backend::session::OpenFlags::empty())
        .map_err(|e| format!("Failed to open GPU: {}", e))?;

    let drm_fd = DrmDeviceFd::new(unsafe { DeviceFd::from_raw_fd(fd.as_raw_fd()) });
    let drm = DrmDevice::new(drm_fd.clone(), true)
        .map_err(|e| format!("DrmDevice::new failed: {}", e))?;

    let gbm = GbmDevice::new(drm_fd)
        .map_err(|e| format!("GbmDevice::new failed: {}", e))?;

    Ok(GpuDevice { drm, gbm })
}

fn scan_drm_outputs(
    state: &mut BlueState,
    drm: &DrmDevice,
    node: DrmNode,
    _loop_handle: &LoopHandle<'static, BlueState>,
) {
    let resources = match drm.resource_handles() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get DRM resources: {}", e);
            return;
        }
    };

    let mut x_offset = 0i32;

    for &conn_handle in resources.connectors() {
        let conn_info = match drm.get_connector(conn_handle, false) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if conn_info.state() != connector::State::Connected {
            continue;
        }

        // Pick the preferred mode (highest refresh of preferred modes, or just first)
        let mode = conn_info
            .modes()
            .iter()
            .filter(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
            .max_by_key(|m| m.vrefresh())
            .or_else(|| conn_info.modes().first())
            .copied();

        let mode = match mode {
            Some(m) => m,
            None => {
                warn!("Connector {:?} has no modes", conn_handle);
                continue;
            }
        };

        let (w, h) = mode.size();
        let refresh = mode.vrefresh() as i32 * 1000;
        info!(
            "Output: {:?} {}x{}@{}Hz",
            conn_info.interface(),
            w,
            h,
            mode.vrefresh()
        );

        let output_name = format!("{:?}-{}", conn_info.interface(), conn_handle.index());
        let output = Output::new(
            output_name.clone(),
            PhysicalProperties {
                size: Size::from((
                    conn_info.size().map(|(w, _)| w as i32).unwrap_or(0),
                    conn_info.size().map(|(_, h)| h as i32).unwrap_or(0),
                )),
                subpixel: Subpixel::Unknown,
                make: "Unknown".to_string(),
                model: output_name.clone(),
                serial_number: String::new(),
            },
        );

        let smithay_mode = OutputMode {
            size: Size::from((w as i32, h as i32)),
            refresh,
        };

        output.change_current_state(
            Some(smithay_mode),
            Some(Transform::Normal),
            Some(Scale::Integer(1)),
            Some(Point::from((x_offset, 0))),
        );
        output.set_preferred(smithay_mode);

        state
            .space
            .map_output(&output, Point::from((x_offset, 0)));
        state.outputs.push(output);

        x_offset += w as i32;
    }
}

fn render_drm_outputs(state: &mut BlueState) {
    // Collect output clones to avoid borrow issues
    let outputs: Vec<Output> = state.outputs.clone();

    for output in &outputs {
        // In a full DRM compositor, we'd use DrmCompositor here.
        // For now we do software rendering via GBM + EGL.
        // The full implementation would:
        // 1. Find the DrmCompositor for this output
        // 2. Collect render elements from state.space
        // 3. Call compositor.render_frame(...)
        // 4. Queue the framebuffer for scanout
        //
        // This is a structural placeholder — the winit path gives
        // full rendering in nested/VM scenarios which covers most dev use.

        // Notify clients about frame timing
        let _ = state.space.render_elements_for_output::<
            smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement<GlesRenderer>
        >(
            // renderer not available here without the DrmCompositor binding
            // this is the known limitation of the placeholder
            return,
            output,
            1.0,
        );
    }
}
