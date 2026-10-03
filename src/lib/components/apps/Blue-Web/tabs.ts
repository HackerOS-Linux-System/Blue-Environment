import { writable, get } from 'svelte/store';
import { tick } from 'svelte';
import type { Tab, SearchEngineId } from './types';
import { normalizeUrl } from './types';
import { SystemBridge } from '../../../utils/systemBridge';

function makeTab(url = '', isPrivate = false): Tab {
  const id = `t${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const title = url ? (() => { try { return new URL(url).hostname; } catch { return url; } })() : (isPrivate ? 'New private tab' : 'New Tab');
  return { id, url, title, isNew: !url, isPrivate, zoom: 1 };
}

/** Thin wrapper around `@tauri-apps/api/event`'s `listen` matching the
 * pattern already used throughout `systemBridge.ts` (import the module
 * lazily, swallow errors into a no-op unlisten fn outside Tauri) — kept
 * local to this module rather than added to the big `SystemBridge`
 * object because every caller here needs a *dynamic*, per-tab event
 * name (`web-nav-${id}`, `web-popup-${id}`), which doesn't fit
 * `SystemBridge`'s existing one-fixed-event-name-per-method shape. */
async function listenEvent(name: string, cb: (payload: any) => void): Promise<() => void> {
  if (!SystemBridge.isTauri()) return () => {};
  try {
    const mod = await import('@tauri-apps/api/event');
    return await mod.listen(name, (e: any) => cb(e.payload));
  } catch {
    return () => {};
  }
}

// Which tab ids already have a live embedded webview (backend-side) —
// tracked here, not just derived from `!tab.isNew`, because a tab keeps
// its webview alive across tab switches (hidden, not destroyed — see
// BlueWebApp.svelte's module doc) even though `isNew` only describes
// whether it's ever been navigated at all.
const liveWebviews = new Set<string>();

// Per-tab unlisten functions for the nav/popup events created alongside
// each tab's webview — previously this bookkeeping didn't exist at all
// (BlueWebApp.svelte declared an `unlistenNav` array but nothing ever
// pushed into it, so the backend's `web-nav-{tab_id}` events had no
// listener: the address bar never updated on in-page navigation,
// redirects, or link clicks, only on navigations this frontend itself
// initiated). Keyed by tab id so `closeTab` can clean up exactly the
// right listeners.
const tabUnlisten = new Map<string, (() => void)[]>();

export interface WebRect { x: number; y: number; width: number; height: number }

// Tabs whose embedded webview is being created right now (`web_view_create`
// is async). A second navigation for the same tab while that's in flight
// must wait for it instead of racing it — otherwise two creates for one
// tab id could run, or the second navigate could hit "no webview for tab".
const pendingCreate = new Map<string, Promise<void>>();

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export function createTabs(
  onNavigate: (url: string, tabId: string) => void,
  onFavicon?: (tabId: string, favicon: string) => void,
  getSearchEngine?: () => SearchEngineId,
  getDefaultZoom?: () => number,
  /** Current on-screen rect of the content area (null if not laid out). */
  getBounds?: () => WebRect | null,
  /** Whether this window's webviews may be shown right now (topmost, no overlay). */
  getShouldBeVisible?: () => boolean,
  /** Called after a tab's webview exists (so the caller can force a bounds push). */
  onWebviewCreated?: (tabId: string) => void,
  /** Called when a webview could not be created at all. */
  onCreateError?: (message: string) => void,
) {
  const first = makeTab();
  if (getDefaultZoom) first.zoom = getDefaultZoom();
  const tabs = writable<Tab[]>([first]);
  const activeId = writable(first.id);
  // Most-recently-closed tabs, for Ctrl+Shift+T — a plain URL stack
  // (not full Tab objects) is enough to reopen "the page that was
  // there", which is what people actually want back, not the exact
  // scroll position/history stack of the closed tab.
  const closedStack: string[] = [];

  async function attachTabListeners(id: string) {
    const navUn = await listenEvent(`web-nav-${id}`, (url: string) => {
      const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();
      tabs.update((prev) => prev.map((t) => (t.id === id ? { ...t, url, title } : t)));
      onNavigate(url, id);
      fetchFaviconFor(id, url);
    });
    // Real title + favicon, straight from the page's own DOM (see
    // `web_report_meta`/`META_REPORT_SCRIPT_TEMPLATE` in mod.rs) —
    // supersedes the hostname-only title `web-nav-*` above sets and the
    // third-party favicon-proxy guess `fetchFaviconFor` makes below,
    // whenever the page actually reports something (a page that never
    // loads, e.g. blocked by the popup/download flow before it paints,
    // simply never fires this, and the `web-nav-*`/`fetchFaviconFor`
    // fallbacks above are what's shown instead — this listener only
    // ever *upgrades* the display, never regresses it to something
    // worse, since it's additive to what's already there). Also the
    // only path that ever reflects an SPA's client-side title change,
    // since `web-nav-*` only fires on real top-level navigations.
    const metaUn = await listenEvent(`web-meta-${id}`, (meta: { title?: string; favicon_url?: string }) => {
      tabs.update((prev) => prev.map((t) => {
        if (t.id !== id) return t;
        const title = meta.title && meta.title.trim() ? meta.title : t.title;
        return { ...t, title };
      }));
      if (meta.favicon_url && onFavicon) onFavicon(id, meta.favicon_url);
    });
    const popupUn = await listenEvent(`web-popup-${id}`, (url: string) => {
      // A page's target="_blank"/window.open() call — see mod.rs's
      // `on_new_window` doc comment. Opens as a normal new tab right
      // next to the one that spawned it, the way every real browser's
      // "open in new tab" behaves.
      const t = makeTab();
      tabs.update((prev) => {
        const idx = prev.findIndex((x) => x.id === id);
        const next = [...prev];
        next.splice(idx + 1, 0, t);
        return next;
      });
      activeId.set(t.id);
      openUrl(url, t.id);
    });
    tabUnlisten.set(id, [navUn, metaUn, popupUn]);
  }

  function fetchFaviconFor(id: string, url: string) {
    if (!SystemBridge.isTauri() || !onFavicon) return;
    SystemBridge.invokeCommand('web_fetch_site_info', { url })
      .then((info: any) => { if (info?.favicon_url) onFavicon(id, info.favicon_url); })
      .catch(() => {});
  }

  /**
   * Navigate a tab to `rawUrl`. First navigation for a tab creates its
   * embedded webview (needs real pixel bounds, since this module has no
   * DOM access — BlueWebApp.svelte supplies them through `getBounds`, or
   * the caller passes an already-measured rect); subsequent navigations in
   * the same tab just call `web_view_navigate` on the existing one.
   *
   * IMPORTANT ordering: the tab is flipped to `isNew: false` FIRST and we
   * then wait for Svelte to render the content placeholder (`tick()` + one
   * animation frame) BEFORE measuring. The old code measured while the
   * new-tab page was still on screen, got `null`, and created the webview
   * at a made-up 800x600 rect at (0,0) — on top of the shell's top bar.
   */
  async function openUrl(rawUrl: string, tabId?: string, bounds?: WebRect) {
    const url = normalizeUrl(rawUrl, getSearchEngine?.());
    const id = tabId ?? get(activeId);
    const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();

    tabs.update((prev) => prev.map((t) => (t.id === id ? { ...t, url, title, isNew: false } : t)));
    onNavigate(url, id);

    if (!SystemBridge.isTauri()) {
      window.open(url, '_blank', 'noopener');
      return;
    }

    // Another create for this tab is in flight — let it finish, then navigate.
    const inFlight = pendingCreate.get(id);
    if (inFlight) {
      await inFlight;
      if (liveWebviews.has(id)) {
        try { await SystemBridge.invokeCommand('web_view_navigate', { tabId: id, url }); } catch {}
      }
      return;
    }

    if (liveWebviews.has(id)) {
      try { await SystemBridge.invokeCommand('web_view_navigate', { tabId: id, url }); } catch {}
      fetchFaviconFor(id, url);
      return;
    }

    const creation = createWebview(id, url, bounds);
    pendingCreate.set(id, creation);
    try { await creation; } finally { pendingCreate.delete(id); }
  }

  /** Waits until the content placeholder is mounted and measurable. */
  async function measureWhenReady(fallback?: WebRect): Promise<WebRect | null> {
    await tick();
    for (let attempt = 0; attempt < 8; attempt++) {
      const b = getBounds?.() ?? null;
      if (b && b.width > 1 && b.height > 1) return b;
      // Not laid out yet — wait a frame and look again.
      await new Promise<void>((r) => requestAnimationFrame(() => r()));
    }
    return fallback && fallback.width > 1 && fallback.height > 1 ? fallback : null;
  }

  async function createWebview(id: string, url: string, passedBounds?: WebRect) {
    let lastErr = '';
    for (let attempt = 0; attempt < 3; attempt++) {
      // Always prefer a fresh measurement over the caller's (possibly stale) one.
      const b = await measureWhenReady(passedBounds);
      if (!b) { lastErr = 'content area not laid out'; await sleep(60); continue; }
      // The tab may have been closed while we were waiting.
      if (!get(tabs).some((t) => t.id === id)) return;
      try {
        await SystemBridge.invokeCommand('web_view_create', {
          windowLabel: 'main', tabId: id, url, x: b.x, y: b.y, width: b.width, height: b.height,
        });
        // Tab closed during the (async) creation → don't leak the webview.
        if (!get(tabs).some((t) => t.id === id)) {
          try { await SystemBridge.invokeCommand('web_view_close', { tabId: id }); } catch {}
          return;
        }
        liveWebviews.add(id);
        await attachTabListeners(id);
        fetchFaviconFor(id, url);

        // Re-apply visibility rules immediately: a freshly created webview is
        // visible by default, which is wrong if this tab isn't the active one
        // or the window is currently covered / an overlay is open.
        const shouldShow = id === get(activeId) && (getShouldBeVisible ? getShouldBeVisible() : true);
        if (!shouldShow) {
          try { await SystemBridge.invokeCommand('web_view_set_visible', { tabId: id, visible: false }); } catch {}
        }
        onWebviewCreated?.(id);

        const zoom = get(tabs).find((t) => t.id === id)?.zoom;
        if (zoom && zoom !== 1) {
          try { await SystemBridge.invokeCommand('web_view_set_zoom', { tabId: id, factor: zoom }); } catch {}
        }
        return;
      } catch (e: any) {
        lastErr = String(e?.message ?? e);
        await sleep(80);
      }
    }
    onCreateError?.(lastErr || 'failed to create the embedded webview');
  }

  function addTab(isPrivate = false) {
    const t = makeTab('', isPrivate);
    if (getDefaultZoom) t.zoom = getDefaultZoom();
    tabs.update((prev) => [...prev, t]);
    activeId.set(t.id);
    return t.id;
  }

  async function closeTab(id: string, e?: MouseEvent) {
    e?.stopPropagation();
    const current = get(activeId);
    const closed = get(tabs).find((t) => t.id === id);
    if (closed && !closed.isNew && !closed.isPrivate) {
      // Private tabs are deliberately excluded from the reopen stack —
      // reopening one would defeat the entire point of "private" (its
      // URL living on in a stack that survives the tab closing).
      closedStack.push(closed.url);
      if (closedStack.length > 15) closedStack.shift();
    }
    tabs.update((prev) => {
      if (prev.length === 1) { const t = makeTab(); activeId.set(t.id); return [t]; }
      const next = prev.filter((t) => t.id !== id);
      if (current === id) activeId.set(next[next.length - 1].id);
      return next;
    });
    tabUnlisten.get(id)?.forEach((fn) => fn());
    tabUnlisten.delete(id);
    if (liveWebviews.has(id)) {
      liveWebviews.delete(id);
      if (SystemBridge.isTauri()) {
        try { await SystemBridge.invokeCommand('web_view_close', { tabId: id }); } catch {}
      }
    }
  }

  function reopenClosedTab() {
    const url = closedStack.pop();
    if (!url) return;
    const id = addTab();
    openUrl(url, id);
  }

  /** Show `id`'s webview, hide every other live one. Called on tab switch.
   * The newly shown webview is moved to the CURRENT content rect BEFORE it
   * is shown — hidden tabs don't get bounds updates, so without this a tab
   * would flash at wherever the window used to be (or at (0,0)). */
  async function setActiveWebview(id: string) {
    if (!SystemBridge.isTauri()) return;
    for (const otherId of liveWebviews) {
      if (otherId === id) continue;
      try { await SystemBridge.invokeCommand('web_view_set_visible', { tabId: otherId, visible: false }); } catch {}
    }
    if (liveWebviews.has(id)) {
      const b = getBounds?.();
      if (b && b.width > 1 && b.height > 1) {
        try { await SystemBridge.invokeCommand('web_view_set_bounds', { tabId: id, ...b }); } catch {}
      }
      try { await SystemBridge.invokeCommand('web_view_set_visible', { tabId: id, visible: true }); } catch {}
    }
  }

  /** Explicit override used by the z-order fix in BlueWebApp.svelte —
   * forces every live webview hidden regardless of which is "active",
   * for cases where the whole app window shouldn't show any embedded
   * content at all right now (covered by an overlay, not the topmost
   * window, minimized, etc). Passing `null` restores whichever tab is
   * currently active via the normal `setActiveWebview` rules. */
  async function setAllHidden() {
    if (!SystemBridge.isTauri()) return;
    for (const id of liveWebviews) {
      try { await SystemBridge.invokeCommand('web_view_set_visible', { tabId: id, visible: false }); } catch {}
    }
  }

  /** Click-through toggle for every live webview (window drag/resize guard). */
  async function setInteractive(interactive: boolean) {
    if (!SystemBridge.isTauri()) return;
    for (const id of liveWebviews) {
      try { await SystemBridge.invokeCommand('web_view_set_interactive', { tabId: id, interactive }); } catch {}
    }
  }

  function hasLiveWebview(id: string): boolean {
    return liveWebviews.has(id);
  }

  async function reloadActive() {
    const id = get(activeId);
    if (liveWebviews.has(id) && SystemBridge.isTauri()) {
      try { await SystemBridge.invokeCommand('web_view_reload', { tabId: id }); } catch {}
    }
  }

  async function setZoom(factor: number) {
    const id = get(activeId);
    const clamped = Math.max(0.25, Math.min(5, factor));
    tabs.update((prev) => prev.map((t) => (t.id === id ? { ...t, zoom: clamped } : t)));
    if (liveWebviews.has(id) && SystemBridge.isTauri()) {
      try { await SystemBridge.invokeCommand('web_view_set_zoom', { tabId: id, factor: clamped }); } catch {}
    }
  }

  function zoomOf(id: string): number {
    return get(tabs).find((t) => t.id === id)?.zoom ?? 1;
  }

  async function find(query: string, backwards = false) {
    const id = get(activeId);
    if (liveWebviews.has(id) && SystemBridge.isTauri()) {
      try { await SystemBridge.invokeCommand('web_view_find', { tabId: id, query, backwards }); } catch {}
    }
  }

  async function clearFind() {
    const id = get(activeId);
    if (liveWebviews.has(id) && SystemBridge.isTauri()) {
      try { await SystemBridge.invokeCommand('web_view_clear_find', { tabId: id }); } catch {}
    }
  }

  function cleanup() {
    for (const fns of tabUnlisten.values()) fns.forEach((fn) => fn());
    tabUnlisten.clear();
  }

  return {
    tabs, activeId, openUrl, addTab, closeTab, reopenClosedTab, setActiveWebview, setAllHidden,
    hasLiveWebview, reloadActive, setZoom, zoomOf, find, clearFind, cleanup, setInteractive,
  };
}
