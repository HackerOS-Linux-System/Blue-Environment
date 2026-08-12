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
  import { onMount, onDestroy } from 'svelte';
  import { Plus, X, Globe, ExternalLink } from 'lucide-svelte';
  import { createTabs } from './tabs';
  import { createHistory } from './history';
  import { SystemBridge } from '../../../utils/systemBridge';
  import AddressBar from './AddressBar.svelte';
  import SidePanel from './SidePanel.svelte';
  import NewTabPage from './NewTabPage.svelte';

  type Panel = 'bookmarks' | 'history' | 'none';

  let panel: Panel = 'none';
  let lastError: string | null = null;
  let contentEl: HTMLDivElement;
  let lastRect: { x: number; y: number; width: number; height: number } | null = null;
  let rafId: number | null = null;
  let unlistenNav: (() => void)[] = [];

  const hist = createHistory();
  const { navIdx, navStack, bookmarks, history: historyStore } = hist;

  function handleNavigate(url: string, _tabId: string) {
    const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();
    hist.addHistory(url, title);
    lastError = null;
  }

  const { tabs, activeId, openUrl, addTab, closeTab, setActiveWebview, hasLiveWebview, reloadActive } = createTabs(handleNavigate);

  $: activeTab = $tabs.find((t) => t.id === $activeId) ?? $tabs[0];
  $: isSecure = activeTab.url.startsWith('https://') || activeTab.isNew;
  $: isBookmarked = hist.isBookmarked(activeTab.url);
  $: canGoBack = $navIdx > 0;

  function measureRect() {
    if (!contentEl) return null;
    const r = contentEl.getBoundingClientRect();
    // When a side panel (bookmarks/history) is open, shrink the tracked
    // width so the embedded webview's real bounds don't extend under
    // it. Necessary because the embedded webview is a native OS surface
    // stacked above this window's own DOM content by default — the
    // SidePanel's `absolute` CSS positioning alone can't make it render
    // on top of a genuinely separate native view the way it would for
    // another plain HTML element. `w-72` = 288px, matching SidePanel.svelte.
    const panelWidth = panel === 'none' ? 0 : 288;
    return { x: Math.round(r.left), y: Math.round(r.top), width: Math.round(r.width - panelWidth), height: Math.round(r.height) };
  }

  function rectsEqual(a: typeof lastRect, b: typeof lastRect): boolean {
    if (!a || !b) return a === b;
    return a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;
  }

  // Continuous bounds sync — see the module doc above for why this has
  // to be a polling loop rather than an event listener. Only sends an
  // IPC call when the measured rect actually differs from last frame,
  // so a static window costs nothing beyond the (cheap)
  // getBoundingClientRect() call itself.
  function syncLoop() {
    const rect = measureRect();
    if (rect && !rectsEqual(rect, lastRect) && SystemBridge.isTauri() && hasLiveWebview($activeId)) {
      lastRect = rect;
      SystemBridge.invokeCommand('web_view_set_bounds', { tabId: $activeId, ...rect }).catch(() => {});
    } else if (rect) {
      lastRect = rect;
    }
    rafId = requestAnimationFrame(syncLoop);
  }

  onMount(() => {
    rafId = requestAnimationFrame(syncLoop);
  });

  onDestroy(() => {
    if (rafId !== null) cancelAnimationFrame(rafId);
    unlistenNav.forEach((fn) => fn());
    // Close every embedded webview this Blue Web window owns — they're
    // native OS surfaces, not DOM nodes, so they wouldn't be cleaned up
    // automatically just because this Svelte component unmounts.
    if (SystemBridge.isTauri()) {
      $tabs.forEach((t) => { if (hasLiveWebview(t.id)) SystemBridge.invokeCommand('web_view_close', { tabId: t.id }).catch(() => {}); });
    }
  });

  // Tab switch: hide the previously-active tab's webview, show the new
  // one. Also true on the very first activeId "change" at mount, which
  // is harmless (nothing to hide yet).
  let prevActiveId = $activeId;
  $: if ($activeId !== prevActiveId) {
    prevActiveId = $activeId;
    setActiveWebview($activeId);
    lastRect = null; // force a fresh bounds push for the newly-active tab next frame
  }

  async function navigate(url: string, tabId?: string) {
    const bounds = measureRect() ?? undefined;
    await openUrl(url, tabId, bounds);
  }
</script>

<div class="flex flex-col h-full bg-slate-900 text-white select-none">
  <div class="flex items-center h-9 bg-slate-950/70 border-b border-white/5 overflow-x-auto shrink-0">
    {#each $tabs as t (t.id)}
      <div on:click={() => activeId.set(t.id)}
        class="group flex items-center gap-1.5 px-3 h-full shrink-0 cursor-pointer border-r border-white/5 transition-colors max-w-[180px] {t.id === $activeId ? 'bg-slate-800 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'}">
        <Globe size={12} class="shrink-0 opacity-60" />
        <span class="text-xs truncate flex-1">{t.title || 'New Tab'}</span>
        <button on:click={(e) => closeTab(t.id, e)} class="opacity-0 group-hover:opacity-100 hover:text-red-400 shrink-0 ml-1"><X size={10} /></button>
      </div>
    {/each}
    <button on:click={addTab} class="p-2 text-slate-500 hover:text-white shrink-0"><Plus size={14} /></button>
  </div>

  <AddressBar
    url={activeTab.url} isNew={activeTab.isNew} {isSecure} isBookmarked={hist.isBookmarked(activeTab.url)}
    canGoBack={$navIdx > 0} canGoForward={$navIdx < $navStack.length - 1}
    panelOpen={panel}
    on:back={() => { const u = hist.goBackNav(); if (u) navigate(u); }}
    on:forward={() => { const u = hist.goForwardNav(); if (u) navigate(u); }}
    on:refresh={() => !activeTab.isNew && reloadActive()}
    on:home={() => navigate('https://duckduckgo.com')}
    on:navigate={(e) => navigate(e.detail)}
    on:toggleBookmark={() => hist.toggleBookmark(activeTab.url, activeTab.title)}
    on:toggleBookmarks={() => (panel = panel === 'bookmarks' ? 'none' : 'bookmarks')}
    on:toggleHistory={() => (panel = panel === 'history' ? 'none' : 'history')}
  />

  <div class="flex-1 overflow-hidden relative">
    {#if activeTab.isNew}
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
    {/if}

    <SidePanel {panel} bookmarks={$bookmarks} history={$historyStore}
      on:close={() => (panel = 'none')}
      on:navigate={(e) => navigate(e.detail)}
      on:clearHistory={hist.clearHistory}
    />
  </div>
</div>
