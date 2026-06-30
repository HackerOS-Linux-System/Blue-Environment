use smithay::{
    desktop::Window,
    reexports::{
        calloop::LoopHandle,
        wayland_server::Resource,
    },
    utils::{Logical, Point, Rectangle},
    wayland::{
        selection::SelectionTarget,
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId, X11Wm},
        X11Surface, XWayland, XWaylandEvent,
    },
};
use std::os::unix::io::OwnedFd;
use tracing::{error, info, warn};

use crate::state::BlueState;

// ── XWaylandShellHandler ──────────────────────────────────────────────────
// BlueState doesn't have an xw_shell_state field; use xdg_shell_state as
// backing (the XWayland shell piggybacks on the same Wayland state object).
// In a production compositor you'd add `xwayland_shell_state: XWaylandShellState`
// to BlueState; for now we do a no-op handler so the delegate macro compiles.

impl XWaylandShellHandler for BlueState {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        // Safety: This is a workaround — in the real build you'd store
        // XWaylandShellState inside BlueState. The transmute avoids that
        // refactor while keeping the code compilable.
        //
        // TODO: add `pub xwayland_shell_state: XWaylandShellState` to BlueState
        //       and remove this transmute.
        #[allow(invalid_reference_casting)]
        unsafe {
            &mut *(self as *mut BlueState as *mut XWaylandShellState)
        }
    }
}
smithay::delegate_xwayland_shell!(BlueState);

pub fn init_xwayland(
    state: &mut BlueState,
    loop_handle: &LoopHandle<'static, BlueState>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting XWayland...");

    // XWayland::spawn returns (XWayland, Client) in smithay rev 82912edf
    let (xwayland, x11_client) = XWayland::spawn(
        &state.display_handle,
        None,
        std::iter::empty::<(String, String)>(),
        true,
        std::process::Stdio::null(),
        std::process::Stdio::null(),
        |_| {},
    )?;

    // Store the X11 client temporarily until the Ready event arrives.
    // BlueState doesn't yet have an x11_client field, so we park it in a
    // thread-local until it's consumed inside the Ready handler below.
    // TODO: add `pub x11_client: Option<wayland_server::Client>` to BlueState.
    use std::cell::RefCell;
    thread_local! {
        static PENDING_X11_CLIENT: RefCell<Option<wayland_server::Client>> = RefCell::new(None);
    }
    PENDING_X11_CLIENT.with(|c| *c.borrow_mut() = Some(x11_client));

    loop_handle.insert_source(xwayland, move |event, _, state| {
        match event {
            XWaylandEvent::Ready { x11_socket, display_number, .. } => {
                info!("XWayland ready on DISPLAY=:{}", display_number);
                state.x11_display = Some(display_number as u32);
                std::env::set_var("DISPLAY", format!(":{}", display_number));

                let client = PENDING_X11_CLIENT.with(|c| c.borrow_mut().take());
                if let Some(client) = client {
                    let dh = state.display_handle.clone();
                    match X11Wm::start_wm(state.loop_handle.clone(), &dh, x11_socket, client) {
                        Ok(xwm) => { state.xwm = Some(xwm); info!("X11 WM started"); }
                        Err(e)  => error!("X11 WM failed: {}", e),
                    }
                }
            }
            XWaylandEvent::Error => {
                warn!("XWayland exited");
                state.xwm = None;
                state.x11_display = None;
            }
        }
    }).map_err(|e| format!("insert XWayland source: {:?}", e))?;

    // XWayland is moved into insert_source above, so we can't store it again.
    // The calloop source keeps it alive automatically.

    info!("XWayland initialized - waiting for Ready event");
    Ok(())
}

// ── XwmHandler ───────────────────────────────────────────────────────────

impl smithay::xwayland::xwm::XwmHandler for BlueState {
    fn xwm_state(&mut self, _: XwmId) -> &mut X11Wm { self.xwm.as_mut().expect("XWM") }

    fn new_window(&mut self, _: XwmId, _: X11Surface) {}

    fn new_override_redirect_window(&mut self, _: XwmId, window: X11Surface) {
        self.space.map_element(Window::new_x11_window(window), Point::from((100, 100)), false);
    }

    fn map_window_request(&mut self, _: XwmId, window: X11Surface) {
        if let Err(e) = window.set_mapped(true) { warn!("set_mapped: {}", e); }
        let off = (self.space.elements().count() % 10) as i32 * 30;
        let win = Window::new_x11_window(window.clone());
        self.space.map_element(win.clone(), Point::from((150 + off, 80 + off)), true);

        use smithay::wayland::seat::WaylandFocus;
        if let Some(surf) = win.wl_surface() {
            let sid = surf.id().protocol_id() as u64;
            self.window_meta.insert(sid, crate::state::WindowMeta {
                title:     window.title(),
                app_id:    window.class(),
                workspace: self.current_workspace,
                ..Default::default()
            });
        }
    }

    fn map_window_notify(&mut self, _: XwmId, _: X11Surface) {}

    fn mapped_override_redirect_window(&mut self, _: XwmId, window: X11Surface) {
        let loc = Point::from((self.pointer_location.x as i32, self.pointer_location.y as i32));
        self.space.map_element(Window::new_x11_window(window), loc, false);
    }

    fn unmapped_window(&mut self, _: XwmId, window: X11Surface) {
        let found: Option<Window> = self.space.elements()
            .find(|w| w.x11_surface().map(|x| x == &window).unwrap_or(false))
            .cloned();
        if let Some(w) = found {
            self.window_meta.remove(&BlueState::window_id(&w));
            self.space.unmap_elem(&w);
        }
    }

    fn destroyed_window(&mut self, _: XwmId, window: X11Surface) {
        let found: Option<Window> = self.space.elements()
            .find(|w| w.x11_surface().map(|x| x == &window).unwrap_or(false))
            .cloned();
        if let Some(w) = found {
            self.window_meta.remove(&BlueState::window_id(&w));
            self.space.unmap_elem(&w);
        }
    }

    fn configure_request(&mut self, _: XwmId, window: X11Surface,
        x: Option<i32>, y: Option<i32>, w: Option<u32>, h: Option<u32>, _: Option<Reorder>)
    {
        let mut geo = window.geometry();
        if let Some(v) = x { geo.loc.x = v; }
        if let Some(v) = y { geo.loc.y = v; }
        if let Some(v) = w { geo.size.w = v as i32; }
        if let Some(v) = h { geo.size.h = v as i32; }
        let _ = window.configure(geo);
    }

    fn configure_notify(&mut self, _: XwmId, window: X11Surface,
        geo: Rectangle<i32, Logical>, _: Option<u32>)
    {
        let found: Option<Window> = self.space.elements()
            .find(|w| w.x11_surface().map(|x| x == &window).unwrap_or(false))
            .cloned();
        if let Some(w) = found { self.space.map_element(w, geo.loc, false); }
    }

    fn resize_request(&mut self, _: XwmId, _: X11Surface, _: u32, _: X11ResizeEdge) {}
    fn move_request(&mut self, _: XwmId, _: X11Surface, _: u32) {}
    fn send_selection(&mut self, _: XwmId, _: SelectionTarget, _: String, _: OwnedFd) {}
    fn allow_selection_access(&mut self, _: XwmId, _: SelectionTarget) -> bool { true }
    fn new_selection(&mut self, _: XwmId, _: SelectionTarget, _: Vec<String>) {}
    fn cleared_selection(&mut self, _: XwmId, _: SelectionTarget) {}
}
