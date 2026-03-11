use smithay::{
    backend::input::{
        AbsolutePositionEvent, ButtonState, Event, InputBackend, InputEvent,
        KeyState, KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::keyboard::{FilterResult, ModifiersState},
    utils::{Logical, Point, SERIAL_COUNTER},
};
use std::time::Instant;
use tracing::info;

use crate::ipc::{self, IpcMessage};

// xkeysym constants — raw u32 values, convert to Keysym for comparison
mod keys {
    pub const SUPER_L: u32  = 0xffeb;
    pub const SUPER_R: u32  = 0xffec;
    pub const KEY_1:   u32  = 0x31;
    pub const KEY_9:   u32  = 0x39;
    pub const KEY_D:   u32  = 0x64;
    pub const KEY_L:   u32  = 0x6c;
    pub const KEY_Q:   u32  = 0x71;
    pub const KEY_T:   u32  = 0x74;
    pub const KEY_TAB: u32  = 0xff09;
    pub const KEY_F4:  u32  = 0xffc1;
    pub const KEY_LEFT: u32 = 0xff51;
    pub const KEY_RIGHT:u32 = 0xff53;
    pub const KEY_UP:   u32 = 0xff52;
    pub const KEY_DOWN: u32 = 0xff54;
    pub const KEY_PRINT:u32 = 0xff61;
    pub const KEY_DELETE:u32= 0xffff;
}

const WIN_DOUBLE_MS: u128 = 350;

pub struct InputState {
    pub ptr_pos: Point<f64, Logical>,
    pub mods:    ModifiersState,
    last_win_release: Option<Instant>,
    win_held:         bool,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            ptr_pos:          Point::from((0.0, 0.0)),
            mods:             ModifiersState::default(),
            last_win_release: None,
            win_held:         false,
        }
    }

    pub fn handle<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        state: &mut crate::compositor::BlueState,
    ) {
        match event {
            InputEvent::Keyboard { event }             => self.on_key::<I>(event, state),
            InputEvent::PointerMotion { event }        => {
                let d = event.delta();
                self.ptr_pos.x = (self.ptr_pos.x + d.x).max(0.0);
                self.ptr_pos.y = (self.ptr_pos.y + d.y).max(0.0);
                self.forward_ptr(state);
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let raw = event.position();
                self.ptr_pos = Point::from((raw.x, raw.y));
                self.forward_ptr(state);
            }
            InputEvent::PointerButton { event }        => self.on_button::<I>(event, state),
            _ => {}
        }
    }

    fn forward_ptr(&self, state: &mut crate::compositor::BlueState) {
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(ptr) = state.pointer.clone() {
            ptr.motion(
                state,
                None,
                &smithay::input::pointer::MotionEvent {
                    location: self.ptr_pos,
                    serial,
                    time: 0,
                },
            );
        }
    }

    fn on_key<I: InputBackend>(
        &mut self,
        event: I::KeyboardKeyEvent,
        state: &mut crate::compositor::BlueState,
    ) {
        let serial    = SERIAL_COUNTER.next_serial();
        let time      = event.time_msec();
        let key_code  = event.key_code();
        let key_state = event.state();

        let Some(kb) = state.keyboard.clone() else { return };

        kb.input::<(), _>(
            state,
            key_code,
            key_state,
            serial,
            time,
            |cs, mods, keysym| {
                cs.input_state.mods = *mods;
                // Keysym.raw() zwraca u32 w smithay 0.6
                let raw = keysym.modified_sym().raw();

                let is_super = raw == keys::SUPER_L || raw == keys::SUPER_R;

                if is_super {
                    match key_state {
                        KeyState::Pressed => {
                            cs.input_state.win_held = true;
                        }
                        KeyState::Released => {
                            let now    = Instant::now();
                            let double = cs
                            .input_state
                            .last_win_release
                            .map(|t| now.duration_since(t).as_millis() < WIN_DOUBLE_MS)
                            .unwrap_or(false);

                            if double {
                                info!("Win+Win → fullscreen launcher");
                                ipc::send(&IpcMessage::ToggleFullscreenLauncher);
                                cs.input_state.last_win_release = None;
                            } else {
                                info!("Win → app menu");
                                ipc::send(&IpcMessage::ToggleAppMenu);
                                cs.input_state.last_win_release = Some(now);
                            }
                            cs.input_state.win_held = false;
                        }
                    }
                    return FilterResult::Intercept(());
                }

                if key_state != KeyState::Pressed {
                    return FilterResult::Forward;
                }

                let logo  = mods.logo;
                let ctrl  = mods.ctrl;
                let alt   = mods.alt;
                let shift = mods.shift;

                match (logo, ctrl, alt, shift, raw) {
                    (true, false, false, false, k) if k == keys::KEY_D => {
                        cs.workspaces.toggle_show_desktop();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_L => {
                        let _ = std::process::Command::new("loginctl").arg("lock-session").spawn();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_TAB => {
                        ipc::send(&IpcMessage::ShowWindowSwitcher);
                        FilterResult::Intercept(())
                    }
                    // Super+1..9 — przełącz workspace
                    (true, false, false, false, k)
                    if k >= keys::KEY_1 && k <= keys::KEY_9 =>
                    {
                        cs.workspaces.switch_to((k - keys::KEY_1) as usize);
                        FilterResult::Intercept(())
                    }
                    // Super+Shift+1..9 — przenieś okno na workspace
                    (true, false, false, true, k)
                    if k >= keys::KEY_1 && k <= keys::KEY_9 =>
                    {
                        cs.workspaces.move_focused_to((k - keys::KEY_1) as usize);
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_LEFT  => {
                        cs.workspaces.tile_focused_left();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_RIGHT => {
                        cs.workspaces.tile_focused_right();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_UP   => {
                        cs.workspaces.maximize_focused();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_DOWN  => {
                        cs.workspaces.restore_or_minimize_focused();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k)
                    if k == keys::KEY_Q || k == keys::KEY_F4 =>
                    {
                        cs.workspaces.close_focused();
                        FilterResult::Intercept(())
                    }
                    (false, false, true, false, k) if k == keys::KEY_F4 => {
                        cs.workspaces.close_focused();
                        FilterResult::Intercept(())
                    }
                    (false, true, true, false, k) if k == keys::KEY_T => {
                        let _ = std::process::Command::new("alacritty").spawn()
                        .or_else(|_| std::process::Command::new("foot").spawn())
                        .or_else(|_| std::process::Command::new("xterm").spawn());
                        FilterResult::Intercept(())
                    }
                    (false, true, true, false, k) if k == keys::KEY_DELETE => {
                        ipc::send(&IpcMessage::ShowLogoutDialog);
                        FilterResult::Intercept(())
                    }
                    (false, false, false, false, k) if k == keys::KEY_PRINT => {
                        let _ = std::process::Command::new("blue-screenshot").spawn();
                        FilterResult::Intercept(())
                    }
                    _ => FilterResult::Forward,
                }
            },
        );
    }

    fn on_button<I: InputBackend>(
        &mut self,
        event: I::PointerButtonEvent,
        state: &mut crate::compositor::BlueState,
    ) {
        let serial = SERIAL_COUNTER.next_serial();
        if event.state() == ButtonState::Pressed {
            if let Some(w) = state.workspaces.window_at(self.ptr_pos) {
                state.workspaces.focus_window(&w);
            }
        }
        if let Some(ptr) = state.pointer.clone() {
            ptr.button(
                state,
                &smithay::input::pointer::ButtonEvent {
                    serial,
                    time:   event.time_msec(),
                       button: event.button_code(),
                       state:  event.state(),
                },
            );
        }
    }
}
