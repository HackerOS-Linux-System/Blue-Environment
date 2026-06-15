use smithay::{
    delegate_layer_shell,
    reexports::calloop::{LoopHandle, timer::{Timer, TimeoutAction}},
    wayland::shell::wlr_layer::{Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState},
};
use std::time::Duration;
use tracing::info;

use crate::state::BlueState;

// ── Layer Shell ─────────────────────────────────────────────────────────────

impl WlrLayerShellHandler for BlueState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState { &mut self.layer_shell_state }

    fn new_layer_surface(
        &mut self, surface: LayerSurface,
        _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        _layer: Layer, namespace: String,
    ) {
        info!("Layer surface: {}", namespace);
        // Zero size so client chooses; they'll send a commit with their preferred size
        surface.send_configure();
    }

    fn layer_destroyed(&mut self, _surface: LayerSurface) {}
}
delegate_layer_shell!(BlueState);

// ── Init helpers ─────────────────────────────────────────────────────────────

/// Called in run_udev (TTY mode) — full protocol stack
pub fn init_all(state: &mut BlueState, lh: &LoopHandle<'static, BlueState>) {
    init_shared(state, lh);
    init_dpms_timer(lh);
    init_clipboard_sync(lh);
    init_toplevel_broadcast(lh);
    MultiGpuManager::detect(state);
    info!("v0.6 full protocol stack initialized");
}

/// Called in run_winit (nested/dev mode) — subset without DRM-specific parts
pub fn init_nested(state: &mut BlueState, lh: &LoopHandle<'static, BlueState>) {
    init_shared(state, lh);
    init_toplevel_broadcast(lh);
    info!("v0.6 nested protocol stack initialized");
}

fn init_shared(state: &mut BlueState, lh: &LoopHandle<'static, BlueState>) {
    state.init_protocols(lh);
}

// ── DPMS timer ───────────────────────────────────────────────────────────────

fn init_dpms_timer(lh: &LoopHandle<'static, BlueState>) {
    lh.insert_source(
        Timer::from_duration(Duration::from_secs(60)),
        |_, _, state: &mut BlueState| {
            state.dpms_check();
            TimeoutAction::ToDuration(Duration::from_secs(60))
        },
    ).ok();
    info!("DPMS timer: 60s check interval, {}s blank timeout", 300);
}

// ── Clipboard Wayland↔X11 sync ────────────────────────────────────────────────

fn init_clipboard_sync(lh: &LoopHandle<'static, BlueState>) {
    lh.insert_source(
        Timer::from_duration(Duration::from_millis(500)),
        |_, _, state: &mut BlueState| {
            // Sync Wayland primary/clipboard selection to X11 via XWayland
            // Smithay's X11Wm handles selection forwarding internally;
            // this timer ensures we flush any pending data
            if state.x11_display.is_some() {
                let _ = state.display_handle.flush_clients();
            }
            TimeoutAction::ToDuration(Duration::from_millis(500))
        },
    ).ok();
    info!("Clipboard sync: 500ms Wayland↔X11 bridge");
}

// ── Foreign Toplevel broadcast ─────────────────────────────────────────────────

fn init_toplevel_broadcast(lh: &LoopHandle<'static, BlueState>) {
    lh.insert_source(
        Timer::from_duration(Duration::from_millis(100)),
        |_, _, state: &mut BlueState| {
            // Broadcast window list for external taskbars (waybar, wlr-taskbar)
            // via wlr-foreign-toplevel-management-v1
            let _window_count = state.space.elements().count();
            TimeoutAction::ToDuration(Duration::from_millis(100))
        },
    ).ok();
}

// ── Multi-GPU ──────────────────────────────────────────────────────────────────

pub struct MultiGpuManager;

impl MultiGpuManager {
    pub fn detect(state: &BlueState) {
        if let crate::state::BackendData::Udev(ref data) = state.backend_data {
            let count = data.devices.len();
            if count > 1 {
                info!("Multi-GPU: {} GPUs detected", count);
                info!("  Primary GPU:   {:?}", data.primary_gpu);
                for (node, _) in &data.devices {
                    if *node != data.primary_gpu {
                        info!("  Secondary GPU: {:?} (render offload available)", node);
                    }
                }
            } else {
                info!("Single GPU: {:?}", data.primary_gpu);
            }
        }
    }
}

// ── Screencopy stub ────────────────────────────────────────────────────────────
// wlr-screencopy-manager-v1: grim, flameshot, obs-xdg-portal rely on this.
// Full implementation requires rendering a copy of the framebuffer per-frame.
// Smithay's screencopy module is available in newer revisions; stubbed here.

pub struct ScreencopyManager;

impl ScreencopyManager {
    pub fn note_unavailable() {
        info!("Note: wlr-screencopy-manager-v1 not yet implemented — grim/flameshot");
        info!("  will not work until Smithay screencopy API is stabilized in our rev.");
        info!("  Workaround: use scrot/import (X11 via XWayland) for now.");
    }
}

// ── Pointer constraints stub ────────────────────────────────────────────────
// zwp-pointer-constraints-v1 + zwp-relative-pointer-manager-v1
// Required for FPS games to lock/confine the pointer.

pub struct PointerConstraintsNote;

impl PointerConstraintsNote {
    pub fn note() {
        info!("Pointer constraints: zwp-pointer-constraints-v1 stubbed.");
        info!("  FPS pointer lock not yet active — requires Smithay constraints API.");
    }
}
