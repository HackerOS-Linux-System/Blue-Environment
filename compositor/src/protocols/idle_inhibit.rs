use smithay::{
    delegate_idle_inhibit,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::IsAlive,
    wayland::idle_inhibit::IdleInhibitHandler,
};
use tracing::info;
use crate::state::BlueState;

impl IdleInhibitHandler for BlueState {
    fn inhibit(&mut self, surface: WlSurface) {
        info!("idle-inhibit: surface requested an inhibitor");
        self.idle_inhibiting_surfaces.push(surface);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        info!("idle-inhibit: inhibitor released");
        self.idle_inhibiting_surfaces.retain(|s| s != &surface);
    }
}

delegate_idle_inhibit!(BlueState);

impl BlueState {
    /// True while at least one visible, non-destroyed surface holds an
    /// idle inhibitor. `protocols/idle.rs`'s DPMS timer should check this
    /// before blanking outputs.
    pub fn is_idle_inhibited(&self) -> bool {
        self.idle_inhibiting_surfaces.iter().any(|s| s.alive())
    }
}

/// No-op: the global is created by `IdleInhibitManagerState::new` inside
/// `BlueState::new()` already; kept as a stable call site in case future
/// setup (logging, metrics) is needed here.
pub fn init(_state: &mut BlueState) {}
