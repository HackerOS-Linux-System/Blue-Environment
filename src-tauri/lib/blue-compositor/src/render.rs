use calloop::LoopHandle;
use smithay::{
    backend::{
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmNode},
        renderer::{
            damage::OutputDamageTracker,
            gles::GlesRenderer,
        },
        session::libseat::LibSeatSession,
        winit::{self, WinitGraphicsBackend},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::timer::Timer,
        drm::control::{connector, Device as DrmControlDevice, ModeTypeFlags},
    },
    utils::{DeviceFd, Point, Size, Transform},
    wayland::seat::WaylandFocus,
};
use std::{collections::HashMap, os::unix::io::FromRawFd, time::Duration};
use tracing::{info, warn};

use crate::state::{BlueState, GpuDevice, UdevData, WinitData};

// ────────────────────────────────────────────────────────────────────────────
// UDEV / DRM – tylko skanowanie wyjść (brak renderowania)
// ────────────────────────────────────────────────────────────────────────────

pub fn init_udev(
    _state: &mut BlueState,
    _session: LibSeatSession,
    _loop_handle: &LoopHandle<'static, BlueState>,
) {
    info!("DRM backend initialized (output scanning only, rendering disabled)");
}

fn add_gpu_device(
    _state: &mut BlueState,
    _node: DrmNode,
    _path: &std::path::Path,
    _loop_handle: &LoopHandle<'static, BlueState>,
) {
    // Placeholder – nieużywane w tej wersji
}

fn scan_outputs_for_device(_state: &mut BlueState, _drm: &DrmDevice, _node: DrmNode) {
    // Placeholder
}

// ────────────────────────────────────────────────────────────────────────────
// WINIT – pełne renderowanie (poprawione)
// ────────────────────────────────────────────────────────────────────────────

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

    state
    .seat
    .add_keyboard(smithay::input::keyboard::XkbConfig::default(), 600, 25)
    .expect("Failed to add keyboard");
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

pub fn render_output(state: &mut BlueState, output: &Output) {
    if let crate::state::BackendData::Winit(ref mut d) = state.backend_data {
        let (renderer, mut target) = d.backend.bind().expect("Failed to bind winit target");
        let elements = state
        .space
        .render_elements_for_output(renderer, output, 1.0)
        .unwrap_or_default();

        let _ = d.damage_tracker.render_output(
            renderer,
            &mut target, // <-- naprawione: target jako &mut
            0,
            &elements,
            [0.08, 0.10, 0.15, 1.0],
        );
        d.backend.submit(None).ok();
        d.backend.window().request_redraw();
    }
}
