use smithay::{
    desktop::{Space, Window},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::shell::xdg::{PopupSurface, PositionerState},
};
use tracing::{debug, info};

pub struct Workspace {
    pub id:    usize,
    pub name:  String,
    pub space: Space<Window>,
}

impl Workspace {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            name:  format!("Workspace {}", id + 1),
            space: Space::default(),
        }
    }
}

pub struct WorkspaceManager {
    pub workspaces: Vec<Workspace>,
    pub active:     usize,
    pub show_desktop: bool,
}

impl WorkspaceManager {
    pub fn new(count: usize) -> Self {
        Self {
            workspaces:   (0..count.max(1)).map(Workspace::new).collect(),
            active:       0,
            show_desktop: false,
        }
    }

    pub fn active_space(&self) -> &Space<Window> {
        &self.workspaces[self.active].space
    }

    pub fn active_space_mut(&mut self) -> &mut Space<Window> {
        &mut self.workspaces[self.active].space
    }

    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.workspaces.len() {
            info!("Workspace → {}", idx + 1);
            self.active = idx;
        }
    }

    pub fn add_window(&mut self, window: Window) {
        let n = self.workspaces[self.active].space.elements().count() as i32;
        let pos = Point::from((50 + n * 28, 50 + n * 28));
        self.workspaces[self.active].space.map_element(window, pos, true);
    }

    pub fn focus_window(&mut self, window: &Window) {
        self.workspaces[self.active].space.raise_element(window, true);
    }

    pub fn window_at(&self, pos: Point<f64, Logical>) -> Option<Window> {
        self.workspaces[self.active]
        .space
        .element_under(pos)
        .map(|(w, _)| w.clone())
    }

    pub fn close_focused(&mut self) {
        if let Some(w) = self.workspaces[self.active]
            .space.elements().last().cloned()
            {
                if let Some(t) = w.toplevel() {
                    t.send_close();
                }
            }
    }

    pub fn maximize_focused(&mut self) {
        debug!("maximize_focused (stub)");
    }

    pub fn restore_or_minimize_focused(&mut self) {
        debug!("restore_or_minimize_focused (stub)");
    }

    pub fn tile_focused_left(&mut self) {
        debug!("tile_focused_left (stub)");
    }

    pub fn tile_focused_right(&mut self) {
        debug!("tile_focused_right (stub)");
    }

    pub fn move_focused_to(&mut self, to: usize) {
        if to >= self.workspaces.len() { return; }
        if let Some(w) = self.workspaces[self.active]
            .space.elements().last().cloned()
            {
                let loc = self.workspaces[self.active]
                .space.element_location(&w)
                .unwrap_or_default();
                self.workspaces[self.active].space.unmap_elem(&w);
                self.workspaces[to].space.map_element(w, loc, false);
            }
    }

    pub fn toggle_show_desktop(&mut self) {
        self.show_desktop = !self.show_desktop;
        info!("Show desktop: {}", self.show_desktop);
    }

    // Space nie ma .commit() w smithay 0.6 — zamiast tego refresh()
    pub fn handle_commit(&mut self, _surface: &WlSurface) {
        for ws in &mut self.workspaces {
            ws.space.refresh();
        }
    }

    pub fn handle_new_popup(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
    ) {}

    pub fn count(&self) -> usize {
        self.workspaces.len()
    }
}
