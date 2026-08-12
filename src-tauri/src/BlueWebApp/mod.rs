use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Webview, WebviewBuilder, WebviewUrl};

/// Tauri-managed state: every live embedded browser tab's webview,
/// keyed by the frontend's own tab id (not the Tauri webview label,
/// though the label is derived from it — `format!("web-{tab_id}")` —
/// so a tab id must itself already be a valid Tauri window/webview
/// label, which `tabs.ts`'s generated ids already are: `web-<n>`).
#[derive(Default)]
pub struct WebViewRegistry(pub Mutex<HashMap<String, Webview>>);

#[derive(Serialize)]
pub struct SiteInfo {
    pub title: String,
    pub description: String,
    pub favicon_url: String,
    pub reachable: bool,
}

fn label_for(tab_id: &str) -> String {
    format!("web-{tab_id}")
}

/// Creates the embedded webview for a newly-opened tab and immediately
/// positions/sizes it. `window_label` is the shell's single OS window
/// (always `"main"` in this codebase, but passed explicitly rather than
/// hardcoded so this doesn't silently break if that ever changes).
#[tauri::command]
pub fn web_view_create(
    app: AppHandle,
    registry: tauri::State<WebViewRegistry>,
    window_label: String,
    tab_id: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if width <= 0.0 || height <= 0.0 {
        // The content area can legitimately be zero-sized for one frame
        // during initial layout, before the first real
        // getBoundingClientRect() measurement — silently no-op rather
        // than create a degenerate webview the person would never see
        // and that'd need cleaning up.
        return Ok(());
    }

    let window = app
        .get_window(&window_label)
        .ok_or_else(|| format!("no such window: {window_label}"))?;

    let parsed = tauri::Url::parse(&url).map_err(|e| format!("invalid URL: {e}"))?;

    let tab_id_for_nav = tab_id.clone();
    let app_for_nav = app.clone();
    let builder = WebviewBuilder::new(label_for(&tab_id), WebviewUrl::External(parsed)).on_navigation(
        move |navigated_url| {
            // Real navigation tracking — fires on every navigation
            // (link clicks, redirects, JS-initiated, not just the
            // initial load), unlike the old code which only ever knew
            // the URL it was originally told to open. The frontend
            // listens for this per-tab event to update the address bar
            // and (derived from the new hostname) the tab title.
            let _ = app_for_nav.emit(&format!("web-nav-{tab_id_for_nav}"), navigated_url.to_string());
            true // never block navigation — this isn't a content filter
        },
    );

    let webview = window
        .add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(width, height))
        .map_err(|e| format!("failed to create embedded webview: {e}"))?;

    registry.0.lock().unwrap().insert(tab_id, webview);
    Ok(())
}

#[tauri::command]
pub fn web_view_navigate(registry: tauri::State<WebViewRegistry>, tab_id: String, url: String) -> Result<(), String> {
    let parsed = tauri::Url::parse(&url).map_err(|e| format!("invalid URL: {e}"))?;
    let reg = registry.0.lock().unwrap();
    let webview = reg.get(&tab_id).ok_or_else(|| format!("no webview for tab {tab_id}"))?;
    webview.navigate(parsed).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn web_view_reload(registry: tauri::State<WebViewRegistry>, tab_id: String) -> Result<(), String> {
    let reg = registry.0.lock().unwrap();
    let webview = reg.get(&tab_id).ok_or_else(|| format!("no webview for tab {tab_id}"))?;
    webview.reload().map_err(|e| e.to_string())
}

/// Called continuously (via rAF polling on the frontend, see module doc)
/// while Blue Web's content area might be moving — dragging/resizing the
/// window, switching workspaces, etc. Cheap no-op-if-unchanged is the
/// frontend's job (it only calls this when the measured rect actually
/// differs from the last one it sent); this command doesn't itself
/// debounce.
#[tauri::command]
pub fn web_view_set_bounds(
    registry: tauri::State<WebViewRegistry>,
    tab_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let reg = registry.0.lock().unwrap();
    let Some(webview) = reg.get(&tab_id) else { return Ok(()) }; // tab may have just closed; not an error
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }
    webview.set_position(LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    webview.set_size(LogicalSize::new(width, height)).map_err(|e| e.to_string())
}

/// Tab switching: only the active tab's webview should be visible.
/// Hiding rather than destroying keeps background tabs' state (scroll
/// position, form input, in-page JS state) alive across switches,
/// matching how every real browser's tabs behave.
#[tauri::command]
pub fn web_view_set_visible(registry: tauri::State<WebViewRegistry>, tab_id: String, visible: bool) -> Result<(), String> {
    let reg = registry.0.lock().unwrap();
    let Some(webview) = reg.get(&tab_id) else { return Ok(()) };
    if visible { webview.show().map_err(|e| e.to_string()) } else { webview.hide().map_err(|e| e.to_string()) }
}

#[tauri::command]
pub fn web_view_close(registry: tauri::State<WebViewRegistry>, tab_id: String) -> Result<(), String> {
    if let Some(webview) = registry.0.lock().unwrap().remove(&tab_id) {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Opens `url` in a genuinely separate native OS window — kept as a
/// real, deliberate feature (a "pop out to its own window" action,
/// useful for e.g. a video call or a picture-in-picture-style site)
/// rather than removed, but no longer Blue Web's *only* way to show a
/// page. Unchanged from before.
#[tauri::command]
pub fn web_open_native(url: String, app: tauri::AppHandle) -> Result<String, String> {
    use tauri::WebviewWindowBuilder;

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("Invalid URL: {}", url));
    }

    let label = format!("web-{}", chrono::Local::now().timestamp_millis());
    let title = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(&url)
        .to_string();

    WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(url.parse().map_err(|e| format!("URL parse error: {}", e))?))
        .title(title)
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .decorations(true)
        .build()
        .map_err(|e| format!("Failed to open window: {}", e))?;

    Ok(url)
}

/// Fetches basic metadata (title, description, favicon) for `url`.
/// Used by the new-tab page to show site previews.
/// Intentionally a fire-and-forget async operation — returns empty fields
/// rather than erroring when the network is unavailable.
#[tauri::command]
pub async fn web_fetch_site_info(url: String) -> SiteInfo {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (like Blue/0.6)")
        .build();

    let Ok(client) = client else {
        return SiteInfo { title: String::new(), description: String::new(), favicon_url: String::new(), reachable: false };
    };

    let Ok(resp) = client.get(&url).send().await else {
        return SiteInfo { title: String::new(), description: String::new(), favicon_url: String::new(), reachable: false };
    };

    let Ok(body) = resp.text().await else {
        return SiteInfo { title: String::new(), description: String::new(), favicon_url: String::new(), reachable: false };
    };

    let title = extract_tag(&body, "title").unwrap_or_default();
    let description = extract_meta(&body, "description").unwrap_or_default();
    let host = url.split('/').nth(2).unwrap_or("").to_string();
    let favicon_url = format!("https://www.google.com/s2/favicons?sz=32&domain={}", host);

    SiteInfo { title, description, favicon_url, reachable: true }
}

fn extract_tag(html: &str, tag: &str) -> Option<String> {
    let open  = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let start = html.to_lowercase().find(&open)?;
    let content_start = html[start..].find('>')? + start + 1;
    let content_end   = html[content_start..].to_lowercase().find(&close)? + content_start;
    Some(html[content_start..content_end].trim().to_string())
}

fn extract_meta(html: &str, name: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let needle = format!("name=\"{}\"", name);
    let pos = lower.find(&needle)?;
    let after = &html[pos..];
    let content_pos = after.to_lowercase().find("content=\"")? + "content=\"".len();
    let end = after[content_pos..].find('"')?;
    Some(after[content_pos..content_pos + end].trim().to_string())
}
