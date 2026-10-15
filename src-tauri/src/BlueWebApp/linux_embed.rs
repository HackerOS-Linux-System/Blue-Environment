use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebView, WebViewExt};

use super::clip::{resolve, IRect};
use super::snapshot::RawSnapshot;

/// Where a page lives and which part of it may be seen (see `clip.rs`).
#[derive(Default, Clone, Copy)]
struct Placed {
    page: Option<IRect>,
    clip: Option<IRect>,
}

thread_local! {
    /// Overlay okna powłoki. Dostępny tylko z głównego wątku GTK.
    static OVERLAY: RefCell<Option<gtk::Overlay>> = const { RefCell::new(None) };
    /// Per-page placement, keyed by the webview's widget pointer (main GTK thread only).
    static PLACED: RefCell<HashMap<usize, Placed>> = RefCell::new(HashMap::new());
}

fn key(w: &WebView) -> usize {
    w.upcast_ref::<gtk::Widget>().as_ptr() as usize
}

/// `BLUE_WEB_NO_CLIP=1` restores the plain (unwrapped) embedding: pages are then
/// only ever shown or hidden as a whole.
fn clipping_enabled() -> bool {
    std::env::var_os("BLUE_WEB_NO_CLIP").is_none()
}

/// The widget that sits in the overlay for this page: the clipping holder
/// (`GtkViewport`) when there is one, otherwise the webview itself.
fn top_of(child: &WebView) -> gtk::Widget {
    match child.parent() {
        Some(p) if p.is::<gtk::Viewport>() => p,
        _ => child.clone().upcast(),
    }
}

fn holder_of(child: &WebView) -> Option<gtk::Viewport> {
    child.parent().and_then(|p| p.downcast::<gtk::Viewport>().ok())
}

fn new_holder() -> gtk::Viewport {
    let vp = gtk::Viewport::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    vp.set_shadow_type(gtk::ShadowType::None);
    vp.set_halign(gtk::Align::Start);
    vp.set_valign(gtk::Align::Start);
    vp.set_hexpand(false);
    vp.set_vexpand(false);
    vp.set_can_focus(false);
    vp
}

/// Zwraca overlay powłoki, tworząc go przy pierwszym użyciu.
///
/// `any_webview_in_vbox` to dowolny webview, który aktualnie jest
/// bezpośrednim dzieckiem `GtkBox` okna (główny webview powłoki albo świeżo
/// utworzone dziecko, które wry upchnął w `GtkBox`).
fn ensure_overlay(any_webview_in_vbox: &WebView) -> Option<gtk::Overlay> {
    if let Some(existing) = OVERLAY.with(|c| c.borrow().clone()) {
        return Some(existing);
    }

    let parent = any_webview_in_vbox.parent()?;

    // Już jesteśmy w overlayu (np. po ponownym wywołaniu) — po prostu użyj go.
    if let Ok(overlay) = parent.clone().downcast::<gtk::Overlay>() {
        OVERLAY.with(|c| *c.borrow_mut() = Some(overlay.clone()));
        return Some(overlay);
    }

    let vbox = parent.downcast::<gtk::Box>().ok()?;

    // Znajdź główny webview powłoki: pierwszy WebView w vboxie.
    let main_widget = vbox
        .children()
        .into_iter()
        .find(|w| w.is::<WebView>())?;

    let position = vbox
        .children()
        .iter()
        .position(|w| w == &main_widget)
        .map(|p| p as i32)
        .unwrap_or(0);
    let (expand, fill, padding, _pack) = vbox.query_child_packing(&main_widget);

    let overlay = gtk::Overlay::new();
    overlay.set_hexpand(true);
    overlay.set_vexpand(true);

    // Trzymamy własną referencję, żeby `remove` nie zniszczył widgetu.
    let keep = main_widget.clone();
    vbox.remove(&keep);
    overlay.add(&keep);
    vbox.pack_start(&overlay, expand, fill, padding);
    vbox.reorder_child(&overlay, position);
    overlay.show_all();

    OVERLAY.with(|c| *c.borrow_mut() = Some(overlay.clone()));
    Some(overlay)
}

/// Wołane raz na starcie (po utworzeniu głównego okna), żeby przebudowa
/// hierarchii GTK nie następowała dopiero przy otwarciu pierwszej karty.
/// Bezpieczne do wielokrotnego wywołania.
pub fn init_for_main_webview(main_webview: &WebView) {
    let _ = ensure_overlay(main_webview);
}

fn clamp_i32(v: f64, min: i32, max: i32) -> i32 {
    if !v.is_finite() {
        return min;
    }
    (v.round() as i64).clamp(min as i64, max as i64) as i32
}

/// Pushes the stored page rectangle + clip into GTK.
///
/// With a holder: the holder is the visible window (position via its margins,
/// size via `size_request`), the page keeps its FULL size inside it and is
/// scrolled so the right part shows — the page never reflows, it is only cut.
/// Without one (clipping disabled) the webview is placed directly, as before.
fn apply_placement(child: &WebView) {
    let placed = PLACED.with(|m| m.borrow().get(&key(child)).copied().unwrap_or_default());
    let Some(page) = placed.page else { return };
    let top = top_of(child);

    if let Some(holder) = holder_of(child) {
        let pl = resolve(page, placed.clip);
        set_margins(&top, pl.holder.x, pl.holder.y);
        set_request(&top, pl.holder.w, pl.holder.h);
        // The page itself: full size, no margins of its own.
        set_margins(child.upcast_ref(), 0, 0);
        set_request(child.upcast_ref(), page.w, page.h);
        if let Some(h) = holder.hadjustment() {
            h.configure(pl.offset_x as f64, 0.0, page.w as f64, 1.0, 10.0, pl.holder.w as f64);
        }
        if let Some(v) = holder.vadjustment() {
            v.configure(pl.offset_y as f64, 0.0, page.h as f64, 1.0, 10.0, pl.holder.h as f64);
        }
    } else {
        set_margins(&top, page.x, page.y);
        set_request(&top, page.w, page.h);
    }
}

fn set_margins(w: &gtk::Widget, x: i32, y: i32) {
    if w.margin_start() != x {
        w.set_margin_start(x);
    }
    if w.margin_top() != y {
        w.set_margin_top(y);
    }
}

fn set_request(w: &gtk::Widget, width: i32, height: i32) {
    let (cw, ch) = w.size_request();
    if cw != width || ch != height {
        w.set_size_request(width, height);
    }
}

fn store_page(child: &WebView, x: f64, y: f64, width: f64, height: f64) {
    let rect = IRect::new(clamp_i32(x, 0, 32767), clamp_i32(y, 0, 32767), clamp_i32(width, 1, 32767), clamp_i32(height, 1, 32767));
    PLACED.with(|m| m.borrow_mut().entry(key(child)).or_default().page = Some(rect));
}

/// Przenosi świeżo utworzony webview-dziecko z `GtkBox` do overlaya i ustawia
/// jego pozycję/rozmiar. Główny wątek GTK.
pub fn attach_child(child: &WebView, x: f64, y: f64, width: f64, height: f64) {
    // Po `add_child` wry zrobił już `pack_start` do vboxa, więc `child` jest
    // dzieckiem `GtkBox` — z niego (lub z siostrzanego głównego webview)
    // wyprowadzamy overlay, jeśli jeszcze go nie ma.
    let overlay = match ensure_overlay(child) {
        Some(o) => o,
        None => {
            tracing::warn!("[blue-web] nie udało się utworzyć GtkOverlay — webview zostaje w GtkBox");
            return;
        }
    };

    // Już w overlayu (jako holder albo bezpośrednio)? — tylko zaktualizuj bounds.
    let already_in_overlay = top_of(child).parent().map(|p| p == overlay.clone().upcast::<gtk::Widget>()).unwrap_or(false);

    if !already_in_overlay {
        let keep = child.clone();
        if let Some(parent) = keep.parent().and_then(|p| p.downcast::<gtk::Container>().ok()) {
            parent.remove(&keep);
        }
        if clipping_enabled() {
            let holder = new_holder();
            holder.add(&keep);
            overlay.add_overlay(&holder);
        } else {
            overlay.add_overlay(&keep);
        }
    }

    child.set_halign(gtk::Align::Start);
    child.set_valign(gtk::Align::Start);
    child.set_hexpand(false);
    child.set_vexpand(false);
    let top = top_of(child);
    overlay.set_overlay_pass_through(&top, false);
    // Najnowsze dziecko na wierzch (nad ewentualnymi starszymi kartami).
    overlay.reorder_overlay(&top, -1);

    store_page(child, x, y, width, height);
    apply_placement(child);
    top.show_all();
    overlay.queue_resize();
}

/// Aktualizuje pozycję i rozmiar webview-dziecka. Główny wątek GTK.
pub fn set_bounds(child: &WebView, x: f64, y: f64, width: f64, height: f64) {
    store_page(child, x, y, width, height);
    apply_placement(child);
    if let Some(overlay) = OVERLAY.with(|c| c.borrow().clone()) {
        overlay.queue_resize();
    }
}

/// Cuts the live page to `clip` (shell coordinates), or shows it whole for
/// `None`. Returns `false` when this page has no clipping holder, so the
/// caller can fall back to hiding it.
pub fn set_clip(child: &WebView, clip: Option<(f64, f64, f64, f64)>) -> bool {
    if holder_of(child).is_none() {
        return false;
    }
    let rect = clip.map(|(x, y, w, h)| IRect::new(clamp_i32(x, 0, 32767), clamp_i32(y, 0, 32767), clamp_i32(w, 1, 32767), clamp_i32(h, 1, 32767)));
    PLACED.with(|m| m.borrow_mut().entry(key(child)).or_default().clip = rect);
    apply_placement(child);
    if let Some(overlay) = OVERLAY.with(|c| c.borrow().clone()) {
        overlay.queue_resize();
    }
    true
}

/// Shows / hides the page together with its holder (an empty but visible
/// holder would still swallow clicks meant for the shell underneath).
pub fn set_visible(child: &WebView, visible: bool) {
    let top = top_of(child);
    if visible {
        top.show();
        child.show();
    } else {
        top.hide();
    }
}

/// Takes the page out of its holder and the overlay before it is closed, so
/// nothing is left behind (an empty holder would keep catching clicks).
pub fn detach(child: &WebView) {
    PLACED.with(|m| m.borrow_mut().remove(&key(child)));
    if let Some(holder) = holder_of(child) {
        let keep = child.clone();
        holder.remove(&keep);
        if let Some(parent) = holder.parent().and_then(|p| p.downcast::<gtk::Container>().ok()) {
            parent.remove(&holder);
        }
    }
}

/// `interactive = false` → zdarzenia myszy przechodzą "przez" webview-dziecko
/// do DOM-u powłoki (używane na czas przeciągania/zmiany rozmiaru okna).
pub fn set_interactive(child: &WebView, interactive: bool) {
    if let Some(overlay) = OVERLAY.with(|c| c.borrow().clone()) {
        overlay.set_overlay_pass_through(&top_of(child), !interactive);
    }
}

/// Renders the visible part of `webview` into pixels. Main GTK thread only;
/// `done` runs later on that same thread, once WebKit has finished rendering.
///
/// Only the raw pixel buffer is copied here — colour conversion, downscaling
/// and JPEG encoding happen off the UI thread (see `snapshot.rs`).
pub fn capture<F>(webview: &WebView, done: F)
where
    F: FnOnce(Result<RawSnapshot, String>) + 'static,
{
    webview.snapshot(
        SnapshotRegion::Visible,
        SnapshotOptions::NONE,
        None::<&gio::Cancellable>,
        move |result| done(result.map_err(|e| e.to_string()).and_then(copy_surface)),
    );
}

fn copy_surface(surface: gtk::cairo::Surface) -> Result<RawSnapshot, String> {
    use gtk::cairo::{Format, ImageSurface};

    let mut img = ImageSurface::try_from(surface).map_err(|_| "snapshot is not an image surface".to_string())?;
    let has_alpha = match img.format() {
        Format::ARgb32 => true,
        Format::Rgb24 => false,
        other => return Err(format!("unsupported snapshot pixel format: {other:?}")),
    };
    let (width, height, stride) = (img.width(), img.height(), img.stride());
    if width <= 0 || height <= 0 || stride <= 0 {
        return Err("empty snapshot".to_string());
    }
    img.flush();
    let data = img.data().map_err(|e| e.to_string())?;
    Ok(RawSnapshot {
        width: width as u32,
        height: height as u32,
        stride: stride as usize,
        has_alpha,
        bytes: data.to_vec(),
    })
}

/// Calls `f` whenever the person presses a mouse button inside the page.
///
/// The page is a native surface above the shell's DOM, so such a click never
/// reaches the shell's own `mousedown` handler — without this a Blue Web
/// window that is visible but not focused (another window is on top elsewhere)
/// would not come to the front when clicked.
pub fn on_pressed<F: Fn() + 'static>(child: &WebView, f: F) {
    child.connect_button_press_event(move |_, _| {
        f();
        gtk::glib::Propagation::Proceed
    });
}
