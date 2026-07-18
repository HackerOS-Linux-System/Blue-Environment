<script lang="ts">
  import { Plus, X, Globe, ExternalLink } from 'lucide-svelte';
  import { createTabs } from './tabs';
  import { createHistory } from './history';
  import AddressBar from './AddressBar.svelte';
  import SidePanel from './SidePanel.svelte';
  import NewTabPage from './NewTabPage.svelte';

  type Panel = 'bookmarks' | 'history' | 'none';

  let panel: Panel = 'none';
  let lastError: string | null = null;

  const hist = createHistory();
  const { navIdx, navStack, bookmarks, history: historyStore } = hist;

  function handleNavigate(url: string, _tabId: string) {
    const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();
    hist.addHistory(url, title);
    lastError = null;
  }

  const { tabs, activeId, openUrl, addTab, closeTab } = createTabs(handleNavigate);

  $: activeTab = $tabs.find((t) => t.id === $activeId) ?? $tabs[0];
  $: isSecure = activeTab.url.startsWith('https://') || activeTab.isNew;
  $: isBookmarked = hist.isBookmarked(activeTab.url);
  $: canGoBack = $navIdx > 0;
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
    on:back={() => { const u = hist.goBackNav(); if (u) openUrl(u); }}
    on:forward={() => { const u = hist.goForwardNav(); if (u) openUrl(u); }}
    on:refresh={() => !activeTab.isNew && openUrl(activeTab.url)}
    on:home={() => openUrl('https://duckduckgo.com')}
    on:navigate={(e) => openUrl(e.detail)}
    on:toggleBookmark={() => hist.toggleBookmark(activeTab.url, activeTab.title)}
    on:toggleBookmarks={() => (panel = panel === 'bookmarks' ? 'none' : 'bookmarks')}
    on:toggleHistory={() => (panel = panel === 'history' ? 'none' : 'history')}
  />

  <div class="flex-1 overflow-hidden relative">
    {#if activeTab.isNew}
      <NewTabPage error={lastError} on:navigate={(e) => openUrl(e.detail)} />
    {:else}
      <div class="flex-1 flex flex-col items-center justify-center gap-4 text-center px-8 h-full">
        <div class="w-14 h-14 rounded-2xl bg-gradient-to-br from-blue-600 to-indigo-700 flex items-center justify-center shadow-lg shadow-blue-500/20 mx-auto">
          <ExternalLink size={26} class="text-white" />
        </div>
        <div>
          <p class="text-white font-semibold mb-1">{activeTab.title}</p>
          <p class="text-slate-400 text-xs mb-4 font-mono break-all max-w-sm">{activeTab.url}</p>
          <p class="text-slate-500 text-sm max-w-sm mx-auto">This site is open in a native webview window. Switch to it using the taskbar or Alt+Tab.</p>
        </div>
        <div class="flex gap-2">
          <button on:click={() => openUrl(activeTab.url)} class="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">
            <ExternalLink size={14} /> Re-open
          </button>
          <button on:click={addTab} class="px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded-xl text-sm">New Tab</button>
        </div>
      </div>
    {/if}

    <SidePanel {panel} bookmarks={$bookmarks} history={$historyStore}
      on:close={() => (panel = 'none')}
      on:navigate={(e) => openUrl(e.detail)}
      on:clearHistory={hist.clearHistory}
    />
  </div>
</div>
