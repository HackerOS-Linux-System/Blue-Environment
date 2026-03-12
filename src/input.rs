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

// ── Keysym constants (raw u32) ────────────────────────────
mod keys {
    pub const SUPER_L:   u32 = 0xffeb;
    pub const SUPER_R:   u32 = 0xffec;
    pub const KEY_0:     u32 = 0x30;
    pub const KEY_1:     u32 = 0x31;
    pub const KEY_4:     u32 = 0x34;
    pub const KEY_9:     u32 = 0x39;
    pub const KEY_D:     u32 = 0x64;
    pub const KEY_L:     u32 = 0x6c;
    pub const KEY_Q:     u32 = 0x71;
    pub const KEY_T:     u32 = 0x74;
    pub const KEY_TAB:   u32 = 0xff09;
    pub const KEY_F4:    u32 = 0xffc1;
    pub const KEY_F5:    u32 = 0xffc2;
    pub const KEY_LEFT:  u32 = 0xff51;
    pub const KEY_RIGHT: u32 = 0xff53;
    pub const KEY_UP:    u32 = 0xff52;
    pub const KEY_DOWN:  u32 = 0xff54;
    pub const KEY_PRINT: u32 = 0xff61;
    pub const KEY_DELETE:u32 = 0xffff;
    pub const KEY_ESC:   u32 = 0xff1b;
    pub const KEY_RETURN:u32 = 0xff0d;
    pub const KEY_SPACE: u32 = 0x0020;

    // XF86 media keys
    pub const XF86_VOL_UP:     u32 = 0x1008ff13;
    pub const XF86_VOL_DOWN:   u32 = 0x1008ff11;
    pub const XF86_VOL_MUTE:   u32 = 0x1008ff12;
    pub const XF86_BRIGHT_UP:  u32 = 0x1008ff02;
    pub const XF86_BRIGHT_DOWN:u32 = 0x1008ff03;
    pub const XF86_AUDIO_PLAY: u32 = 0x1008ff14;
    pub const XF86_AUDIO_STOP: u32 = 0x1008ff15;
    pub const XF86_AUDIO_NEXT: u32 = 0x1008ff17;
    pub const XF86_AUDIO_PREV: u32 = 0x1008ff16;
}

const WIN_DOUBLE_MS: u128 = 350;

pub struct InputState {
    pub ptr_pos:          Point<f64, Logical>,
    pub mods:             ModifiersState,
    last_win_release:     Option<Instant>,
    win_held:             bool,
    super_just_pressed:   bool,   // Super wciśnięty w tej ramce
}

impl InputState {
    pub fn new() -> Self {
        Self {
            ptr_pos:            Point::from((0.0, 0.0)),
            mods:               ModifiersState::default(),
            last_win_release:   None,
            win_held:           false,
            super_just_pressed: false,
        }
    }
}

impl Default for InputState {
    fn default() -> Self { Self::new() }
}

impl InputState {

    pub fn handle<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        state: &mut crate::compositor::BlueState,
    ) {
        match event {
            InputEvent::Keyboard { event }              => self.on_key::<I>(event, state),
            InputEvent::PointerMotion { event }         => {
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
            InputEvent::PointerButton { event }         => self.on_button::<I>(event, state),
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
                let raw = keysym.modified_sym().raw();

                // ── Super / Win key ───────────────────────
                let is_super = raw == keys::SUPER_L || raw == keys::SUPER_R;
                if is_super {
                    match key_state {
                        KeyState::Pressed => {
                            cs.input_state.win_held = true;
                            cs.input_state.super_just_pressed = true;
                        }
                        KeyState::Released => {
                            let now = Instant::now();
                            // Double-tap Super → fullscreen launcher
                            let double = cs.input_state.last_win_release
                            .map(|t| now.duration_since(t).as_millis() < WIN_DOUBLE_MS)
                            .unwrap_or(false);

                            if double {
                                info!("Super+Super → FullscreenLauncher");
                                ipc::send(&IpcMessage::ToggleFullscreenLauncher);
                                cs.input_state.last_win_release = None;
                            } else {
                                // Single tap tylko jeśli nie było kombinacji z innym klawiszem
                                if cs.input_state.super_just_pressed {
                                    info!("Super → ToggleAppMenu");
                                    ipc::send(&IpcMessage::ToggleAppMenu);
                                }
                                cs.input_state.last_win_release = Some(now);
                            }
                            cs.input_state.win_held             = false;
                            cs.input_state.super_just_pressed   = false;
                        }
                    }
                    return FilterResult::Intercept(());
                }

                // Jeśli inny klawisz wciśnięty przy trzymaniu Super
                // → nie traktuj jako "single tap"
                if cs.input_state.win_held && key_state == KeyState::Pressed {
                    cs.input_state.super_just_pressed = false;
                }

                // Tylko Pressed events dla pozostałych skrótów
                if key_state != KeyState::Pressed {
                    return FilterResult::Forward;
                }

                let logo  = mods.logo;
                let ctrl  = mods.ctrl;
                let alt   = mods.alt;
                let shift = mods.shift;

                match (logo, ctrl, alt, shift, raw) {

                    // ── Super + klawisze ──────────────────

                    // Super+L → Lock screen
                    (true, false, false, false, k) if k == keys::KEY_L => {
                        info!("Super+L → LockScreen");
                        ipc::send(&IpcMessage::ShowLogoutDialog);
                        // Uruchom blue-lock jeśli dostępny
                        let _ = std::process::Command::new("blue-lock")
                        .spawn()
                        .or_else(|_| std::process::Command::new("loginctl")
                        .arg("lock-session").spawn());
                        FilterResult::Intercept(())
                    }

                    // Super+D → Show/Hide Desktop
                    (true, false, false, false, k) if k == keys::KEY_D => {
                        info!("Super+D → ShowDesktop");
                        cs.workspaces.toggle_show_desktop();
                        FilterResult::Intercept(())
                    }

                    // Super+Tab lub Alt+Tab → Window Switcher
                    (true, false, false, false, k) if k == keys::KEY_TAB => {
                        info!("Super+Tab → WindowSwitcher");
                        ipc::send(&IpcMessage::ShowWindowSwitcher);
                        FilterResult::Intercept(())
                    }
                    (false, false, true, false, k) if k == keys::KEY_TAB => {
                        info!("Alt+Tab → WindowSwitcher");
                        ipc::send(&IpcMessage::ShowWindowSwitcher);
                        FilterResult::Intercept(())
                    }

                    // Super+1..4 → Przełącz workspace (4 workspace'y)
                    (true, false, false, false, k)
                    if k >= keys::KEY_1 && k <= keys::KEY_4 =>
                    {
                        let idx = (k - keys::KEY_1) as usize;
                        info!("Super+{} → SwitchWorkspace({})", idx + 1, idx);
                        cs.workspaces.switch_to(idx);
                        ipc::send(&IpcMessage::WorkspaceChanged {
                            active: idx,
                            total:  cs.workspaces.count(),
                        });
                        FilterResult::Intercept(())
                    }

                    // Super+1..9 → obsługa extra workspace'ów
                    (true, false, false, false, k)
                    if k > keys::KEY_4 && k <= keys::KEY_9 =>
                    {
                        let idx = (k - keys::KEY_1) as usize;
                        cs.workspaces.switch_to(idx);
                        FilterResult::Intercept(())
                    }

                    // Super+Shift+1..4 → Przenieś okno na workspace
                    (true, false, false, true, k)
                    if k >= keys::KEY_1 && k <= keys::KEY_9 =>
                    {
                        let idx = (k - keys::KEY_1) as usize;
                        info!("Super+Shift+{} → MoveToWorkspace({})", idx + 1, idx);
                        cs.workspaces.move_focused_to(idx);
                        FilterResult::Intercept(())
                    }

                    // Super+strzałki → Tiling
                    (true, false, false, false, k) if k == keys::KEY_LEFT => {
                        cs.workspaces.tile_focused_left();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_RIGHT => {
                        cs.workspaces.tile_focused_right();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_UP => {
                        cs.workspaces.maximize_focused();
                        FilterResult::Intercept(())
                    }
                    (true, false, false, false, k) if k == keys::KEY_DOWN => {
                        cs.workspaces.restore_or_minimize_focused();
                        FilterResult::Intercept(())
                    }

                    // Super+Q / Super+F4 → Zamknij okno
                    (true, false, false, false, k)
                    if k == keys::KEY_Q || k == keys::KEY_F4 =>
                    {
                        cs.workspaces.close_focused();
                        FilterResult::Intercept(())
                    }

                    // ── Alt + klawisze ────────────────────

                    // Alt+F4 → Zamknij okno
                    (false, false, true, false, k) if k == keys::KEY_F4 => {
                        cs.workspaces.close_focused();
                        FilterResult::Intercept(())
                    }

                    // ── Ctrl+Alt ──────────────────────────

                    // Ctrl+Alt+T → Terminal
                    (false, true, true, false, k) if k == keys::KEY_T => {
                        info!("Ctrl+Alt+T → Terminal");
                        let _ = std::process::Command::new("konsole").spawn()
                        .or_else(|_| std::process::Command::new("alacritty").spawn())
                        .or_else(|_| std::process::Command::new("foot").spawn())
                        .or_else(|_| std::process::Command::new("xterm").spawn());
                        FilterResult::Intercept(())
                    }

                    // Ctrl+Alt+Delete → Logout dialog
                    (false, true, true, false, k) if k == keys::KEY_DELETE => {
                        ipc::send(&IpcMessage::ShowLogoutDialog);
                        FilterResult::Intercept(())
                    }

                    // ── Klawisze funkcyjne ────────────────

                    // Print → Screenshot
                    (false, false, false, false, k) if k == keys::KEY_PRINT => {
                        let _ = std::process::Command::new("blue-screenshot").spawn()
                        .or_else(|_| std::process::Command::new("spectacle").spawn())
                        .or_else(|_| std::process::Command::new("gnome-screenshot").spawn());
                        FilterResult::Intercept(())
                    }
                    // Super+Print → Region screenshot
                    (true, false, false, false, k) if k == keys::KEY_PRINT => {
                        let _ = std::process::Command::new("blue-screenshot")
                        .arg("--region").spawn()
                        .or_else(|_| std::process::Command::new("spectacle")
                        .arg("-r").spawn());
                        FilterResult::Intercept(())
                    }

                    // ── XF86 Media Keys ───────────────────

                    // Głośność +
                    (_, _, _, _, k) if k == keys::XF86_VOL_UP => {
                        let _ = std::process::Command::new("wpctl")
                        .args(["set-volume", "-l", "1.0",
                              "@DEFAULT_AUDIO_SINK@", "5%+"])
                        .spawn()
                        .or_else(|_| std::process::Command::new("pactl")
                        .args(["set-sink-volume", "@DEFAULT_SINK@", "+5%"])
                        .spawn());
                        FilterResult::Intercept(())
                    }
                    // Głośność -
                    (_, _, _, _, k) if k == keys::XF86_VOL_DOWN => {
                        let _ = std::process::Command::new("wpctl")
                        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"])
                        .spawn()
                        .or_else(|_| std::process::Command::new("pactl")
                        .args(["set-sink-volume", "@DEFAULT_SINK@", "-5%"])
                        .spawn());
                        FilterResult::Intercept(())
                    }
                    // Mute
                    (_, _, _, _, k) if k == keys::XF86_VOL_MUTE => {
                        let _ = std::process::Command::new("wpctl")
                        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
                        .spawn()
                        .or_else(|_| std::process::Command::new("pactl")
                        .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
                        .spawn());
                        FilterResult::Intercept(())
                    }
                    // Jasność +
                    (_, _, _, _, k) if k == keys::XF86_BRIGHT_UP => {
                        let _ = std::process::Command::new("brightnessctl")
                        .args(["set", "10%+"])
                        .spawn()
                        .or_else(|_| std::process::Command::new("xbacklight")
                        .args(["-inc", "10"])
                        .spawn());
                        FilterResult::Intercept(())
                    }
                    // Jasność -
                    (_, _, _, _, k) if k == keys::XF86_BRIGHT_DOWN => {
                        let _ = std::process::Command::new("brightnessctl")
                        .args(["set", "10%-"])
                        .spawn()
                        .or_else(|_| std::process::Command::new("xbacklight")
                        .args(["-dec", "10"])
                        .spawn());
                        FilterResult::Intercept(())
                    }
                    // Media: Play/Pause
                    (_, _, _, _, k) if k == keys::XF86_AUDIO_PLAY => {
                        let _ = std::process::Command::new("playerctl")
                        .arg("play-pause").spawn();
                        FilterResult::Intercept(())
                    }
                    // Media: Next
                    (_, _, _, _, k) if k == keys::XF86_AUDIO_NEXT => {
                        let _ = std::process::Command::new("playerctl")
                        .arg("next").spawn();
                        FilterResult::Intercept(())
                    }
                    // Media: Previous
                    (_, _, _, _, k) if k == keys::XF86_AUDIO_PREV => {
                        let _ = std::process::Command::new("playerctl")
                        .arg("previous").spawn();
                        FilterResult::Intercept(())
                    }

                    // ── F5 → Reload config ────────────────
                    (true, false, false, false, k) if k == keys::KEY_F5 => {
                        ipc::send(&IpcMessage::ConfigReload);
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
