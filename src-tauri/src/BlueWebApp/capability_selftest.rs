use tauri::{AppHandle, Listener, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl};

const FORBIDDEN_PROBE_COMMAND: &str = "web_downloads_list";
const SELFTEST_TAB_ID: &str = "__selftest__";

pub fn run(app: &AppHandle) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some(window) = app.get_window("main") else { return };

    let event_name = format!("web-meta-{SELFTEST_TAB_ID}");
    let app_for_listener = app.clone();
    // `Manager::listen` (not `Webview::title()` — see this module's
    // revision note) — subscribes to the same app-wide event
    // `web_report_meta` already emits for every real tab via
    // `app.emit(&format!("web-meta-{tab_id}"), ...)` in `mod.rs`; the
    // probe page below just deliberately uses tab id `"__selftest__"`
    // so this listener only ever sees its own probe's result, never a
    // real tab's.
    app.listen(event_name.clone(), move |event| {
        let payload = event.payload();
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(payload);
        let Ok(value) = parsed else {
            tracing::warn!("capability self-test: couldn't parse probe result payload: {payload}");
            return;
        };
        let Some(json) = value.get("title").and_then(|t| t.as_str()) else {
            tracing::warn!("capability self-test: probe result had no title field: {payload}");
            return;
        };
        if json.contains("\"forbidden_blocked\":true") && json.contains("\"allowed_command_worked\":true") {
            tracing::info!("capability self-test PASSED: web-* webviews correctly denied {FORBIDDEN_PROBE_COMMAND} and allowed web_report_meta");
        } else {
            tracing::error!(
                "capability self-test FAILED ({json}) — web-* webview capability scoping is NOT \
                 behaving as web-content.json/default.json intend. If forbidden_blocked is false, \
                 embedded page content can currently reach commands it shouldn't (default.json's \
                 broad grant, or a Tauri capability-matching bug — see mod.rs's module doc). \
                 Do not treat the title/favicon bridge's scoping as a real security boundary \
                 until this is fixed."
            );
        }
        // One-shot — this listener has nothing left to do once the
        // single probe webview (created below, never recreated) has
        // reported its one result.
        app_for_listener.unlisten(event.id());
    });

    let html = format!(
        r#"data:text/html,<script>
(async function() {{
  var result = {{ forbidden_blocked: null, allowed_command_worked: null }};
  try {{
    await window.__TAURI_INTERNALS__.invoke('{forbidden}', {{}});
    result.forbidden_blocked = false;
  }} catch (e) {{
    result.forbidden_blocked = true;
  }}
  try {{
    await window.__TAURI_INTERNALS__.invoke('web_report_meta', {{ tabId: '{tab_id}', title: JSON.stringify(result), faviconUrl: '' }});
    result.allowed_command_worked = true;
  }} catch (e) {{
    result.allowed_command_worked = false;
    try {{
      await window.__TAURI_INTERNALS__.invoke('web_report_meta', {{ tabId: '{tab_id}', title: JSON.stringify(result), faviconUrl: '' }});
    }} catch (e2) {{ /* if web_report_meta itself is denied, there is genuinely no way
                        left for this probe to report out — see this module's doc,
                        this is the one failure mode that shows up as "no event ever
                        arrives" rather than a captured FAILED result */ }}
  }}
}})();
</script>"#,
        forbidden = FORBIDDEN_PROBE_COMMAND,
        tab_id = SELFTEST_TAB_ID,
    );

    let builder = WebviewBuilder::new("web-selftest", WebviewUrl::External(
        html.parse().expect("static data: URL is always valid"),
    ));
    // No `.visible(false)` — that method doesn't exist on
    // `WebviewBuilder` either (same real-compiler-error basis as the
    // `.title()` fix above). 1x1 logical pixels at (0,0) is the
    // available fallback for "as unobtrusive as possible" — a
    // sub-pixel flicker in the corner on debug-build startup, not
    // invisible, but not worth more API-guessing to chase further.
    let Ok(webview) = window.add_child(builder, LogicalPosition::new(0, 0), LogicalSize::new(1, 1)) else {
        tracing::warn!("capability self-test: failed to create probe webview, skipping");
        return;
    };

    // Best-effort cleanup after the probe has had time to run and
    // report — same fixed-delay simplification as before (see this
    // module's original doc reasoning, unchanged: "not a general
    // security audit", a debug-only diagnostic doesn't need a real
    // completion signal to close its own probe webview on).
    let webview_for_cleanup = webview.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        let _ = webview_for_cleanup.close();
    });
}
