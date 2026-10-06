<script lang="ts">
  import { onMount } from 'svelte';
  import { Package, RefreshCw, Search, LayoutGrid, List, AlertCircle, Loader2, X } from 'lucide-svelte';
  import type { SoftwareTab, ViewMode, PackageInfo } from './types';
  import { createPackages } from './usePackages';
  import PackageCard from './PackageCard.svelte';
  import PackageRow from './PackageRow.svelte';
  import InstallLogTerminal from './InstallLogTerminal.svelte';
  import CommunityStore from './CommunityStore.svelte';
  import IconStore from './IconStore.svelte';
  import { t } from '../../../stores/language';
  import { showContextMenu } from '../../../stores/contextMenu';
  import type { PackageKind } from '../../../utils/blueStore';

  // Top-level sections: the system package manager (apt/dnf/pacman/flatpak/…)
  // plus the three community stores fed by the JSON indexes.
  type Section = 'system' | PackageKind | 'icons';
  let section: Section = 'system';
  // `labelKey` → translated; the older sections keep their plain labels.
  const sections: { id: Section; label?: string; labelKey?: string }[] = [
    { id: 'system', label: 'System' }, { id: 'app', label: 'Community Apps' },
    { id: 'plugin', label: 'Plugins' }, { id: 'theme', label: 'Themes' },
    { id: 'icons', labelKey: 'software.section.icons' },
  ];
  /** Blue Software gets its window id from the host; the icon store needs it to hide its native page. */
  export let windowId: string = '';
  // The store page is created lazily on first visit, then kept alive (just hidden) when switching sections.
  let iconsOpened = false;
  $: if (section === 'icons') iconsOpened = true;

  const { packages, loading, refreshing, error, activeAction, installLog, loadPackages, performAction, closeLog } = createPackages();

  let activeTab: SoftwareTab = 'installed';
  // Rendering thousands of cards at once (a normal Installed list) freezes
  // the webview; show a page at a time instead.
  const PAGE = 120;
  let shown = PAGE;
  $: if (searchQuery !== undefined || activeTab) shown = PAGE;
  let searchQuery = '';
  let viewMode: ViewMode = 'grid';

  onMount(loadPackages);

  function pkgMenu(e: MouseEvent, pkg: PackageInfo) {
    const copy = (t: string) => navigator.clipboard.writeText(t).catch(() => {});
    showContextMenu(e, [
      ...(pkg.installed
        ? [
            ...(pkg.update_available ? [{ label: 'Update', action: () => performAction(pkg, 'update') }] : []),
            { label: 'Uninstall', danger: true, action: () => performAction(pkg, 'remove') },
          ]
        : [{ label: 'Install', action: () => performAction(pkg, 'install') }]),
      { separator: true },
      { label: 'Copy package name', action: () => copy(pkg.name) },
      { label: 'Copy description', disabled: !pkg.description, action: () => copy(pkg.description) },
    ]);
  }

  $: filtered = $packages.filter((p) => {
    const q = searchQuery.toLowerCase();
    const matchSearch = !q || p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q);
    if (activeTab === 'available') return matchSearch && !p.installed;
    if (activeTab === 'installed') return matchSearch && p.installed;
    if (activeTab === 'updates') return matchSearch && p.installed && p.update_available;
    return matchSearch;
  });

  $: installed = $packages.filter((p) => p.installed).length;
  $: updates = $packages.filter((p) => p.installed && p.update_available).length;

  const tabs: { id: SoftwareTab; label: (pkgs: PackageInfo[]) => string }[] = [
    { id: 'available', label: (pkgs) => `Available (${pkgs.filter((p) => !p.installed).length})` },
    { id: 'installed', label: () => `Installed (${installed})` },
    { id: 'updates', label: () => `Updates (${updates})` },
  ];
</script>

<div class="flex flex-col h-full bg-slate-900 text-white">
  <div class="p-4 border-b border-white/5 shrink-0">
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-2">
        <Package size={26} class="text-blue-400" />
        <h1 class="text-xl font-bold">Blue Software</h1>
      </div>
      <button on:click={loadPackages} disabled={$refreshing} class="p-2 hover:bg-white/10 rounded-full transition-colors disabled:opacity-50">
        <RefreshCw size={16} class={$refreshing ? 'animate-spin' : ''} />
      </button>
    </div>

    <div class="flex gap-1 mb-3 p-1 bg-slate-800/60 rounded-xl w-fit">
      {#each sections as sec (sec.id)}
        <button on:click={() => (section = sec.id)} class="px-4 py-1.5 rounded-lg text-sm font-medium transition-colors {section === sec.id ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white hover:bg-white/5'}">{sec.labelKey ? $t(sec.labelKey) : sec.label}</button>
      {/each}
    </div>

    {#if section === 'system'}
    <div class="flex gap-0 border-b border-white/10 mb-3">
      {#each tabs as t (t.id)}
        <button on:click={() => (activeTab = t.id)}
          class="px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px {activeTab === t.id ? 'border-blue-500 text-blue-400' : 'border-transparent text-slate-400 hover:text-white'}">
          {t.label($packages)}
          {#if t.id === 'updates' && updates > 0}
            <span class="ml-1.5 px-1.5 py-0.5 rounded-full text-[10px] bg-orange-500/20 text-orange-300">{updates}</span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="flex items-center gap-3">
      <div class="relative flex-1">
        <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
        <input type="text" placeholder="Search packages…" bind:value={searchQuery}
          class="w-full bg-slate-800 border border-white/10 rounded-lg py-2 pl-9 pr-4 text-sm text-white focus:outline-none focus:border-blue-500/50" />
      </div>
      <div class="flex gap-1 bg-slate-800 rounded-lg p-1">
        <button on:click={() => (viewMode = 'grid')} class="p-1.5 rounded {viewMode === 'grid' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/10'}"><LayoutGrid size={15} /></button>
        <button on:click={() => (viewMode = 'list')} class="p-1.5 rounded {viewMode === 'list' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/10'}"><List size={15} /></button>
      </div>
    </div>
    {/if}
  </div>

  {#if iconsOpened}
    <div class="flex-1 min-h-0" class:hidden={section !== 'icons'}><IconStore {windowId} visible={section === 'icons'} /></div>
  {/if}
  {#if section === 'icons'}
    <!-- rendered by IconStore above -->
  {:else if section !== 'system'}
    <div class="flex-1 min-h-0"><CommunityStore kind={section} /></div>
  {:else}
  <div class="flex-1 overflow-y-auto p-4">
    {#if $error}
      <div class="bg-red-500/10 border border-red-500/30 rounded-xl p-3 mb-4 flex items-center gap-2 text-red-400 text-sm">
        <AlertCircle size={15} /> {$error}
        <button on:click={() => error.set(null)} class="ml-auto"><X size={14} /></button>
      </div>
    {/if}

    {#if $loading && filtered.length === 0}
      <div class="flex items-center justify-center h-48"><Loader2 size={28} class="animate-spin text-blue-400" /></div>
    {:else if filtered.length === 0}
      <div class="text-center py-16 text-slate-500">
        <Package size={40} class="mx-auto mb-3 opacity-40" />
        <p class="text-sm">No packages found</p>
        {#if activeTab === 'updates'}<p class="text-xs mt-1 text-slate-600">Everything is up to date</p>{/if}
        {#if activeTab === 'available' && !$loading}
          <p class="text-xs mt-1 text-slate-600">No packages returned — make sure apt/flatpak/snap is reachable and try refreshing.</p>
        {/if}
      </div>
    {:else if viewMode === 'grid'}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
        {#each filtered.slice(0, shown) as pkg (`${pkg.source}-${pkg.id}`)}
          <div on:contextmenu={(e) => pkgMenu(e, pkg)} role="presentation"><PackageCard {pkg} tab={activeTab} busy={$activeAction === pkg.id} on:action={(e) => performAction(pkg, e.detail)} /></div>
        {/each}
      </div>
    {:else}
      <div class="space-y-1">
        {#each filtered.slice(0, shown) as pkg (`${pkg.source}-${pkg.id}`)}
          <div on:contextmenu={(e) => pkgMenu(e, pkg)} role="presentation"><PackageRow {pkg} tab={activeTab} busy={$activeAction === pkg.id} on:action={(e) => performAction(pkg, e.detail)} /></div>
        {/each}
      </div>
    {/if}
    {#if filtered.length > shown}
      <div class="text-center py-4">
        <button on:click={() => (shown += PAGE)} class="px-4 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 text-sm">
          Show more ({filtered.length - shown} left)
        </button>
      </div>
    {/if}
    {#if $refreshing && !$loading}
      <div class="text-center text-xs text-slate-500 py-2 flex items-center justify-center gap-2"><Loader2 size={12} class="animate-spin" /> Checking updates and available apps…</div>
    {/if}
  </div>
  {/if}

  {#if $installLog}
    <InstallLogTerminal log={$installLog} on:close={closeLog} />
  {/if}
</div>
