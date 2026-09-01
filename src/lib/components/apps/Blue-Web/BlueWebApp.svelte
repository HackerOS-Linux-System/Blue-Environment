<script lang="ts">
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
  // ── The z-order bug, and how `webviewShouldBeVisible` below fixes it ──
  // Because the embedded webview is a separate native surface, it
  // doesn't just render on top of *this* window's own DOM — it renders
  // on top of every other window and every shell overlay too (start
  // menu, control center, notifications, dialogs, the alt-tab switcher,
  // another app window dragged on top of this one, a PiP window
  // floating above it). None of that is visible to a native child
  // surface, which only knows "I'm a rectangle positioned at (x,y)
  // within the OS window" — it has no concept of what else the shell
  // has drawn since. `topmostVisibleWindowId` and `blockingOverlayOpen`
  // (see those stores' doc comments) are exactly the signals the DOM
  // side already computes correctly for its own stacking; this
  // component just also uses them to decide whether the *native*
  // webview should be shown at all, hiding it via `web_view_set_visible`
  // whenever the answer is no. This is necessarily a blunt fix — "hide
  // entirely" rather than "clip to the visible region" — because
  // there's no API to partially occlude a native child webview by an
  // arbitrary DOM shape; hiding it whenever this window isn't strictly
  // topmost is the closest correct approximation available.
  import { onMount, onDestroy } from 'svelte';
  import { Plus, X, Globe, ExternalLink, Search, ZoomIn, ZoomOut, ArrowUp, ArrowDown, EyeOff, Download } from 'lucide-svelte';
  import { createTabs } from './tabs';
  import { createHistory } from './history';
  import { createWebSettings } from './webSettings';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { topmostVisibleWindowId } from '../../../stores/windowManager';
  import { windows } from '../../../stores/windowManager';
  import { blockingOverlayOpen } from '../../../stores/overlayState';
  import { openApp } from '../../../stores/windowManager';
  import { AppId } from '../../../types';
  import { ZOOM_LEVELS } from './types';
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
    hasLiveWebview, reloadActive, setZoom, zoomOf, find, clearFind, cleanup,
  } = createTabs(handleNavigate, handleFavicon, () => $settings.searchEngine, () => $settings.defaultZoom);

  $: activeTab = $tabs.find((t) => t.id === $activeId) ?? $tabs[0];
  $: isSecure = activeTab.url.startsWith('https://') || activeTab.isNew;
  $: isBookmarked = hist.isBookmarked(activeTab.url);
  $: canGoBack = $navIdx > 0;
  $: zoomPct = Math.round((activeTab.zoom ?? 1) * 100);

  // ── The z-order fix itself ──────────────────────────────────────────
  // Recomputed whenever any of its inputs change: this window's own
  // topmost-ness, whether a shell overlay is open, or which tab is
  // active (a tab switch needs to re-run `setActiveWebview`'s
  // show/hide logic anyway). `webviewShouldBeVisible` is the single
  // source of truth `setActiveWebview`/`setAllHidden` below are driven
  // from — nothing else in this file calls `web_view_set_visible`
  // directly.
  $: webviewShouldBeVisible = $topmostVisibleWindowId === windowId && !$blockingOverlayOpen && !settingsOpen;

  let prevVisible = webviewShouldBeVisible;
  let prevActiveId = $activeId;
  $: {
    const activeChanged = $activeId !== prevActiveId;
    const visibilityChanged = webviewShouldBeVisible !== prevVisible;
    if (activeChanged || visibilityChanged) {
      prevActiveId = $activeId;
      prevVisible = webviewShouldBeVisible;
      if (webviewShouldBeVisible) {
        setActiveWebview($activeId);
      } else {
        // Not the topmost window right now (covered by another window,
        // a PiP window, or a shell overlay) — hide every live webview
        // this component owns rather than just the previously-active
        // one, since a tab switch could otherwise race with the
        // visibility change and leave the wrong tab's webview showing.
        setAllHidden();
      }
      if (activeChanged) lastRect = null; // force a fresh bounds push for the newly-active tab next frame
    }
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
    // send bounds that extend past the actual OS window's own viewport.
    // This is the one clamp that can't be wrong by definition — `
    // window.innerWidth`/`innerHeight` *is* the real, current size of
    // the native window this whole app runs inside.
    width = Math.max(0, Math.min(width, window.innerWidth - x));
    height = Math.max(0, Math.min(height, window.innerHeight - y));

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
  // webview is meant to be hidden (`webviewShouldBeVisible` false) —
  // no point pushing bounds for a surface nothing should be showing.
  function syncLoop() {
    if (webviewShouldBeVisible) {
      const rect = measureRect();
      if (rect && !rectsEqual(rect, lastRect) && SystemBridge.isTauri() && hasLiveWebview($activeId)) {
        lastRect = rect;
        SystemBridge.invokeCommand('web_view_set_bounds', { tabId: $activeId, ...rect }).catch(() => {});
      } else if (rect) {
        lastRect = rect;
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
      <div on:click={() => activeId.set(t.id)} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => activeId.set(t.id))(); } }}
        class="group flex items-center gap-1.5 px-3 h-full shrink-0 cursor-pointer border-r border-white/5 transition-colors max-w-[180px] {t.id === $activeId ? 'bg-slate-800 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'} {t.isPrivate ? 'bg-indigo-950/40' : ''}">
        {#if t.isPrivate}<EyeOff size={11} class="shrink-0 text-indigo-400" />
        {:else if t.favicon}<img src={t.favicon} alt="" class="w-3 h-3 shrink-0 rounded-sm" on:error={() => (t.favicon = undefined)} />
        {:else}<Globe size={12} class="shrink-0 opacity-60" />{/if}
        <span class="text-xs truncate flex-1">{t.title || (t.isPrivate ? 'New private tab' : 'New Tab')}</span>
        <button on:click={(e) => closeTab(t.id, e)} class="opacity-0 group-hover:opacity-100 hover:text-red-400 shrink-0 ml-1"><X size={10} /></button>
      </div>
    {/each}
    <button on:click={() => addTab()} title="New tab (Ctrl+T)" class="p-2 text-slate-500 hover:text-white shrink-0"><Plus size={14} /></button>
    <button on:click={() => addTab(true)} title="New private tab (Ctrl+Shift+N)" class="p-2 text-slate-500 hover:text-indigo-300 shrink-0"><EyeOff size={12} /></button>
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
        placeholder="Find in page…" class="flex-1 bg-transparent text-sm text-white placeholder:text-slate-500 focus:outline-none" />
      <button on:click={() => find(findQuery, true)} title="Previous match" class="p-1 rounded hover:bg-white/10"><ArrowUp size={13} /></button>
      <button on:click={() => find(findQuery, false)} title="Next match" class="p-1 rounded hover:bg-white/10"><ArrowDown size={13} /></button>
      <button on:click={closeFindBar} class="p-1 rounded hover:bg-white/10"><X size={13} /></button>
    </div>
  {/if}

  <div class="flex-1 overflow-hidden relative">
    {#if settingsOpen}
      <WebSettingsPanel settings={$settings} onUpdate={webSettings.update}
        onAddBlocked={webSettings.addBlockedDomain} onRemoveBlocked={webSettings.removeBlockedDomain}
        on:close={() => (settingsOpen = false)} />
    {:else if activeTab.isNew}
      <NewTabPage error={lastError} on:navigate={(e) => navigate(e.detail)} />
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
              <p class="text-slate-500 text-sm max-w-sm mx-auto">Embedded browsing needs the Tauri desktop app — this preview environment opens links in a new browser tab instead.</p>
            </div>
          </div>
        {/if}
      </div>
      {#if !webviewShouldBeVisible}
        <!-- Nothing renders here in the common case — this is purely a
             debug/legibility affordance for the (rare) moment this
             window is genuinely open but not topmost, e.g. right after
             alt-tabbing away. Without it, a click landing where the
             page used to be would silently do nothing, which is more
             confusing than a clear "backgrounded" placeholder. -->
        <div class="absolute inset-0 flex items-center justify-center bg-slate-900/70 pointer-events-none">
          <span class="text-xs text-slate-500">Backgrounded</span>
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
      <button on:click={() => adjustZoom(-1)} title="Zoom out (Ctrl+-)" class="p-0.5 rounded hover:bg-white/10 hover:text-white"><ZoomOut size={12} /></button>
      <button on:click={() => setZoom(1)} title="Reset zoom (Ctrl+0)" class="text-[10px] w-9 text-center hover:text-white">{zoomPct}%</button>
      <button on:click={() => adjustZoom(1)} title="Zoom in (Ctrl+=)" class="p-0.5 rounded hover:bg-white/10 hover:text-white"><ZoomIn size={12} /></button>
      {#if downloads.some((d) => d.state === 'downloading')}
        <span class="flex items-center gap-1 ml-2 text-[10px] text-blue-300"><Download size={11} class="animate-bounce" /> Downloading…</span>
      {/if}
    </div>
  {/if}
</div>
