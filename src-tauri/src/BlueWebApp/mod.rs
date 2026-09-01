use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Webview, WebviewBuilder, WebviewUrl};
use tauri::webview::{DownloadEvent, NewWindowResponse};

mod content_blocking;
pub mod capability_selftest;

/// Tauri-managed state: every live embedded browser tab's webview,
/// keyed by the frontend's own tab id (not the Tauri webview label,
/// though the label is derived from it — `format!("web-{tab_id}")` —
/// so a tab id must itself already be a valid Tauri window/webview
/// label, which `tabs.ts`'s generated ids already are: `web-<n>`).
#[derive(Default)]
pub struct WebViewRegistry(pub Mutex<HashMap<String, Webview>>);

/// One tracked download — see `web_view_create`'s `.on_download(...)`
/// hook below. Keyed by a compositor-assigned id (not the URL, since
/// the same URL could legitimately be downloaded twice in one session).
#[derive(Default)]
pub struct DownloadRegistry(pub Mutex<HashMap<String, DownloadRecord>>);

/// Domains to block navigation to — see `web_set_blocklist` and
/// `on_navigation`'s check below. Frontend-driven, same as everything
/// else content-blocking related: `webSettings.ts` owns the actual
/// list (built-in starter set + user additions) and pushes it here
/// whenever it changes; this is just where the current snapshot lives
/// so `on_navigation`'s closures (one per tab, created at different
/// times) all see the same, current list rather than each capturing
/// their own copy frozen at tab-creation time.
///
/// Wrapped in `Arc` (not just the bare `Mutex` this used to be) so
/// `content_blocking::install` can hold its own clone of the same
/// shared lock — needed for the Windows subresource-blocking path
/// specifically, which checks this list live on every resource request
/// rather than a frozen-at-creation-time snapshot (see
/// `content_blocking.rs`'s `install_windows` doc) — while
/// `web_set_blocklist`/`on_navigation` keep working exactly as before
/// (`Arc<Mutex<T>>` derefs to `Mutex<T>`, so `.lock()` is unchanged at
/// every existing call site).
#[derive(Default)]
pub struct BlockList(pub std::sync::Arc<Mutex<std::collections::HashSet<String>>>);

#[tauri::command]
pub fn web_set_blocklist(
    blocklist: tauri::State<BlockList>,
    registry: tauri::State<WebViewRegistry>,
    domains: Vec<String>,
) -> Result<(), String> {
    *blocklist.0.lock().unwrap() = domains.into_iter().collect();

    // Live-update subresource blocking on every currently-open tab —
    // previously (Linux specifically) a filter was only ever compiled
    // and attached once, at tab-creation time, so changing the
    // blocklist afterward silently did nothing for already-open tabs
    // until they were reloaded. `content_blocking::install` is cheap to
    // call again here even when nothing changed for a given tab: on
    // Linux it hashes the new ruleset and only actually recompiles
    // (rather than reusing the on-disk-cached filter) when the hash
    // differs from what's already installed — see that module's
    // `install_linux` doc. Windows already checked the live blocklist
    // per-request and doesn't need this at all, but re-`install`ing is
    // harmless for it too (it just re-registers the same filter/handler
    // — see `install_windows`'s idempotency note).
    let webviews: Vec<Webview> = registry.0.lock().unwrap().values().cloned().collect();
    let shared_list = blocklist.0.clone();
    for webview in webviews {
        content_blocking::install(&webview, shared_list.clone());
    }

    Ok(())
}

/// True when `host` (or any parent domain of it — `ads.example.com`
/// matches a blocked `example.com`, but not vice versa) is on the
/// current blocklist. Suffix match on `.`-boundaries only (not a plain
/// substring check — `notexample.com` must not match a blocked
/// `example.com`).
fn host_is_blocked(host: &str, blocklist: &std::collections::HashSet<String>) -> bool {
    blocklist.iter().any(|blocked| host == blocked || host.ends_with(&format!(".{blocked}")))
}

#[derive(Clone, Serialize)]
pub struct DownloadRecord {
    pub id: String,
    pub tab_id: String,
    pub url: String,
    pub filename: String,
    pub path: String,
    pub state: String, // "downloading" | "done" | "error" | "cancelled"
}

#[derive(Serialize)]
pub struct SiteInfo {
    pub title: String,
    pub description: String,
    pub favicon_url: String,
    pub reachable: bool,
}

#[derive(Clone, Serialize)]
pub struct TabMeta {
    pub tab_id: String,
    pub title: String,
    pub favicon_url: String,
}

/// Scoped title/favicon bridge — see this file's module doc ("Update:
/// the scoped title/favicon bridge now exists") for the full security
/// reasoning and its one real caveat. Emits `web-meta-{tab_id}`, the
/// same per-tab event shape `on_navigation` already uses for
/// `web-nav-{tab_id}`, so the frontend has one consistent pattern for
/// "something changed about this specific tab" rather than two.
#[tauri::command]
pub fn web_report_meta(app: AppHandle, tab_id: String, title: String, favicon_url: String) -> Result<(), String> {
    // Defensive length caps — bound how much a broken or malicious page
    // can push through this into the frontend's tab-strip DOM. Chosen
    // generously above what any real title/favicon URL needs (a title
    // this long would already be unreadable, truncated by CSS, in any
    // real tab strip); truncated rather than rejected outright so a
    // legitimately unusual (if long) title still shows *something*
    // instead of the tab silently never updating.
    let title: String = title.chars().take(300).collect();
    let favicon_url: String = favicon_url.chars().take(2000).collect();
    let _ = app.emit(&format!("web-meta-{tab_id}"), TabMeta { tab_id: tab_id.clone(), title, favicon_url });
    Ok(())
}

/// Injected once per tab (via `WebviewBuilder::initialization_script`,
/// so it re-runs on every navigation within that tab — including client-
/// side/SPA navigations that never re-trigger `on_navigation`, which
/// only fires for real top-level navigations — meaning title/favicon
/// tracking here is actually *more* current than the hostname-based
/// fallback the old code relied on for SPAs specifically). Reports via
/// the single scoped command above, nothing else — this script has no
/// more capability than any bookmarklet a real user could paste into
/// their own address bar; the *reporting* side (`web_report_meta`) is
/// what's actually gated, not this script's ability to read
/// `document.title` (which every page already can, about itself,
/// trivially — there's nothing to gate there).
///
/// `{TAB_ID}` is substituted with the real tab id in `web_view_create`
/// below (via `.replace(...)`, not string interpolation of untrusted
/// input — `tab_id` here is this app's own frontend-generated id, e.g.
/// `web-3`, never page-controlled).
const META_REPORT_SCRIPT_TEMPLATE: &str = r#"
(function() {
  var lastTitle = null, lastFavicon = null;
  function faviconHref() {
    var link = document.querySelector('link[rel~="icon"]') || document.querySelector('link[rel~="shortcut icon"]');
    if (link && link.href) return link.href;
    try { return new URL('/favicon.ico', location.href).href; } catch (e) { return ''; }
  }
  function report() {
    var title = document.title || location.hostname || '';
    var favicon = faviconHref();
    if (title === lastTitle && favicon === lastFavicon) return;
    lastTitle = title; lastFavicon = favicon;
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      window.__TAURI_INTERNALS__.invoke('web_report_meta', {
        tabId: "{TAB_ID}", title: title, faviconUrl: favicon
      }).catch(function () { /* command not permitted for this webview — nothing to do */ });
    }
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', report);
  } else {
    report();
  }
  window.addEventListener('load', report);
  // Covers SPA client-side title changes (React Router / etc. setting
  // `document.title` without a real navigation) and late favicon
  // `<link>` insertion, neither of which fires `load` again.
  var headEl = document.querySelector('head') || document.documentElement;
  new MutationObserver(report).observe(headEl, { childList: true, subtree: true, characterData: true });
})();
"#;

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
    let tab_id_for_popup = tab_id.clone();
    let app_for_popup = app.clone();
    let tab_id_for_download = tab_id.clone();
    let app_for_download = app.clone();
    let meta_script = META_REPORT_SCRIPT_TEMPLATE.replace("{TAB_ID}", &tab_id);
    let builder = WebviewBuilder::new(label_for(&tab_id), WebviewUrl::External(parsed))
        .initialization_script(&meta_script)
        .on_navigation(move |navigated_url| {
            // Content blocking — checked against whatever `BlockList`
            // currently holds (see `web_set_blocklist`, driven by
            // `webSettings.ts`'s built-in + custom domain list). This
            // is a per-*navigation* check, not a per-*subresource*
            // check: it can stop the top-level page itself from loading
            // if its own host is blocked, but it can't block an ad
            // script or tracking pixel embedded inside an otherwise
            // allowed page — that would need a request-level hook
            // (e.g. WebView2's `add_WebResourceRequested` /
            // WebKitGTK's URI-scheme-request interception), which wry
            // doesn't expose a cross-platform API for today. A
            // navigation-level check is still real, useful blocking
            // for the common "this whole domain is an ad network"
            // case (see `BUILTIN_BLOCKLIST`), just not a full
            // subresource-level adblocker.
            let blocked = navigated_url.host_str()
                .map(|h| host_is_blocked(h, &app_for_nav.state::<BlockList>().0.lock().unwrap()))
                .unwrap_or(false);
            if blocked {
                return false;
            }
            // Real navigation tracking — fires on every navigation
            // (link clicks, redirects, JS-initiated, not just the
            // initial load), unlike the old code which only ever knew
            // the URL it was originally told to open. The frontend
            // listens for this per-tab event to update the address bar
            // and (derived from the new hostname) the tab title.
            let _ = app_for_nav.emit(&format!("web-nav-{tab_id_for_nav}"), navigated_url.to_string());
            true
        })
        .on_new_window(move |url, _features| {
            // A page's `target="_blank"` link or `window.open()` call —
            // previously this had no handler at all, so the outcome was
            // whatever wry's own platform default happens to be
            // (typically either silently swallowed or, worse, a bare
            // native OS popup window with none of this app's chrome).
            // `Deny` here means it never becomes a real OS window;
            // instead we tell the frontend about the URL so it can open
            // it as an ordinary new Blue Web tab — the behavior people
            // actually expect from "open in new tab" links.
            let _ = app_for_popup.emit(&format!("web-popup-{tab_id_for_popup}"), url.to_string());
            NewWindowResponse::Deny
        })
        .on_download(move |_webview, event| {
            let downloads = app_for_download.state::<DownloadRegistry>();
            match event {
                DownloadEvent::Requested { url, destination } => {
                    // Redirect into the real Downloads folder rather
                    // than whatever default the webview backend would
                    // otherwise pick (often the same directory as the
                    // running binary, which isn't writable/sensible for
                    // an installed app). Falls back to the URL's own
                    // filename, then to a generic name if even that's
                    // unavailable (e.g. a bare directory URL).
                    let dl_dir = app_for_download.path().download_dir().unwrap_or_else(|_| PathBuf::from("."));
                    let filename = url
                        .path_segments()
                        .and_then(|mut s| s.next_back())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("download")
                        .to_string();
                    let mut target = dl_dir.join(&filename);
                    // Don't clobber an existing file of the same name —
                    // append " (n)" before the extension like every
                    // desktop file manager does, rather than silently
                    // overwriting a previous download.
                    if target.exists() {
                        let stem = target.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        let ext = target.extension().map(|s| s.to_string_lossy().to_string());
                        let mut n = 1u32;
                        loop {
                            let candidate_name = match &ext {
                                Some(e) => format!("{stem} ({n}).{e}"),
                                None => format!("{stem} ({n})"),
                            };
                            let candidate = dl_dir.join(&candidate_name);
                            if !candidate.exists() { target = candidate; break; }
                            n += 1;
                        }
                    }
                    *destination = target.clone();

                    let id = format!("dl{}", chrono::Local::now().timestamp_millis());
                    downloads.0.lock().unwrap().insert(id.clone(), DownloadRecord {
                        id: id.clone(),
                        tab_id: tab_id_for_download.clone(),
                        url: url.to_string(),
                        filename: target.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(filename),
                        path: target.to_string_lossy().to_string(),
                        state: "downloading".to_string(),
                    });
                    let _ = app_for_download.emit("web-download-started", downloads.0.lock().unwrap().get(&id).cloned());
                }
                DownloadEvent::Finished { url, path, success } => {
                    // No `id` comes back with `Finished` — this hook only
                    // gives us the URL and final path, not the id we
                    // minted in `Requested`, so we match on those two
                    // fields to find the record again. Safe in practice
                    // (one in-flight download per exact URL+destination
                    // pair at a time is the only case that could
                    // collide, and even then both records just get the
                    // same, correct final state).
                    let mut reg = downloads.0.lock().unwrap();
                    if let Some(rec) = reg.values_mut().find(|r| {
                        r.url == url.to_string() && r.state == "downloading"
                            && path.as_ref().map(|p| r.path == p.to_string_lossy()).unwrap_or(true)
                    }) {
                        rec.state = if success { "done".to_string() } else { "error".to_string() };
                        let _ = app_for_download.emit("web-download-finished", rec.clone());
                    }
                }
                _ => {}
            }
            true // let it proceed to `destination` (or the default handling for whatever variant this non_exhaustive enum grows next)
        });

    let webview = window
        .add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(width, height))
        .map_err(|e| format!("failed to create embedded webview: {e}"))?;

    // Subresource-level blocking — see content_blocking.rs's module doc
    // for exactly what this covers per-platform (real on Linux/Windows,
    // not yet on macOS) and why it's a separate step from the
    // navigation-level `host_is_blocked` check inside `on_navigation`
    // above (that one only stops the top-level page itself from
    // loading; this one can also stop an ad script or tracking pixel a
    // still-allowed page embeds).
    content_blocking::install(&webview, app.state::<BlockList>().0.clone());

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

/// Page zoom — real per-webview zoom via `Webview::set_zoom` (confirmed
/// against the tauri 2.9.1 crate source), not a CSS `zoom` hack applied
/// through injected JS. `factor` is an absolute scale (1.0 = 100%), not
/// a delta — the frontend (`AddressBar.svelte`) tracks each tab's
/// current zoom level itself and always sends the resulting absolute
/// value, so this command can stay a thin, stateless wrapper.
#[tauri::command]
pub fn web_view_set_zoom(registry: tauri::State<WebViewRegistry>, tab_id: String, factor: f64) -> Result<(), String> {
    let reg = registry.0.lock().unwrap();
    let Some(webview) = reg.get(&tab_id) else { return Ok(()) };
    webview.set_zoom(factor.clamp(0.25, 5.0)).map_err(|e| e.to_string())
}

/// Find-in-page. There's no Tauri/wry API to run a native browser
/// find-bar, and — per this file's module doc on why no JS bridge is
/// injected into page content — there's also no channel for the page to
/// report a match count back to Rust. So this works the same way a
/// bookmarklet would: `webview.eval(...)` runs a small, self-contained
/// script that walks text nodes, wraps matches in a highlight `<mark>`,
/// and scrolls the current match into view, keeping its own cursor
/// state in a `window.__blueFind` object between calls (persists for
/// the page's lifetime, i.e. until the next navigation). `eval` fires
/// the script into the page and returns `Ok(())` immediately — it
/// doesn't and can't wait for or return the script's result, so the
/// frontend has no match-count UI: only "found something" vs. "found
/// nothing" is even theoretically knowable, and this command doesn't
/// try to surface even that (silent no-highlight is the failure mode
/// for a 0-match query — a real browser's "Phrase not found" toast is a
/// nicety this can't cheaply replicate without a bridge).
#[tauri::command]
pub fn web_view_find(registry: tauri::State<WebViewRegistry>, tab_id: String, query: String, backwards: bool) -> Result<(), String> {
    let reg = registry.0.lock().unwrap();
    let Some(webview) = reg.get(&tab_id) else { return Ok(()) };
    let q = serde_json::to_string(&query).map_err(|e| e.to_string())?;
    let dir = if backwards { "-1" } else { "1" };
    let script = format!(r#"(function() {{
        const q = {q};
        const state = window.__blueFind || (window.__blueFind = {{ query: '', marks: [], idx: -1 }});
        if (state.query !== q) {{
            state.marks.forEach(m => {{ const p = m.parentNode; if (p) {{ p.replaceChild(document.createTextNode(m.textContent), m); p.normalize(); }} }});
            state.marks = [];
            state.idx = -1;
            state.query = q;
            if (q.length > 0) {{
                const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {{
                    acceptNode: n => (n.parentElement && n.parentElement.tagName !== 'SCRIPT' && n.parentElement.tagName !== 'STYLE' && n.nodeValue.toLowerCase().includes(q.toLowerCase())) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT
                }});
                const targets = [];
                let n;
                while ((n = walker.nextNode())) targets.push(n);
                targets.forEach(textNode => {{
                    const lower = textNode.nodeValue.toLowerCase();
                    const needle = q.toLowerCase();
                    let start = 0, i;
                    const frag = document.createDocumentFragment();
                    while ((i = lower.indexOf(needle, start)) !== -1) {{
                        frag.appendChild(document.createTextNode(textNode.nodeValue.slice(start, i)));
                        const mark = document.createElement('mark');
                        mark.style.cssText = 'background:#fde047;color:#111;';
                        mark.textContent = textNode.nodeValue.slice(i, i + needle.length);
                        frag.appendChild(mark);
                        state.marks.push(mark);
                        start = i + needle.length;
                    }}
                    frag.appendChild(document.createTextNode(textNode.nodeValue.slice(start)));
                    textNode.parentNode.replaceChild(frag, textNode);
                }});
            }}
        }}
        if (state.marks.length === 0) return;
        if (state.idx >= 0 && state.marks[state.idx]) state.marks[state.idx].style.outline = '';
        state.idx = ((state.idx + ({dir})) % state.marks.length + state.marks.length) % state.marks.length;
        const current = state.marks[state.idx];
        current.style.outline = '2px solid #f97316';
        current.scrollIntoView({{ block: 'center', behavior: 'smooth' }});
    }})();"#);
    webview.eval(script).map_err(|e| e.to_string())
}

/// Clears find-in-page highlighting — called when the find bar closes.
/// Simplest correct implementation is just re-running the same
/// highlight/unwrap logic with an empty query, which `web_view_find`'s
/// script already treats as "tear down existing marks, add none back".
#[tauri::command]
pub fn web_view_clear_find(registry: tauri::State<WebViewRegistry>, tab_id: String) -> Result<(), String> {
    web_view_find(registry, tab_id, String::new(), false)
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

/// Snapshot of every download this session knows about, most recent
/// first — backs the Downloads panel in `SidePanel.svelte`. Session-only
/// (in-memory `DownloadRegistry`, not persisted to disk): a real browser
/// keeps download history across restarts in a database, which is a
/// reasonable follow-up but a different, larger feature (needs its own
/// storage, not just an in-memory `HashMap`) than what's implemented
/// here.
#[tauri::command]
pub fn web_downloads_list(registry: tauri::State<DownloadRegistry>) -> Vec<DownloadRecord> {
    let mut v: Vec<DownloadRecord> = registry.0.lock().unwrap().values().cloned().collect();
    v.sort_by(|a, b| b.id.cmp(&a.id)); // id embeds a millis timestamp, so this is newest-first
    v
}

/// Removes a completed/errored download from the tracked list — does
/// *not* delete the downloaded file itself, matching what "remove from
/// downloads" means in every mainstream browser (the file stays on
/// disk; this only clears the entry from the panel).
#[tauri::command]
pub fn web_download_remove(registry: tauri::State<DownloadRegistry>, id: String) -> Result<(), String> {
    registry.0.lock().unwrap().remove(&id);
    Ok(())
}

/// Reveals a downloaded file by opening its containing folder in the
/// OS's default file manager, via `tauri-plugin-shell` (already a
/// dependency — see Cargo.toml/main.rs's `.plugin(tauri_plugin_shell::
/// init())`). This opens the *folder*, not a file-manager-specific
/// "select this exact file" action — wry/tauri has no cross-platform
/// API for the latter (that'd mean shelling out to platform-specific
/// tools like `explorer.exe /select,` or a `nautilus --select` style
/// invocation per file manager), so this is the honest, portable
/// version of "show me where that went" rather than a fragile
/// per-desktop-environment special case.
#[tauri::command]
pub fn web_download_reveal(app: AppHandle, registry: tauri::State<DownloadRegistry>, id: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = registry.0.lock().unwrap().get(&id).map(|r| r.path.clone())
        .ok_or_else(|| format!("no download with id {id}"))?;
    let parent = std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or(path);
    // `tauri_plugin_shell::Shell::open` (used here previously) is
    // deprecated in favor of this plugin — real deprecation warning
    // from a `cargo build`, migrated rather than silenced. `open_path`
    // is `OpenerExt`'s equivalent of the old `shell().open(path, None)`
    // call: hand a path to the OS's default handler for it (a folder,
    // here, so "open with" resolves to the file manager) — `None` for
    // "with" (no specific app override), same default behavior as
    // before.
    app.opener().open_path(parent, None::<&str>).map_err(|e| e.to_string())
}
