#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        IRect { x, y, w, h }
    }

    pub fn intersect(&self, o: &IRect) -> Option<IRect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        (x1 > x0 && y1 > y0).then(|| IRect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

/// What the GTK holder must be set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// Holder (the visible window onto the page), in shell coordinates.
    pub holder: IRect,
    /// Scroll of the page inside the holder: how much of the page's left / top
    /// edge lies outside it.
    pub offset_x: i32,
    pub offset_y: i32,
}

/// `page` is where the whole page lives; `clip` (if any) the part of it that
/// may be seen. A clip that does not overlap the page leaves the page whole —
/// the caller hides the view in that case.
pub fn resolve(page: IRect, clip: Option<IRect>) -> Placement {
    let visible = clip.and_then(|c| page.intersect(&c)).unwrap_or(page);
    Placement { holder: visible, offset_x: visible.x - page.x, offset_y: visible.y - page.y }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: IRect = IRect { x: 100, y: 50, w: 800, h: 600 };

    #[test]
    fn no_clip_shows_the_whole_page() {
        let p = resolve(PAGE, None);
        assert_eq!(p.holder, PAGE);
        assert_eq!((p.offset_x, p.offset_y), (0, 0));
    }

    #[test]
    fn clip_to_the_left_half_keeps_the_origin() {
        let p = resolve(PAGE, Some(IRect::new(100, 50, 400, 600)));
        assert_eq!(p.holder, IRect::new(100, 50, 400, 600));
        assert_eq!((p.offset_x, p.offset_y), (0, 0));
    }

    #[test]
    fn clip_to_the_right_half_scrolls_the_page_inside_the_holder() {
        let p = resolve(PAGE, Some(IRect::new(500, 50, 400, 600)));
        assert_eq!(p.holder, IRect::new(500, 50, 400, 600));
        assert_eq!((p.offset_x, p.offset_y), (400, 0));
    }

    #[test]
    fn clip_is_limited_to_the_page() {
        let p = resolve(PAGE, Some(IRect::new(0, 0, 5000, 5000)));
        assert_eq!(p.holder, PAGE);
        let p = resolve(PAGE, Some(IRect::new(850, 600, 500, 500)));
        assert_eq!(p.holder, IRect::new(850, 600, 50, 50));
        assert_eq!((p.offset_x, p.offset_y), (750, 550));
    }

    #[test]
    fn a_clip_outside_the_page_falls_back_to_the_whole_page() {
        assert_eq!(resolve(PAGE, Some(IRect::new(2000, 2000, 10, 10))).holder, PAGE);
    }
}
