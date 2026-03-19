use smithay::{
    backend::input::{
        Axis, AxisSource, ButtonState, InputBackend, InputEvent,
        KeyState, KeyboardKeyEvent,
        PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
        PointerMotionAbsoluteEvent,
    },
    desktop::WindowSurfaceType,
    input::{
        keyboard::{FilterResult, Keysym},
        pointer::{
            AxisFrame, ButtonEvent,
            GrabStartData as PointerGrabStartData,
            MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
        },
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::seat::WaylandFocus,
};

use crate::state::BlueState;

pub fn handle_input<B: InputBackend>(state: &mut BlueState, event: InputEvent<B>) {
    match event {
        InputEvent::Keyboard { event } => handle_keyboard(state, &event),
        InputEvent::PointerMotion { event } => handle_pointer_motion(state, &event),
        InputEvent::PointerMotionAbsolute { event } => handle_pointer_motion_abs(state, &event),
        InputEvent::PointerButton { event } => handle_pointer_button(state, &event),
        InputEvent::PointerAxis { event } => handle_pointer_axis(state, &event),
        _ => {}
    }
}

fn handle_keyboard<B: InputBackend, E: KeyboardKeyEvent<B>>(state: &mut BlueState, event: &E) {
    let serial = SERIAL_COUNTER.next_serial();
    let keyboard = state.seat.get_keyboard().unwrap();
    keyboard.input(
        state,
        event.key_code(),
                   event.state(),
                   serial,
                   event.time_msec(),
                   |state, mods, handle| {
                       let sym = handle.modified_sym();
                       if mods.logo && event.state() == KeyState::Pressed {
                           let ws = match sym {
                               Keysym::_1 => Some(0usize),
                   Keysym::_2 => Some(1),
                   Keysym::_3 => Some(2),
                   Keysym::_4 => Some(3),
                   _ => None,
                           };
                           if let Some(idx) = ws {
                               state.current_workspace = idx;
                               return FilterResult::Intercept(());
                           }
                       }
                       if mods.alt && sym == Keysym::F4 && event.state() == KeyState::Pressed {
                           if let Some(surface) = state.seat.get_keyboard().unwrap().current_focus() {
                               if let Some(w) = state.window_by_surface(&surface) {
                                   if let Some(t) = w.toplevel() { t.send_close(); }
                               }
                           }
                           return FilterResult::Intercept(());
                       }
                       FilterResult::Forward
                   },
    );
}

fn handle_pointer_motion<B: InputBackend, E: PointerMotionEvent<B>>(
    state: &mut BlueState,
    event: &E,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let delta = event.delta();

    // Get output bounds without keeping a borrow on state.space
    let (min_x, min_y, max_x, max_y) = {
        let bounds = state
        .space
        .outputs()
        .next()
        .and_then(|o| state.space.output_geometry(o))
        .unwrap_or(smithay::utils::Rectangle::from_loc_and_size((0, 0), (1920, 1080)));
        (
            bounds.loc.x as f64,
         bounds.loc.y as f64,
         (bounds.loc.x + bounds.size.w) as f64,
         (bounds.loc.y + bounds.size.h) as f64,
        )
    };

    state.pointer_location.x = (state.pointer_location.x + delta.x).clamp(min_x, max_x);
    state.pointer_location.y = (state.pointer_location.y + delta.y).clamp(min_y, max_y);

    update_pointer_focus(state, serial, event.time_msec());
}

fn handle_pointer_motion_abs<B: InputBackend, E: PointerMotionAbsoluteEvent<B>>(
    state: &mut BlueState,
    event: &E,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let size = {
        state
        .space
        .outputs()
        .next()
        .and_then(|o| state.space.output_geometry(o))
        .map(|g| g.size)
        .unwrap_or((1920, 1080).into())
    };
    state.pointer_location = event.position_transformed(size);
    update_pointer_focus(state, serial, event.time_msec());
}

fn update_pointer_focus(state: &mut BlueState, serial: smithay::utils::Serial, time: u32) {
    let pointer = state.seat.get_pointer().unwrap();
    let pos = state.pointer_location;

    // Collect surface + location without holding space borrow
    let focus: Option<(WlSurface, Point<f64, Logical>)> = state
    .space
    .element_under(pos)
    .and_then(|(win, win_loc)| {
        let rel = pos - win_loc.to_f64();
        win.surface_under(rel, WindowSurfaceType::ALL)
        .map(|(s, sp)| (s, (win_loc + sp).to_f64()))
    });

    pointer.motion(state, focus, &MotionEvent { location: pos, serial, time });
    pointer.frame(state);
}

fn handle_pointer_button<B: InputBackend, E: PointerButtonEvent<B>>(
    state: &mut BlueState,
    event: &E,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let pos = state.pointer_location;

    if event.state() == ButtonState::Pressed {
        // Clone to avoid borrow conflict
        let maybe_window = state
        .space
        .element_under(pos)
        .map(|(w, _)| w.clone());

        if let Some(window) = maybe_window {
            state.space.raise_element(&window, true);
            let keyboard = state.seat.get_keyboard().unwrap();
            if let Some(surface) = window.wl_surface() {
                keyboard.set_focus(state, Some(surface.into_owned()), serial);
            }
        } else {
            let keyboard = state.seat.get_keyboard().unwrap();
            keyboard.set_focus(state, Option::<WlSurface>::None, serial);
        }
    }

    let pointer = state.seat.get_pointer().unwrap();
    pointer.button(
        state,
        &ButtonEvent {
            button: event.button_code(),
                   state: event.state(),
                   serial,
                   time: event.time_msec(),
        },
    );
    pointer.frame(state);
}

fn handle_pointer_axis<B: InputBackend, E: PointerAxisEvent<B>>(
    state: &mut BlueState,
    event: &E,
) {
    let pointer = state.seat.get_pointer().unwrap();
    let mut frame = AxisFrame::new(event.time_msec()).source(AxisSource::Wheel);
    for axis in [Axis::Horizontal, Axis::Vertical] {
        if let Some(v) = event.amount(axis) {
            frame = frame
            .relative_direction(axis, event.relative_direction(axis))
            .value(axis, v);
            if let Some(d) = event.amount_v120(axis) {
                frame = frame.v120(axis, d as i32);
            }
        }
    }
    pointer.axis(state, frame);
    pointer.frame(state);
}

// ── Move grab ─────────────────────────────────────────────────────────────

pub struct MoveGrab {
    pub start_data: PointerGrabStartData<BlueState>,
    pub window: smithay::desktop::Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<BlueState> for MoveGrab {
    fn motion(
        &mut self,
        data: &mut BlueState,
        handle: &mut PointerInnerHandle<'_, BlueState>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
              event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        let delta = event.location - self.start_data.location;
        let new_loc = self.initial_window_location + delta.to_i32_round();
        data.space.map_element(self.window.clone(), new_loc, true);
    }

    fn relative_motion(
        &mut self,
        data: &mut BlueState,
        handle: &mut PointerInnerHandle<'_, BlueState>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
                       event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut BlueState,
        handle: &mut PointerInnerHandle<'_, BlueState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        // Release on any button up
        if event.state == ButtonState::Released {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(&mut self, data: &mut BlueState, handle: &mut PointerInnerHandle<'_, BlueState>, details: AxisFrame) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut BlueState, handle: &mut PointerInnerHandle<'_, BlueState>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GestureSwipeBeginEvent) {}
    fn gesture_swipe_update(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GestureSwipeUpdateEvent) {}
    fn gesture_swipe_end(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GestureSwipeEndEvent) {}
    fn gesture_pinch_begin(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GesturePinchBeginEvent) {}
    fn gesture_pinch_update(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GesturePinchUpdateEvent) {}
    fn gesture_pinch_end(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GesturePinchEndEvent) {}
    fn gesture_hold_begin(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GestureHoldBeginEvent) {}
    fn gesture_hold_end(&mut self, _: &mut BlueState, _: &mut PointerInnerHandle<'_, BlueState>, _: &smithay::input::pointer::GestureHoldEndEvent) {}

    fn start_data(&self) -> &PointerGrabStartData<BlueState> { &self.start_data }
    fn unset(&mut self, _: &mut BlueState) {}
}

pub fn start_move_grab(
    state: &mut BlueState,
    window: smithay::desktop::Window,
    start_data: PointerGrabStartData<BlueState>,
    _serial: smithay::utils::Serial,
) {
    let initial = state.space.element_location(&window).unwrap_or_default();
    let grab = MoveGrab { start_data, window, initial_window_location: initial };
    state.seat.get_pointer().unwrap().set_grab(
        state,
        grab,
        SERIAL_COUNTER.next_serial(),
                                               smithay::input::pointer::Focus::Clear,
    );
}
