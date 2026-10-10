<script lang="ts">
  import { t as tr } from '../../../stores/language';
  // ── Real embedded browsing ────────────────────────────────────────────
  // Previously every URL opened in a brand-new, separate OS window and
  // this component just showed a "switch to it via the taskbar" message
  // — not actually browsing inside the app at all. Now each tab gets a
  // real embedded child webview (Tauri's `Window::add_child`, wired up
  // in src-tauri/src/BlueWebApp/mod.rs — see that file's module doc for
  // the full explanation, including why title/favicon still come from
  // the URL's hostname rather than a JS bridge: security, not an
  // oversight).
  //
  // The embedded webview is a native OS-level surface, not a DOM
  // element — it can't be styled or clipped by CSS, and it always
  // renders on top of this window's own web content. So `contentEl`
  // below isn't where the page visually appears; it's a plain
  // placeholder div whose on-screen rectangle (measured every frame via
  // `getBoundingClientRect()`) tells the backend where to position and
  // size the real embedded webview so it lines up exactly with this
  // app's content area — including while the window is being dragged
  // or resized, since there's no DOM event for "an ancestor's CSS
  // transform changed", only continuous measurement.
  //
  // ── The z-order problem, and how this file deals with it ───────────────
  // Because the embedded webview is a separate native surface, it
  // doesn't just render on top of *this* window's own DOM — it renders
  // on top of every other window and every shell overlay too (start
  // menu, control center, dialogs, the alt-tab switcher, another app
  // window dragged on top of this one, a PiP window floating above it…).
  // A native child surface only knows "I'm a rectangle at (x,y)"; it
  // cannot be partially occluded by DOM, so the only correct way to let
  // something cover it is to HIDE it (`web_view_set_visible`).
  //
  // Hiding used to leave an empty "in the background" placeholder where
  // the page had been — whenever another window was on top, or the person
  // so much as opened a menu. Now:
  //   1. the page is hidden ONLY when something really overlaps its
  //      rectangle (`occlusion.ts`: windows stacked above it, plus every
  //      shell overlay that carries `data-shell-occluder`) — a window
  //      elsewhere on screen, or a widget in another corner, changes nothing,
  //      and the page stays live (video keeps playing, links stay clickable);
  //   2. right before it is hidden, a snapshot of the page is taken
  //      (`web_view_snapshot`) and painted in the DOM placeholder, so the
  //      page is still SEEN — just underneath whatever covers it — instead
  //      of vanishing. When a platform can't take snapshots, a neutral card
  //      with the page's title takes its place.
  // `reconcileVisibility()` below is the only place that shows/hides.
  import { onMount, onDestroy, tick } from 'svelte';
  import { Plus, X, Globe, ExternalLink, Search, ZoomIn, ZoomOut, ArrowUp, ArrowDown, EyeOff, Download } from 'lucide-svelte';
  import { createTabs } from './tabs';
  import { createHistory } from './history';
  import { createWebSettings } from './webSettings';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { windows, activeWindowId, focusWindow } from '../../../stores/windowManager';
  import { get } from 'svelte/store';
  import { isCovered, windowsAbove, type Rect } from './occlusion';
  import { blockingOverlayOpen, windowInteracting } from '../../../stores/overlayState';
  import { openApp } from '../../../stores/windowManager';
  import { AppId } from '../../../types';
  import { ZOOM_LEVELS, SEARCH_ENGINES } from './types';
  import type { DownloadItem } from './types';
  import AddressBar from './AddressBar.svelte';
  import SidePanel from './SidePanel.svelte';
  import NewTabPage from './NewTabPage.svelte';
  import WebSettingsPanel from './WebSettingsPanel.svelte';

  export let windowId: string;
  // Launch-arg entry point — other apps opening a link (see
  // AboutApp.svelte, and `utils/openInBlueWeb.ts`) call `openApp(AppId.
  // BLUE_WEB, false, undefined, { launchUrl: url })` rather than
  // shelling out to `xdg-open`, so the link opens inside this shell's
  // own browser instead of whatever the OS's default browser happens
  // to be. `openApp` always creates a fresh window (this codebase has
  // no single-instance-app dedup), so there's no "already open, add a
  // tab to the existing window" case to handle here — every launch is
  // a new window, navigated straight to `launchUrl`.
  export let launchUrl: string | undefined = undefined;

  type Panel = 'bookmarks' | 'history' | 'downloads' | 'none';

  let panel: Panel = 'none';
  let lastError: string | null = null;
  $: engineName = (SEARCH_ENGINES.find((e) => e.id === $settings.searchEngine) ?? SEARCH_ENGINES[0]).name;
  let contentEl: HTMLDivElement;
  let lastRect: { x: number; y: number; width: number; height: number } | null = null;
  let rafId: number | null = null;
  let findBarOpen = false;
  let findQuery = '';
  let findInputEl: HTMLInputElement;
  let downloads: DownloadItem[] = [];
  let settingsOpen = false;

  const webSettings = createWebSettings();
  const { settings } = webSettings;

  const hist = createHistory();
  const { navIdx, navStack, bookmarks, history: historyStore } = hist;

  function handleNavigate(url: string, tabId: string) {
    const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();
    const tab = $tabs.find((t) => t.id === tabId);
    hist.addHistory(url, title, { record: !tab?.isPrivate });
    lastError = null;
    if (findBarOpen) closeFindBar();
  }

  function handleFavicon(tabId: string, favicon: string) {
    tabs.update((prev) => prev.map((t) => (t.id === tabId ? { ...t, favicon } : t)));
  }

  const {
    tabs, activeId, openUrl, addTab, closeTab, reopenClosedTab, setActiveWebview, setAllHidden,
    hasLiveWebview, reloadActive, setZoom, zoomOf, find, clearFind, cleanup, setInteractive, snapshot,
  } = createTabs(
    handleNavigate, handleFavicon, () => $settings.searchEngine, () => $settings.defaultZoom,
    // getBounds / getShouldBeVisible — let tabs.ts measure the content area itself
    // AFTER Svelte has rendered it (see `openUrl`'s doc), and re-apply the
    // visibility rules the instant a webview is created.
    () => measureRect(),
    () => webviewShouldBeVisible,
    // A fresh webview must get its real bounds on the very next frame.
    () => { lastRect = null; },
    // Creation failed for good: go back to the new-tab page and say why,
    // instead of leaving a blank content area.
    (message) => {
      lastError = message;
      tabs.update((prev) => prev.map((t) => (t.id === $activeId ? { ...t, isNew: true } : t)));
    },
    // A click inside the page: it never reaches this window's own mousedown handler.
    () => { if (get(activeWindowId) !== windowId) focusWindow(windowId); },
  );

  // While an app window is being dragged/resized, embedded (native) webviews
  // become click-through so the shell keeps receiving mousemove/mouseup — see
  // `windowInteracting` in overlayState.ts.
  $: setInteractive(!$windowInteracting);

  $: activeTab = $tabs.find((t) => t.id === $activeId) ?? $tabs[0];
  $: isSecure = activeTab.url.startsWith('https://') || activeTab.isNew;
  $: isBookmarked = hist.isBookmarked(activeTab.url);
  $: canGoBack = $navIdx > 0;
  $: zoomPct = Math.round((activeTab.zoom ?? 1) * 100);

  // ── Visibility ──────────────────────────────────────────────────────
  /** Something (a window above this one, a shell overlay) covers the page right now. */
  let occluded = false;
  /** Whether the native page may be shown. The single input to `reconcileVisibility`. */
  $: wantVisible = !occluded && !settingsOpen;
  // Kept for tabs.ts's `getShouldBeVisible` (a freshly created webview asks right away).
  $: webviewShouldBeVisible = wantVisible;

  /** What the native layer currently shows: `nativeHidden` ⇒ the DOM placeholder is on view. */
  let nativeHidden = false;
  let shownTabId: string | null = null;
  /** Last snapshot per tab (JPEG data URL) — what the placeholder paints while the page is covered. */
  let snapshots: Record<string, string> = {};
  let snapshotFailures = 0;
  const SNAPSHOT_GIVE_UP_AFTER = 3;
  const SNAPSHOT_WAIT_MS = 450;

  let reconciling = false;
  let reconcileAgain = false;

  const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));

  /** Snapshot the shown page (bounded wait), remembering whatever arrives, even late. */
  async function captureSnapshot(tabId: string) {
    if (snapshotFailures >= SNAPSHOT_GIVE_UP_AFTER || !hasLiveWebview(tabId)) return;
    const pending = snapshot(tabId).then((shot) => {
      if (shot) { snapshots = { ...snapshots, [tabId]: shot }; snapshotFailures = 0; }
      else snapshotFailures += 1;
    });
    await Promise.race([pending, new Promise<void>((r) => setTimeout(r, SNAPSHOT_WAIT_MS))]);
  }

  /**
   * Brings the native layer in line with `wantVisible` / the active tab. Serialised: a change
   * that arrives mid-transition is applied right after it, never concurrently with it.
   */
  async function reconcileVisibility() {
    if (reconciling) { reconcileAgain = true; return; }
    reconciling = true;
    try {
      do {
        reconcileAgain = false;
        const want = wantVisible;
        const id = $activeId;
        if (want) {
          if (nativeHidden || shownTabId !== id) {
            await setActiveWebview(id);
            shownTabId = id;
            lastRect = null; // fresh bounds push for the (re)shown page on the next frame
            // Only now drop the placeholder: the live page is up, so there is no blank frame.
            nativeHidden = false;
          }
        } else if (!nativeHidden) {
          // Capture while the page is still on screen, paint it underneath, THEN hide it.
          if (shownTabId) await captureSnapshot(shownTabId);
          nativeHidden = true;
          await tick();
          await nextFrame();
          await setAllHidden();
        } else if (shownTabId !== id) {
          shownTabId = id; // tab switched while covered: the placeholder follows the active tab
        }
      } while (reconcileAgain);
    } finally {
      reconciling = false;
    }
  }

  $: { wantVisible; $activeId; reconcileVisibility(); }

  /** Rects of everything drawn above this window's page right now. `null` = unknown (be safe). */
  function coverRects(): Rect[] | null {
    const rects: Rect[] = [];
    const me = $windows.find((w) => w.id === windowId);
    if (me) {
      for (const above of windowsAbove(me, $windows)) {
        const el = document.querySelector(`[data-window-root="${above.id}"]`);
        if (el) rects.push(domRect(el));
      }
    }
    const overlays = document.querySelectorAll('[data-shell-occluder]');
    for (const el of overlays) rects.push(domRect(el));
    // An overlay is open but nothing identifies where it is: assume it covers everything.
    if (get(blockingOverlayOpen) && overlays.length === 0) return null;
    return rects;
  }

  function domRect(el: Element): Rect {
    const r = el.getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  }

  /** Per frame: is the page's rectangle covered by a window above it or by a shell overlay? */
  function computeOccluded(): boolean {
    const page = measureRect();
    if (!page) return false; // nothing laid out, nothing to cover
    const covers = coverRects();
    return covers === null ? true : isCovered(page, covers);
  }

  function measureRect() {
    if (!contentEl) return null;
    const r = contentEl.getBoundingClientRect();
    // When a side panel (bookmarks/history/downloads) is open, shrink
    // the tracked width so the embedded webview's real bounds don't
    // extend under it. Necessary because the embedded webview is a
    // native OS surface stacked above this window's own DOM content by
    // default — the SidePanel's `absolute` CSS positioning alone can't
    // make it render on top of a genuinely separate native view the
    // way it would for another plain HTML element. `w-72` = 288px,
    // matching SidePanel.svelte.
    const panelWidth = panel === 'none' ? 0 : 288;
    const findBarHeight = findBarOpen ? 44 : 0;
    let x = Math.round(r.left);
    let y = Math.round(r.top) + findBarHeight;
    let width = Math.round(r.width - panelWidth);
    let height = Math.round(r.height - findBarHeight);

    // Belt-and-suspenders clamp against this window's own tracked
    // bounds (from `windowManager`'s store — the same numbers Window.
    // svelte itself renders from). `getBoundingClientRect()` should
    // already respect the window's actual on-screen box, since
    // `contentEl` sits inside Window.svelte's flex-constrained content
    // area — but the embedded webview is a *native* surface positioned
    // by raw OS coordinates, not something CSS can clip after the
    // fact. If `contentEl`'s measurement is ever wrong for any reason
    // (a layout race during window creation/resize, a stale measurement
    // sent before Svelte finished a reactive update), an unclamped
    // bounds push would make the real webpage visibly spill outside the
    // browser window's frame — exactly the failure mode a native child
    // surface can't self-correct from the way a normal DOM element
    // would. Clamping here means that failure mode is now structurally
    // impossible regardless of what caused a bad measurement.
    const win = $windows.find((w) => w.id === windowId);
    if (win && !win.isMaximized) {
      const left = Math.max(x, win.x);
      const top = Math.max(y, win.y);
      const right = Math.min(x + width, win.x + win.width);
      const bottom = Math.min(y + height, win.y + win.height);
      x = left;
      y = top;
      width = Math.max(0, right - left);
      height = Math.max(0, bottom - top);
    }

    // Final, unconditional safety net regardless of maximized state or
    // whether the window-store lookup above even found a match: never
    // send bounds that extend past the actual OS window's own viewport,
    // and never send a negative origin (a window dragged partly off-screen
    // would otherwise push the native webview to negative coordinates).
    // `window.innerWidth`/`innerHeight` *is* the real, current size of
    // the native window this whole app runs inside.
    if (x < 0) { width += x; x = 0; }
    if (y < 0) { height += y; y = 0; }
    width = Math.max(0, Math.min(width, window.innerWidth - x));
    height = Math.max(0, Math.min(height, window.innerHeight - y));

    // Nothing sensible to show (not laid out yet / fully off-screen).
    if (width < 2 || height < 2) return null;

    return { x, y, width, height };
  }

  function rectsEqual(a: typeof lastRect, b: typeof lastRect): boolean {
    if (!a || !b) return a === b;
    return a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;
  }

  // Continuous bounds sync — see the module doc above for why this has
  // to be a polling loop rather than an event listener. Only sends an
  // IPC call when the measured rect actually differs from last frame,
  // so a static window costs nothing beyond the (cheap)
  // getBoundingClientRect() call itself. Skips entirely while the
  // webview is meant to be hidden (`wantVisible` false / placeholder shown) —
  // no point pushing bounds for a surface nothing should be showing.
  // `lastRect` is only updated once the backend ACKed the push. The old code
  // stored the rect even when it had NOT been sent (webview not live yet),
  // so once the webview finally existed, "rect === lastRect" suppressed the
  // very first push and the page stayed at its creation placeholder
  // forever. At most one IPC call is in flight at a time (also keeps a fast
  // window drag from flooding the main thread).
  let pushingBounds = false;
  function syncLoop() {
    if (!activeTab.isNew && !settingsOpen) {
      const covered = computeOccluded();
      if (covered !== occluded) occluded = covered;
    } else if (occluded) {
      occluded = false;
    }
    if (wantVisible && !nativeHidden && !pushingBounds) {
      const rect = measureRect();
      if (rect && !rectsEqual(rect, lastRect) && SystemBridge.isTauri() && hasLiveWebview($activeId)) {
        pushingBounds = true;
        const tabId = $activeId;
        SystemBridge.invokeCommand('web_view_set_bounds', { tabId, ...rect })
          .then(() => { if (tabId === $activeId) lastRect = rect; })
          .catch(() => { /* retried on the next frame */ })
          .finally(() => { pushingBounds = false; });
      }
    }
    rafId = requestAnimationFrame(syncLoop);
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;
    if (e.key === 't' && !e.shiftKey) { e.preventDefault(); addTab(); }
    else if (e.key === 't' && e.shiftKey) { e.preventDefault(); reopenClosedTab(); }
    else if (e.key === 'n' && e.shiftKey) { e.preventDefault(); addTab(true); }
    else if (e.key === 'w') { e.preventDefault(); closeTab($activeId); }
    else if (e.key === 'r') { e.preventDefault(); if (!activeTab.isNew) reloadActive(); }
    else if (e.key === 'f') { e.preventDefault(); openFindBar(); }
    else if (e.key === '=' || e.key === '+') { e.preventDefault(); adjustZoom(1); }
    else if (e.key === '-') { e.preventDefault(); adjustZoom(-1); }
    else if (e.key === '0') { e.preventDefault(); setZoom(1); }
    else if (e.key === 'Tab' && e.shiftKey) { e.preventDefault(); cycleTab(-1); }
    else if (e.key === 'Tab') { e.preventDefault(); cycleTab(1); }
  }

  function cycleTab(dir: 1 | -1) {
    const idx = $tabs.findIndex((t) => t.id === $activeId);
    const next = (idx + dir + $tabs.length) % $tabs.length;
    activeId.set($tabs[next].id);
  }

  function adjustZoom(dir: 1 | -1) {
    const current = activeTab.zoom ?? 1;
    const levels = ZOOM_LEVELS as readonly number[];
    const idx = levels.reduce((best, v, i) => (Math.abs(v - current) < Math.abs(levels[best] - current) ? i : best), 0);
    const next = levels[Math.max(0, Math.min(levels.length - 1, idx + dir))];
    setZoom(next);
  }

  function openFindBar() {
    if (activeTab.isNew) return;
    findBarOpen = true;
    lastRect = null; // content area shrinks by the find bar's height — force a bounds resync
    setTimeout(() => findInputEl?.focus(), 30);
  }

  function closeFindBar() {
    findBarOpen = false;
    findQuery = '';
    clearFind();
    lastRect = null;
  }

  function handleFindKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.preventDefault(); closeFindBar(); }
    else if (e.key === 'Enter') { e.preventDefault(); find(findQuery, e.shiftKey); }
  }

  async function refreshDownloads() {
    if (!SystemBridge.isTauri()) return;
    try {
      const list = await SystemBridge.invokeCommand<any[]>('web_downloads_list');
      downloads = (list ?? []).map((d) => ({ id: d.id, tabId: d.tab_id, url: d.url, filename: d.filename, path: d.path, state: d.state }));
    } catch { /* no downloads yet, or not in Tauri */ }
  }

  let unlistenDownloadEvents: (() => void)[] = [];
  async function attachDownloadListeners() {
    if (!SystemBridge.isTauri()) return;
    try {
      const mod = await import('@tauri-apps/api/event');
      const un1 = await mod.listen('web-download-started', () => refreshDownloads());
      const un2 = await mod.listen('web-download-finished', () => refreshDownloads());
      unlistenDownloadEvents = [un1, un2];
    } catch { /* dev preview environment */ }
  }

  onMount(() => {
    rafId = requestAnimationFrame(syncLoop);
    attachDownloadListeners();
    refreshDownloads();
    webSettings.syncBlocklistToBackend();
    if (launchUrl) {
      // Navigate the initial (still-blank) tab rather than opening a
      // second one — bounds aren't measurable yet on the very first
      // frame, so wait one tick for `contentEl` to actually mount.
      requestAnimationFrame(() => navigate(launchUrl!, $activeId));
    }
  });

  onDestroy(() => {
    if (rafId !== null) cancelAnimationFrame(rafId);
    cleanup();
    unlistenDownloadEvents.forEach((fn) => fn());
    // Close every embedded webview this Blue Web window owns — they're
    // native OS surfaces, not DOM nodes, so they wouldn't be cleaned up
    // automatically just because this Svelte component unmounts.
    if (SystemBridge.isTauri()) {
      $tabs.forEach((t) => { if (hasLiveWebview(t.id)) SystemBridge.invokeCommand('web_view_close', { tabId: t.id }).catch(() => {}); });
    }
  });

  async function navigate(url: string, tabId?: string) {
    // `openUrl` re-measures after the content area has rendered; this rect is
    // only a fallback for the case where the area is already on screen.
    const bounds = measureRect() ?? undefined;
    await openUrl(url, tabId, bounds);
  }

  /**
   * "Save to Blue Tasks" — opens (or focuses) Blue Tasks with launch
   * args pre-filling a new task's title/sourceUrl from the current
   * page. Uses the ordinary `openApp`/`launchArgs` mechanism every app
   * gets, not a private Blue-Web-to-Blue-Tasks channel — see
   * BlueTasksApp/mod.rs's module doc on why this is real cross-app
   * integration rather than a special case.
   */
  function saveToTasks() {
    if (activeTab.isNew) return;
    openApp(AppId.BLUE_TASKS, false, undefined, {
      prefillTitle: activeTab.title || activeTab.url,
      prefillUrl: activeTab.url,
    });
  }
</script>

<svelte:window on:keydown={handleWindowKeydown} />

<div class="flex flex-col h-full bg-slate-900 text-white select-none">
  <div class="flex items-center h-9 bg-slate-950/70 border-b border-white/5 overflow-x-auto shrink-0">
    {#each $tabs as t (t.id)}
      <div on:click={() => activeId.set(t.id)} role="button" tabindex="0" on:keydown={(e) => { if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); (() => activeId.set(t.id))(); } }}
        class="group flex items-center gap-1.5 px-3 h-full shrink-0 cursor-pointer border-r border-white/5 transition-colors max-w-[180px] {t.id === $activeId ? 'bg-slate-800 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'} {t.isPrivate ? 'bg-indigo-950/40' : ''}">
        {#if t.isPrivate}<EyeOff size={11} class="shrink-0 text-indigo-400" />
        {:else if t.favicon}<img src={t.favicon} alt="" class="w-3 h-3 shrink-0 rounded-sm" on:error={() => (t.favicon = undefined)} />
        {:else}<Globe size={12} class="shrink-0 opacity-60" />{/if}
        <span class="text-xs truncate flex-1">{t.title || (t.isPrivate ? $tr('blueweb.new_private_tab') : $tr('blueweb.new_tab'))}</span>
        <button on:click={(e) => closeTab(t.id, e)} class="opacity-0 group-hover:opacity-100 hover:text-red-400 shrink-0 ml-1"><X size={10} /></button>
      </div>
    {/each}
    <button on:click={() => addTab()} title="{$tr('blueweb.new_tab')} (Ctrl+T)" class="p-2 text-slate-500 hover:text-white shrink-0"><Plus size={14} /></button>
    <button on:click={() => addTab(true)} title="{$tr('blueweb.new_private_tab')} (Ctrl+Shift+N)" class="p-2 text-slate-500 hover:text-indigo-300 shrink-0"><EyeOff size={12} /></button>
  </div>

  <AddressBar
    url={activeTab.url} isNew={activeTab.isNew} {isSecure} isBookmarked={hist.isBookmarked(activeTab.url)}
    canGoBack={$navIdx > 0} canGoForward={$navIdx < $navStack.length - 1}
    {panel} downloadCount={downloads.filter((d) => d.state === 'downloading').length}
    on:back={() => { const u = hist.goBackNav(); if (u) navigate(u); }}
    on:forward={() => { const u = hist.goForwardNav(); if (u) navigate(u); }}
    on:refresh={() => !activeTab.isNew && reloadActive()}
    on:home={() => navigate($settings.homepage)}
    on:navigate={(e) => navigate(e.detail)}
    on:toggleBookmark={() => hist.toggleBookmark(activeTab.url, activeTab.title, activeTab.favicon)}
    on:toggleBookmarks={() => (panel = panel === 'bookmarks' ? 'none' : 'bookmarks')}
    on:toggleHistory={() => (panel = panel === 'history' ? 'none' : 'history')}
    on:toggleDownloads={() => { panel = panel === 'downloads' ? 'none' : 'downloads'; if (panel === 'downloads') refreshDownloads(); }}
    on:find={openFindBar}
    on:saveToTasks={saveToTasks}
    on:openSettings={() => (settingsOpen = true)}
  />

  {#if findBarOpen}
    <div class="flex items-center gap-2 px-3 h-11 bg-slate-800 border-b border-white/10 shrink-0">
      <Search size={13} class="text-slate-400 shrink-0" />
      <input bind:this={findInputEl} bind:value={findQuery} on:keydown={handleFindKeydown}
        on:input={() => find(findQuery)}
        placeholder={$tr('blueweb.find_placeholder')} class="flex-1 bg-transparent text-sm text-white placeholder:text-slate-500 focus:outline-none" />
      <button on:click={() => find(findQuery, true)} title={$tr('blueweb.find_prev')} class="p-1 rounded hover:bg-white/10"><ArrowUp size={13} /></button>
      <button on:click={() => find(findQuery, false)} title={$tr('blueweb.find_next')} class="p-1 rounded hover:bg-white/10"><ArrowDown size={13} /></button>
      <button on:click={closeFindBar} class="p-1 rounded hover:bg-white/10"><X size={13} /></button>
    </div>
  {/if}

  <div class="flex-1 overflow-hidden relative">
    {#if settingsOpen}
      <WebSettingsPanel settings={$settings} onUpdate={webSettings.update}
        onAddBlocked={webSettings.addBlockedDomain} onRemoveBlocked={webSettings.removeBlockedDomain}
        on:close={() => (settingsOpen = false)} />
    {:else if activeTab.isNew}
      <NewTabPage error={lastError} engineName={engineName} on:navigate={(e) => navigate(e.detail)} />
    {:else}
      <!-- The real page renders in a native embedded webview positioned
           exactly over this div (see the module doc) — this div itself
           stays empty and transparent. The fallback content below only
           ever shows in the (non-Tauri) web dev-preview environment,
           where there's no Tauri backend to create a real embedded
           webview at all. -->
      <div bind:this={contentEl} class="w-full h-full">
        {#if !SystemBridge.isTauri()}
          <div class="flex-1 flex flex-col items-center justify-center gap-4 text-center px-8 h-full">
            <div class="w-14 h-14 rounded-2xl bg-gradient-to-br from-blue-600 to-indigo-700 flex items-center justify-center shadow-lg shadow-blue-500/20 mx-auto">
              <ExternalLink size={26} class="text-white" />
            </div>
            <div>
              <p class="text-white font-semibold mb-1">{activeTab.title}</p>
              <p class="text-slate-400 text-xs mb-4 font-mono break-all max-w-sm">{activeTab.url}</p>
              <p class="text-slate-500 text-sm max-w-sm mx-auto">{$tr('blueweb.no_embedded')}</p>
            </div>
          </div>
        {/if}
      </div>
      {#if nativeHidden}
        <!-- The native page is hidden because something covers it (see the module doc). The page
             stays visible anyway: this is the snapshot taken an instant before it was hidden,
             painted in the DOM — i.e. underneath whatever covers it. It lines up with the
             webview's own bounds (side panel / find bar are excluded exactly like in
             `measureRect`) and ignores the pointer, so a click falls through to the window
             (which focuses it). With no snapshot available a neutral card stands in. -->
        <div class="absolute overflow-hidden bg-slate-900 pointer-events-none"
          style="left:0; bottom:0; top:{findBarOpen ? 44 : 0}px; right:{panel === 'none' ? 0 : 288}px;">
          {#if snapshots[$activeId]}
            <img src={snapshots[$activeId]} alt="" draggable="false" class="block w-full h-full object-cover object-left-top select-none" />
          {:else}
            <div class="flex flex-col items-center justify-center gap-3 h-full px-8 text-center">
              {#if activeTab.favicon}
                <img src={activeTab.favicon} alt="" class="w-10 h-10 rounded-lg" />
              {:else}
                <div class="w-12 h-12 rounded-2xl bg-slate-800 border border-white/5 flex items-center justify-center"><Globe size={22} class="text-slate-500" /></div>
              {/if}
              <p class="text-sm text-slate-300 font-medium max-w-md truncate">{activeTab.title}</p>
              <p class="text-xs text-slate-600 font-mono max-w-md truncate">{activeTab.url}</p>
            </div>
          {/if}
        </div>
      {/if}
    {/if}

    <SidePanel {panel} bookmarks={$bookmarks} history={$historyStore} {downloads}
      on:close={() => (panel = 'none')}
      on:navigate={(e) => navigate(e.detail)}
      on:clearHistory={hist.clearHistory}
      on:removeDownload={async (e) => { await SystemBridge.invokeCommand('web_download_remove', { id: e.detail }).catch(() => {}); refreshDownloads(); }}
      on:revealDownload={(e) => SystemBridge.invokeCommand('web_download_reveal', { id: e.detail }).catch(() => {})}
    />
  </div>

  {#if !activeTab.isNew}
    <div class="flex items-center justify-end gap-1 px-2 h-6 bg-slate-950/70 border-t border-white/5 shrink-0 text-slate-400">
      <button on:click={() => adjustZoom(-1)} title="{$tr('blueweb.zoom_out')} (Ctrl+-)" class="p-0.5 rounded hover:bg-white/10 hover:text-white"><ZoomOut size={12} /></button>
      <button on:click={() => setZoom(1)} title="{$tr('blueweb.zoom_reset')} (Ctrl+0)" class="text-[10px] w-9 text-center hover:text-white">{zoomPct}%</button>
      <button on:click={() => adjustZoom(1)} title="{$tr('blueweb.zoom_in')} (Ctrl+=)" class="p-0.5 rounded hover:bg-white/10 hover:text-white"><ZoomIn size={12} /></button>
      {#if downloads.some((d) => d.state === 'downloading')}
        <span class="flex items-center gap-1 ml-2 text-[10px] text-blue-300"><Download size={11} class="animate-bounce" /> {$tr('blueweb.downloading')}</span>
      {/if}
    </div>
  {/if}
</div>
