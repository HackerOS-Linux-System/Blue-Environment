use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event,
        InputBackend, InputEvent, KeyboardKeyEvent, PointerAxisEvent,
        PointerButtonEvent, PointerMotionEvent, PointerMotionAbsoluteEvent,
    },
    desktop::WindowSurface,
    input::{
        keyboard::{xkb, FilterResult, KeyboardHandle, Keysym, ModifiersState},
        pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent},
    },
    reexports::wayland_server::protocol::wl_pointer,
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::seat::WaylandFocus,
};
use tracing::debug;

use crate::state::BlueState;

pub fn handle_input<I: InputBackend>(state: &mut BlueState, event: InputEvent<I>) {
    match event {
        InputEvent::Keyboard { event } => handle_keyboard(state, &event),
        InputEvent::PointerMotion { event } => handle_pointer_motion(state, &event),
        InputEvent::PointerMotionAbsolute { event } => handle_pointer_motion_abs(state, &event),
        InputEvent::PointerButton { event } => handle_pointer_button(state, &event),
        InputEvent::PointerAxis { event } => handle_pointer_axis(state, &event),
        _ => {}
    }
}

fn handle_keyboard<I: InputBackend>(state: &mut BlueState, event: &I::KeyboardKeyEvent) {
    let serial = SERIAL_COUNTER.next_serial();
    let time = event.time_msec();
    let press_state = event.state();
    let Some(kb) = state.seat.get_keyboard() else { return };

    let action = kb.input::<KeyAction, _>(
        state,
        event.key_code(),
        press_state,
        serial,
        time,
        |state, mods, handle| {
            let keysym = handle.modified_sym();
            if press_state == smithay::backend::input::KeyState::Pressed {
                compositor_keybind(state, mods, keysym)
            } else {
                FilterResult::Forward
            }
        },
    );

    if let Some(KeyAction::Exit) = action {
        state.should_exit = true;
    }
}

#[derive(Debug)]
enum KeyAction { Exit, SwitchWorkspace(usize) }

fn compositor_keybind(
    state: &mut BlueState,
    mods: &ModifiersState,
    keysym: Keysym,
) -> FilterResult<KeyAction> {
    // Super + 1-4: switch workspace
    if mods.logo {
        match keysym {
            Keysym::_1 => return FilterResult::Intercept(KeyAction::SwitchWorkspace(0)),
            Keysym::_2 => return FilterResult::Intercept(KeyAction::SwitchWorkspace(1)),
            Keysym::_3 => return FilterResult::Intercept(KeyAction::SwitchWorkspace(2)),
            Keysym::_4 => return FilterResult::Intercept(KeyAction::SwitchWorkspace(3)),
            _ => {}
        }
    }
    // Ctrl+Alt+Backspace: exit
    if mods.ctrl && mods.alt && keysym == Keysym::BackSpace {
        return FilterResult::Intercept(KeyAction::Exit);
    }
    FilterResult::Forward
}

fn handle_pointer_motion<I: InputBackend>(state: &mut BlueState, event: &I::PointerMotionEvent) {
    let delta = event.delta();
    state.pointer_location += Point::from((delta.x, delta.y));
    clamp_pointer(state);
    update_pointer_focus(state, event.time_msec());
}

fn handle_pointer_motion_abs<I: InputBackend>(state: &mut BlueState, event: &I::PointerMotionAbsoluteEvent) {
    if let Some(output) = state.outputs.first() {
        let geo = state.space.output_geometry(output).unwrap_or_default();
        state.pointer_location = event.position_transformed(geo.size);
    }
    update_pointer_focus(state, event.time_msec());
}

fn clamp_pointer(state: &mut BlueState) {
    if let Some(output) = state.outputs.first() {
        let geo = state.space.output_geometry(output).unwrap_or_default();
        state.pointer_location.x = state.pointer_location.x.clamp(0.0, geo.size.w as f64);
        state.pointer_location.y = state.pointer_location.y.clamp(0.0, geo.size.h as f64);
    }
}

fn update_pointer_focus(state: &mut BlueState, time: u32) {
    let serial = SERIAL_COUNTER.next_serial();
    let loc = state.pointer_location;

    let under = state.space.element_under(loc).map(|(w, p)| {
        let wl = w.wl_surface().map(|s| (s.into_owned(), p));
        wl
    }).flatten();

    if let Some(ptr) = state.seat.get_pointer() {
        ptr.motion(state, under.clone(), &MotionEvent { location: loc, serial, time });
        ptr.frame(state);
    }
}

fn handle_pointer_button<I: InputBackend>(state: &mut BlueState, event: &I::PointerButtonEvent) {
    let serial = SERIAL_COUNTER.next_serial();
    let button = event.button_code();
    let button_state = event.state();

    // Focus window on click
    if button_state == ButtonState::Pressed {
        let loc = state.pointer_location;
        let under = state.space.element_under(loc).map(|(w, _)| w.clone());
        if let Some(win) = under {
            state.space.raise_element(&win, true);
            if let Some(surface) = win.wl_surface() {
                if let Some(kb) = state.seat.get_keyboard() {
                    kb.set_focus(state, Some(surface.into_owned()), serial);
                }
            }
        } else {
            if let Some(kb) = state.seat.get_keyboard() {
                kb.set_focus(state, Option::<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>::None, serial);
            }
        }
    }

    if let Some(ptr) = state.seat.get_pointer() {
        ptr.button(state, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
        ptr.frame(state);
    }
}

fn handle_pointer_axis<I: InputBackend>(state: &mut BlueState, event: &I::PointerAxisEvent) {
    if let Some(ptr) = state.seat.get_pointer() {
        let mut frame = AxisFrame::new(event.time_msec()).source(AxisSource::Wheel);
        let h = event.amount(Axis::Horizontal).unwrap_or(0.0);
        let v = event.amount(Axis::Vertical).unwrap_or(0.0);
        if h != 0.0 { frame = frame.value(Axis::Horizontal, h); }
        if v != 0.0 { frame = frame.value(Axis::Vertical, v); }
        ptr.axis(state, frame);
        ptr.frame(state);
    }
}
