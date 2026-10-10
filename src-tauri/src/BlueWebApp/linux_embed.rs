use gtk::prelude::*;
use std::cell::RefCell;
use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebView, WebViewExt};

use super::snapshot::RawSnapshot;

thread_local! {
    /// Overlay okna powłoki. Dostępny tylko z głównego wątku GTK.
    static OVERLAY: RefCell<Option<gtk::Overlay>> = const { RefCell::new(None) };
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

fn apply_bounds(webview: &WebView, x: f64, y: f64, width: f64, height: f64) {
    let x = clamp_i32(x, 0, 32767);
    let y = clamp_i32(y, 0, 32767);
    let w = clamp_i32(width, 1, 32767);
    let h = clamp_i32(height, 1, 32767);

    if webview.margin_start() != x {
        webview.set_margin_start(x);
    }
    if webview.margin_top() != y {
        webview.set_margin_top(y);
    }
    let (cw, ch) = webview.size_request();
    if cw != w || ch != h {
        webview.set_size_request(w, h);
    }
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

    // Już w overlayu? (np. podwójne wywołanie) — tylko zaktualizuj bounds.
    let already_in_overlay = child
        .parent()
        .map(|p| p == overlay.clone().upcast::<gtk::Widget>())
        .unwrap_or(false);

    if !already_in_overlay {
        let keep = child.clone();
        if let Some(parent) = keep
            .parent()
            .and_then(|p| p.downcast::<gtk::Container>().ok())
        {
            parent.remove(&keep);
        }
        overlay.add_overlay(&keep);
    }

    child.set_halign(gtk::Align::Start);
    child.set_valign(gtk::Align::Start);
    child.set_hexpand(false);
    child.set_vexpand(false);
    overlay.set_overlay_pass_through(child, false);
    // Najnowsze dziecko na wierzch (nad ewentualnymi starszymi kartami).
    overlay.reorder_overlay(child, -1);

    apply_bounds(child, x, y, width, height);
    child.show_all();
    overlay.queue_resize();
}

/// Aktualizuje pozycję i rozmiar webview-dziecka. Główny wątek GTK.
pub fn set_bounds(child: &WebView, x: f64, y: f64, width: f64, height: f64) {
    apply_bounds(child, x, y, width, height);
    if let Some(overlay) = OVERLAY.with(|c| c.borrow().clone()) {
        overlay.queue_resize();
    }
}

/// `interactive = false` → zdarzenia myszy przechodzą "przez" webview-dziecko
/// do DOM-u powłoki (używane na czas przeciągania/zmiany rozmiaru okna).
pub fn set_interactive(child: &WebView, interactive: bool) {
    if let Some(overlay) = OVERLAY.with(|c| c.borrow().clone()) {
        overlay.set_overlay_pass_through(child, !interactive);
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
