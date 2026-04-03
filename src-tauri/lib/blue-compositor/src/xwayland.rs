use calloop::LoopHandle;
use smithay::{
    desktop::Window,
    utils::{Logical, Point, Rectangle},
    wayland::selection::SelectionTarget,
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId, X11Wm},
        X11Surface, XWayland,
    },
};
use std::os::unix::io::OwnedFd;
use tracing::info;

use crate::state::BlueState;

pub fn init_xwayland(state: &mut BlueState, loop_handle: &LoopHandle<'static, BlueState>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Initializing XWayland...");

    // W Smithay rev 82912edf konstruktor XWayland przyjmuje 3 argumenty
    let xwayland = XWayland::new(
        loop_handle,
        &state.display_handle,
        state.seat.clone(),
    )?;

    let (xwm, x11_display) = xwayland.start()?;

    state.xwayland = Some(xwayland);
    state.xwm = Some(xwm);
    state.x11_display = Some(x11_display);

    info!("XWayland started on display :{}", x11_display);

    Ok(())
}

// XwmHandler (bez zmian)
impl smithay::xwayland::xwm::XwmHandler for BlueState {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().unwrap()
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn new_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.space.map_element(
            Window::new_x11_window(window),
                               Point::from((0, 0)),
                               true,
        );
    }

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        window.set_mapped(true).ok();
        let offset = self.space.elements().count() * 30;
        let loc = Point::from(((offset + 150) as i32, (offset + 80) as i32));
        self.space.map_element(Window::new_x11_window(window), loc, true);
    }

    fn map_window_notify(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.space.map_element(
            Window::new_x11_window(window),
                               Point::from((100, 100)),
                               true,
        );
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let found = self.space
        .elements()
        .find(|w| w.x11_surface().map(|x| x == &window).unwrap_or(false))
        .cloned();
        if let Some(w) = found {
            self.space.unmap_elem(&w);
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let mut geo = window.geometry();
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = window.configure(geo);
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        _above: Option<u32>,
    ) {}

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _button: u32,
        _resize_edge: X11ResizeEdge,
    ) {}

    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {}

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        _selection: SelectionTarget,
        _mime_type: String,
        _fd: OwnedFd,
    ) {}

    fn allow_selection_access(&mut self, _xwm: XwmId, _selection: SelectionTarget) -> bool {
        true
    }

    fn new_selection(
        &mut self,
        _xwm: XwmId,
        _selection: SelectionTarget,
        _mime_types: Vec<String>,
    ) {}

    fn cleared_selection(&mut self, _xwm: XwmId, _selection: SelectionTarget) {}
}
