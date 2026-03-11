use smithay::utils::{Logical, Point, Rectangle, Size};
use std::collections::HashMap;
use tracing::debug;

pub const TITLEBAR_H:   i32 = 32;
pub const BORDER_W:     i32 = 4;
pub const BTN_SIZE:     i32 = 18;
pub const BTN_MARGIN:   i32 = 6;
pub const BTN_Y_OFFSET: i32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoButton { Minimize, Maximize, Close }

#[derive(Debug, Clone)]
pub struct WindowDecoration {
    pub title:     String,
    pub maximized: bool,
    pub active:    bool,
    pub mode:      DecoMode,
    pub dragging:           bool,
    pub drag_start_ptr:     Point<f64, Logical>,
    pub drag_start_win:     Point<i32, Logical>,
    pub resizing:           bool,
    pub resize_edge:        ResizeEdge,
    pub resize_start_ptr:   Point<f64, Logical>,
    pub resize_start_geo:   Rectangle<i32, Logical>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoMode { ServerSide, ClientSide }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    None, Top, Bottom, Left, Right,
    TopLeft, TopRight, BottomLeft, BottomRight,
}

impl Default for WindowDecoration {
    fn default() -> Self {
        Self {
            title:     String::new(),
            maximized: false,
            active:    false,
            mode:      DecoMode::ServerSide,
            dragging:          false,
            drag_start_ptr:    Point::from((0.0, 0.0)),
            drag_start_win:    Point::from((0, 0)),
            resizing:          false,
            resize_edge:       ResizeEdge::None,
            resize_start_ptr:  Point::from((0.0, 0.0)),
            resize_start_geo:  Rectangle::new(Point::from((0, 0)), Size::from((800, 600))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTest {
    Client,
    Titlebar,
    Button(DecoButton),
    Edge(ResizeEdge),
}

impl WindowDecoration {
    pub fn hit_test(&self, pos: Point<f64, Logical>, win_size: Size<i32, Logical>) -> HitTest {
        if self.mode == DecoMode::ClientSide { return HitTest::Client; }
        if self.maximized { return self.hit_titlebar_only(pos, win_size); }

        let x = pos.x as i32;
        let y = pos.y as i32;
        let w = win_size.w + BORDER_W * 2;
        let h = win_size.h + TITLEBAR_H + BORDER_W;

        let on_left   = x < BORDER_W;
        let on_right  = x >= w - BORDER_W;
        let on_top    = y < BORDER_W;
        let on_bottom = y >= h - BORDER_W;

        if on_top    && on_left  { return HitTest::Edge(ResizeEdge::TopLeft);     }
        if on_top    && on_right { return HitTest::Edge(ResizeEdge::TopRight);    }
        if on_bottom && on_left  { return HitTest::Edge(ResizeEdge::BottomLeft);  }
        if on_bottom && on_right { return HitTest::Edge(ResizeEdge::BottomRight); }
        if on_top    { return HitTest::Edge(ResizeEdge::Top);    }
        if on_bottom { return HitTest::Edge(ResizeEdge::Bottom); }
        if on_left   { return HitTest::Edge(ResizeEdge::Left);   }
        if on_right  { return HitTest::Edge(ResizeEdge::Right);  }

        self.hit_titlebar_only(pos, win_size)
    }

    fn hit_titlebar_only(&self, pos: Point<f64, Logical>, win_size: Size<i32, Logical>) -> HitTest {
        let x = pos.x as i32;
        let y = pos.y as i32;
        let w = win_size.w + BORDER_W * 2;

        if !(y >= BORDER_W && y < BORDER_W + TITLEBAR_H) { return HitTest::Client; }

        let btn_close_x = w - BTN_MARGIN - BTN_SIZE;
        let btn_max_x   = btn_close_x - BTN_MARGIN - BTN_SIZE;
        let btn_min_x   = btn_max_x   - BTN_MARGIN - BTN_SIZE;
        let btn_y_start = BORDER_W + BTN_Y_OFFSET;
        let btn_y_end   = btn_y_start + BTN_SIZE;

        if y >= btn_y_start && y < btn_y_end {
            if x >= btn_close_x && x < btn_close_x + BTN_SIZE { return HitTest::Button(DecoButton::Close);    }
            if x >= btn_max_x   && x < btn_max_x   + BTN_SIZE { return HitTest::Button(DecoButton::Maximize); }
            if x >= btn_min_x   && x < btn_min_x   + BTN_SIZE { return HitTest::Button(DecoButton::Minimize); }
        }

        HitTest::Titlebar
    }

    pub fn outer_geometry(client_geo: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        Rectangle::new(
            Point::from((client_geo.loc.x - BORDER_W, client_geo.loc.y - TITLEBAR_H - BORDER_W)),
                       Size::from((client_geo.size.w + BORDER_W * 2, client_geo.size.h + TITLEBAR_H + BORDER_W * 2)),
        )
    }

    pub fn client_offset() -> Point<i32, Logical> {
        Point::from((BORDER_W, TITLEBAR_H + BORDER_W))
    }
}

// ── Manager ───────────────────────────────────────────────

pub struct CsdManager {
    decorations: HashMap<u32, WindowDecoration>,
}

impl CsdManager {
    pub fn new() -> Self { Self { decorations: HashMap::new() } }

    pub fn register(&mut self, surface_id: u32, title: &str) {
        debug!("CSD register surface {}: {}", surface_id, title);
        self.decorations.insert(surface_id, WindowDecoration { title: title.to_string(), ..Default::default() });
    }

    pub fn unregister(&mut self, surface_id: u32) { self.decorations.remove(&surface_id); }
    pub fn get(&self,     id: u32) -> Option<&WindowDecoration>     { self.decorations.get(&id) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut WindowDecoration> { self.decorations.get_mut(&id) }

    pub fn set_title(&mut self, id: u32, title: &str) {
        if let Some(d) = self.decorations.get_mut(&id) { d.title = title.to_string(); }
    }
    pub fn set_active(&mut self, id: u32, active: bool) {
        if let Some(d) = self.decorations.get_mut(&id) { d.active = active; }
    }
    pub fn set_maximized(&mut self, id: u32, maximized: bool) {
        if let Some(d) = self.decorations.get_mut(&id) { d.maximized = maximized; }
    }

    pub fn start_move(&mut self, id: u32, ptr: Point<f64, Logical>, win_loc: Point<i32, Logical>) {
        if let Some(d) = self.decorations.get_mut(&id) {
            d.dragging        = true;
            d.drag_start_ptr  = ptr;
            d.drag_start_win  = win_loc;
        }
    }

    pub fn update_move(&self, id: u32, ptr: Point<f64, Logical>) -> Option<Point<i32, Logical>> {
        let d = self.decorations.get(&id)?;
        if !d.dragging { return None; }
        Some(Point::from((
            d.drag_start_win.x + (ptr.x - d.drag_start_ptr.x) as i32,
                          d.drag_start_win.y + (ptr.y - d.drag_start_ptr.y) as i32,
        )))
    }

    pub fn end_move(&mut self, id: u32) {
        if let Some(d) = self.decorations.get_mut(&id) { d.dragging = false; }
    }

    pub fn start_resize(&mut self, id: u32, edge: ResizeEdge, ptr: Point<f64, Logical>, geo: Rectangle<i32, Logical>) {
        if let Some(d) = self.decorations.get_mut(&id) {
            d.resizing         = true;
            d.resize_edge      = edge;
            d.resize_start_ptr = ptr;
            d.resize_start_geo = geo;
        }
    }

    pub fn update_resize(&self, id: u32, ptr: Point<f64, Logical>) -> Option<Rectangle<i32, Logical>> {
        let d = self.decorations.get(&id)?;
        if !d.resizing { return None; }

        let dx   = (ptr.x - d.resize_start_ptr.x) as i32;
        let dy   = (ptr.y - d.resize_start_ptr.y) as i32;
        let base = d.resize_start_geo;
        let min_w = 200i32;
        let min_h = 150i32;

        let (mut x, mut y) = (base.loc.x, base.loc.y);
        let (mut w, mut h) = (base.size.w, base.size.h);

        match d.resize_edge {
            ResizeEdge::Right       => { w = (w + dx).max(min_w); }
            ResizeEdge::Bottom      => { h = (h + dy).max(min_h); }
            ResizeEdge::Left        => { let nw = (w - dx).max(min_w); x += w - nw; w = nw; }
            ResizeEdge::Top         => { let nh = (h - dy).max(min_h); y += h - nh; h = nh; }
            ResizeEdge::BottomRight => { w = (w + dx).max(min_w); h = (h + dy).max(min_h); }
            ResizeEdge::BottomLeft  => { let nw = (w - dx).max(min_w); x += w - nw; w = nw; h = (h + dy).max(min_h); }
            ResizeEdge::TopRight    => { w = (w + dx).max(min_w); let nh = (h - dy).max(min_h); y += h - nh; h = nh; }
            ResizeEdge::TopLeft     => {
                let nw = (w - dx).max(min_w); x += w - nw; w = nw;
                let nh = (h - dy).max(min_h); y += h - nh; h = nh;
            }
            ResizeEdge::None => {}
        }

        Some(Rectangle::new(Point::from((x, y)), Size::from((w, h))))
    }

    pub fn end_resize(&mut self, id: u32) {
        if let Some(d) = self.decorations.get_mut(&id) {
            d.resizing    = false;
            d.resize_edge = ResizeEdge::None;
        }
    }

    pub fn any_dragging(&self) -> bool {
        self.decorations.values().any(|d| d.dragging || d.resizing)
    }
}
