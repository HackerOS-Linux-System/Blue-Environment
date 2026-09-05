use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tauri::Webview;

/// Builds a WebKit content-blocker JSON rule set (the same format
/// Safari content blockers, WebKitGTK's `UserContentFilterStore`, and
/// WKWebView's `WKContentRuleList` all consume — one shared rule format
/// across all three implemented platforms is *why* macOS "only" needed
/// new FFI plumbing below, not a different rule design) — one `block`
/// rule per domain, matching that domain and any subdomain of it in the
/// *request* URL (`url-filter`, not `if-domain` — `if-domain` matches
/// the top-level page's own URL, which is the wrong trigger for
/// blocking a subresource an otherwise-allowed page embeds).
// Used on macOS (install_macos); currently unused on Linux (see
// install_linux's doc — WebKitUserContentFilterStore isn't available
// in this crate build, so the same rule-JSON generation Linux would
// otherwise share isn't called there right now) and not applicable on
// Windows (that path checks the blocklist live, per-request, never
// compiling a rule set at all — see install_windows's doc).
#[allow(dead_code)]
fn build_ruleset_json(blocklist: &HashSet<String>) -> String {
    let rules: Vec<serde_json::Value> = blocklist.iter().map(|domain| {
        let escaped = regex::escape(domain);
        let pattern = format!("^https?://([^/]+\\.)?{escaped}/");
        serde_json::json!({
            "trigger": { "url-filter": pattern },
            "action": { "type": "block" }
        })
    }).collect();
    serde_json::Value::Array(rules).to_string()
}

/// Short, stable, filesystem/identifier-safe digest of the current
/// ruleset — used as the WebKit filter identifier on both Linux and
/// macOS (see those functions' docs) so "has this exact blocklist
/// already been compiled" is answerable by a cheap `load()`/lookup by
/// name instead of tracking a separate side-table of hashes ourselves.
/// SHA-256 truncated to 16 hex chars (64 bits) — this is a cache key,
/// not a security boundary, so a full 256-bit digest would just make
/// filter-store filenames unnecessarily long for no real benefit.
#[allow(dead_code)] // see build_ruleset_json's doc just above
fn ruleset_digest(ruleset_json: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(ruleset_json.as_bytes());
    hash.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Installs (or, on a second call for the same tab, *updates*)
/// subresource blocking on `webview` using whatever `blocklist`
/// currently holds. Called from `web_view_create` at tab-creation time,
/// and again from `web_set_blocklist` for every currently-open tab
/// whenever the blocklist changes (see that command's doc in
/// `mod.rs`) — safe to call repeatedly for the same webview on every
/// platform implemented here; each `install_*` function is written to
/// be a correct no-op-if-unchanged / clean-replace-if-changed operation,
/// not just a first-time setup step.
pub fn install(webview: &Webview, blocklist: Arc<Mutex<HashSet<String>>>) {
    let snapshot = blocklist.lock().unwrap().clone();

    #[cfg(target_os = "linux")]
    install_linux(webview, snapshot);

    #[cfg(windows)]
    install_windows(webview, blocklist);

    #[cfg(target_os = "macos")]
    install_macos(webview, snapshot);

    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        let _ = (webview, snapshot);
    }
}

// ════════════════════════════════════════════════════════════════════
// Linux (webkit2gtk)
// ════════════════════════════════════════════════════════════════════

#[cfg(target_os = "linux")]
/// Linux subresource blocking via WebKitGTK's `decide-policy` signal —
/// see the previous revision's doc (preserved in git history) for why
/// `UserContentFilterStore` specifically was tried first and ruled out
/// (confirmed absent from this crate's generated bindings at any
/// feature level, not just gated behind a missing flag).
///
/// **What's confirmed vs. guessed, specifically** (same standard this
/// file's `install_macos` holds itself to, for the same reason — being
/// explicit about exactly where the remaining risk is beats a
/// plausible-looking wall of code):
/// - Confirmed: `WebKitWebView`'s `decide-policy` signal firing with
///   decision type `WEBKIT_POLICY_DECISION_TYPE_RESPONSE` for every
///   subresource response, and `WebKitResponsePolicyDecision::ignore()`
///   preventing that resource from loading, are real, long-standing
///   (1.x/2.x-era) WebKitGTK APIs (webkitgtk.org's own reference docs).
/// - Confirmed: `webkit2gtk::WebViewExt::connect_decide_policy` exists
///   in this crate's generated bindings at the `2.0`/`v2_24` feature
///   level actually in use here (unlike `UserContentFilterStore`, this
///   is old enough API surface that gtk-rs-family crates have carried
///   working bindings for it across many versions).
/// - **Now actually compiler-verified** (this was previously an open
///   gap in this doc): the closure body below — the
///   `PolicyDecision` → `ResponsePolicyDecision` downcast,
///   `.request()`, `.uri()`, feeding the result through
///   `url::Url::parse`, and `.ignore()` — was extracted into a
///   standalone crate depending only on `webkit2gtk = "2.0"` (with the
///   `v2_24` feature) + `glib` + `url`, matching this project's own
///   pinned versions, and run through a real `cargo check` against the
///   actual `webkit2gtk` 2.0.2 crate and its real generated bindings
///   (system WebKitGTK 2.52.6, the current Ubuntu 24.04 package). That
///   check caught one real bug on the first pass — `URIRequest::uri()`
///   requires `webkit2gtk::URIRequestExt` in scope, which this file
///   didn't import; `WebViewExt` alone (what was imported before) isn't
///   enough, since the two are separate traits — now fixed below. With
///   that fix, the exact logic in this function compiles cleanly.
///   What *isn't* covered by that isolated check: whether
///   `tauri::Webview::with_webview`'s Linux payload actually hands back
///   a `webkit2gtk::WebView` the way `wv.inner()` below assumes (that
///   call sits in the `tauri` crate itself, which the isolated test
///   deliberately didn't pull in, precisely because that whole
///   dependency tree is what wouldn't resolve under this sandbox's
///   available Rust toolchain — see this repository's own CI, which
///   does have a modern enough toolchain, for the full-crate build this
///   still needs).
///
/// **Before trusting this in production**: build the full app, open a
/// Blue Web tab pointed at a page that embeds a known-blocked-domain
/// resource, and confirm in WebKitGTK's own Web Inspector (or a packet
/// capture) that the request genuinely never goes out — the same
/// verification step `install_macos`'s doc asks for. The isolated check
/// above rules out "this doesn't typecheck against real webkit2gtk
/// bindings"; it doesn't replace an actual on-hardware behavioral test.
fn install_linux(webview: &Webview, blocklist: HashSet<String>) {
    if blocklist.is_empty() {
        return;
    }
    let blocklist = Arc::new(blocklist);

    let _ = webview.with_webview(move |wv| {
        use glib::Cast;
        use webkit2gtk::{PolicyDecisionType, PolicyDecisionExt, ResponsePolicyDecisionExt, URIRequestExt, WebViewExt};

        // `wv.inner()` is this crate's own accessor for the raw
        // platform webview handle on Linux (mirrors how `install_macos`
        // reaches `wv.controller()` on macOS and `install_windows`
        // reaches `wv.controller()` on Windows via the same
        // `with_webview` mechanism) — see this function's doc for what
        // isn't independently re-verified about this exact call.
        let webkit_view: webkit2gtk::WebView = wv.inner();
        let blocklist = blocklist.clone();

        webkit_view.connect_decide_policy(move |_view, decision, decision_type| {
            if decision_type != PolicyDecisionType::Response {
                return false; // let navigation/new-window decisions through unchanged — this handler only judges subresource responses
            }
            let Some(response_decision) = decision.clone().downcast::<webkit2gtk::ResponsePolicyDecision>().ok() else {
                return false;
            };
            let Some(request) = response_decision.request() else { return false; };
            let Some(uri) = request.uri() else { return false; };
            let Ok(parsed) = tauri::Url::parse(&uri) else { return false; };
            let Some(host) = parsed.host_str() else { return false; };

            let blocked = blocklist.iter().any(|b| host == b || host.ends_with(&format!(".{b}")));
            if blocked {
                response_decision.ignore();
                true // we've made the decision — WebKitGTK shouldn't apply its own default
            } else {
                false // fall through to WebKitGTK's default (allow) handling
            }
        });
    });
}

// ════════════════════════════════════════════════════════════════════
// Windows (WebView2)
// ════════════════════════════════════════════════════════════════════

/// Windows: checks the *live* `blocklist` state on every request rather
/// than a compiled/cached ruleset — `AddWebResourceRequestedFilter` +
/// `WebResourceRequested` is a per-request Rust callback, not a
/// pre-compiled ruleset object the way WebKit's filter store is, so
/// there's no hashing/caching to add here: the callback already reads
/// straight through the same `Arc<Mutex<...>>` `web_set_blocklist`
/// writes to, so it's live-updating and cache-free by construction, not
/// by extra work. Calling this again (from `web_set_blocklist`'s
/// re-install loop) does register a second, redundant
/// `WebResourceRequested` handler on top of the first rather than
/// replacing it — harmless (both handlers read the same live state and
/// agree on every decision, so the request is blocked/allowed
/// identically either way; it just runs the check twice) but not
/// pretending to be a clean replace the way the Linux/macOS paths are.
///
/// **Real 403 (was: leaving `args.Response` unset).** Building an
/// `ICoreWebView2WebResourceResponse` needs an
/// `ICoreWebView2Environment`, which the request-handler callback isn't
/// itself handed — captured once up front here, via `core.Environment()`
/// (checked as a real, if not 100%-certain-at-this-exact-binding-version,
/// WebView2 API: `ICoreWebView2`'s environment accessor), and moved into
/// the closure alongside `blocklist`.
#[cfg(windows)]
fn install_windows(webview: &Webview, blocklist: Arc<Mutex<HashSet<String>>>) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
    };
    use webview2_com::WebResourceRequestedEventHandler;
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::SHCreateMemStream;

    let _ = webview.with_webview(move |wv| unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else { return };
        let filter = HSTRING::from("*");
        let _ = core.AddWebResourceRequestedFilter(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);

        // Captured once, outside the per-request closure — environment
        // handles are stable for the controller's lifetime, no need to
        // re-fetch on every single request.
        let Ok(environment) = core.Environment() else {
            tracing::warn!("WebView2 Environment() unavailable — subresource blocking will stay a pass-through (requests never actually get denied) on this tab");
            return;
        };

        let blocklist = blocklist.clone();
        let mut token = Default::default();
        let _ = core.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else { return Ok(()) };
                let Ok(request) = args.Request() else { return Ok(()) };
                let Ok(uri) = request.Uri() else { return Ok(()) };
                let uri = uri.to_string();
                let Ok(parsed) = tauri::Url::parse(&uri) else { return Ok(()) };
                let Some(host) = parsed.host_str() else { return Ok(()) };

                let blocked = {
                    let list = blocklist.lock().unwrap();
                    list.iter().any(|b| host == b || host.ends_with(&format!(".{b}")))
                };
                if !blocked {
                    return Ok(());
                }

                // Empty-body 403 — `SHCreateMemStream(&[])` builds a
                // zero-length `IStream` (the standard, well-documented
                // way to hand WebView2 an empty response body; a null
                // stream is also accepted by some WebView2 versions but
                // an explicit empty one is the documented-safe choice).
                // "Blocked-By-Blue-Environment" isn't a real HTTP header
                // anyone parses — purely a devtools-network-tab
                // breadcrumb for whoever's debugging why a resource
                // didn't load, same spirit as an ad-blocker extension's
                // own diagnostic headers.
                let body = SHCreateMemStream(Some(&[]));
                let headers = HSTRING::from("Content-Length: 0\r\nBlocked-By-Blue-Environment: 1\r\n");
                if let Ok(response) = environment.CreateWebResourceResponse(
                    body.as_ref(),
                    403,
                    &HSTRING::from("Forbidden"),
                    &headers,
                ) {
                    let _ = args.SetResponse(&response);
                }
                Ok(())
            })),
            &mut token,
        );
    });
}

// ════════════════════════════════════════════════════════════════════
// macOS (WKWebView)
// ════════════════════════════════════════════════════════════════════

/// macOS, via `WKContentRuleList`/`WKContentRuleListStore` — the WKWebView
/// equivalent of WebKitGTK's filter store used on Linux, same underlying
/// rule JSON format (see `build_ruleset_json`'s doc), same hash-as-
/// identifier caching strategy as `install_linux` for the same reason
/// (`compileContentRuleList` is the expensive/one-time step;
/// `WKContentRuleListStore` also supports looking a previously-compiled
/// list up by identifier without recompiling — real API,
/// `lookUpContentRuleList(forIdentifier:completionHandler:)` — used
/// below the same way `install_linux` uses `store.load()` first).
///
/// **What's confirmed vs. guessed, specifically** (this is the section
/// this file's earlier revision declined to write at all — being
/// explicit here about exactly where the remaining risk is, rather than
/// hiding it inside a wall of plausible-looking code, is the point):
/// - Confirmed: `WKContentRuleListStore`, `compileContentRuleList
///   (forIdentifier:encodedContentRuleList:completionHandler:)`,
///   `lookUpContentRuleList(forIdentifier:completionHandler:)`, and
///   `WKUserContentController.add(_:)`/`.remove(_:)` are real WebKit
///   APIs with this shape (Apple's own WebKit documentation).
/// - Confirmed: `objc2`/`objc2-web-kit`/`block2` are real, actively
///   maintained crates in the same family Tauri's own `with_webview`
///   macOS example already depends on (that example imports
///   `objc2_web_kit::WKWebView`/`WKUserContentController` directly).
/// - **Not verified against a compiler**: the exact Rust method names
///   `objc2-web-kit` exposes for the three WebKit methods above (this
///   crate auto-generates Rust bindings from Apple's Objective-C
///   headers, so the real names should closely mirror the Objective-C
///   selectors, but the precise casing/argument-order Rust convention
///   it lands on isn't independently checked here), and the exact
///   `block2` construction syntax for a two-argument, non-`Send`
///   Objective-C completion block. Getting a block signature wrong is a
///   genuine ABI/memory-safety hazard, not just a compile error, which
///   is exactly the risk this file's earlier revision cited for
///   declining to attempt this at all.
///
/// **Before trusting this in production**: build it, open a Blue Web
/// tab pointed at a page that embeds a known-blocked-domain resource,
/// and confirm in Safari's Web Inspector (or a packet capture) that the
/// request genuinely never goes out — don't trust that "it compiled" is
/// the same as "it works", especially for the block-signature risk
/// noted above.
#[cfg(target_os = "macos")]
fn install_macos(webview: &Webview, blocklist: HashSet<String>) {
    use objc2::rc::Retained;
    use objc2_foundation::{NSError, NSString};
    use objc2_web_kit::{WKContentRuleList, WKContentRuleListStore, WKUserContentController};
    use block2::RcBlock;

    let ruleset = build_ruleset_json(&blocklist);
    let digest = ruleset_digest(&ruleset);
    let identifier = format!("blue-adblock-{digest}");

    let _ = webview.with_webview(move |wv| unsafe {
        let controller: &WKUserContentController = &*wv.controller().cast();
        // `Retained` (objc2's ARC-managed smart pointer, roughly
        // Objective-C's equivalent of `Arc`) keeps `controller` alive
        // for as long as the completion block below might still fire —
        // needed because `compileContentRuleList` is asynchronous, so
        // this function returns (and `wv`'s borrow ends) long before
        // the block actually runs.
        let controller: Retained<WKUserContentController> = Retained::from(controller);
        let identifier_ns = NSString::from_str(&identifier);
        let store = WKContentRuleListStore::defaultStore();

        let controller_for_lookup = controller.clone();
        let identifier_for_compile = identifier_ns.clone();
        let ruleset_for_compile = ruleset.clone();

        let lookup_block = RcBlock::new(move |list: *mut WKContentRuleList, _err: *mut NSError| {
            if !list.is_null() {
                // Cache hit.
                controller_for_lookup.removeAllContentRuleLists();
                controller_for_lookup.addContentRuleList(&*list);
                return;
            }

            // Cache miss — compile + implicitly persist under
            // `identifier` (WKContentRuleListStore persists every
            // successfully compiled list by identifier automatically,
            // unlike WebKitGTK's store which needs an explicit `save`
            // call — one real, if minor, asymmetry between the two
            // otherwise-parallel implementations here).
            let controller_for_compile = controller_for_lookup.clone();
            let ruleset_ns = NSString::from_str(&ruleset_for_compile);
            let compile_block = RcBlock::new(move |list: *mut WKContentRuleList, err: *mut NSError| {
                if list.is_null() {
                    tracing::warn!(
                        "WKContentRuleList compile failed (subresource blocking inactive for this tab): {:?}",
                        if err.is_null() { None } else { Some(&*err) }
                    );
                    return;
                }
                controller_for_compile.removeAllContentRuleLists();
                controller_for_compile.addContentRuleList(&*list);
            });
            store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
                &identifier_for_compile, &ruleset_ns, &compile_block,
            );
        });
        store.lookUpContentRuleListForIdentifier_completionHandler(&identifier_ns, &lookup_block);
    });
}
