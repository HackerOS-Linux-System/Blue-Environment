<script lang="ts">
  /**
   * Blue Software → Icons: the KDE Store's icon themes as a NATIVE catalogue.
   *
   * Nothing of the website is shown. The backend (`icon_store_ocs.rs`) queries the
   * store's OCS API and this component draws the result itself — cards with
   * previews, search, sorting, details — so it looks like the rest of Blue
   * Software. "Install" downloads the theme archive and unpacks it into
   * `~/.local/share/icons` (where icon themes live); it is never left in
   * ~/Downloads. Progress is reported live, and the theme can be applied at once.
   */
  import { onMount, onDestroy, tick } from 'svelte';
  import {
    Search, X, Download, Check, Trash2, Loader2, ExternalLink, PackageOpen, Palette,
    RefreshCw, Star, ChevronLeft, ChevronRight, AlertTriangle,
  } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { t } from '../../../stores/language';
  import { isCompactWidth } from '../../../utils/compact';
  import { openInBlueWeb } from '../../../utils/openInBlueWeb';
  import {
    iconStore, primaryDownload, progressPercent, formatCount, formatSize, shortDate,
    type StoreIcon, type StoreDownload, type StoreSort, type InstallProgress,
  } from '../../../utils/iconStore';
  import { iconThemes, ICON_THEMES_CHANGED, ICON_THEME_APPLIED, type IconThemeInfo } from '../../../utils/iconThemes';
  import IconStoreThumb from './IconStoreThumb.svelte';

  /** Kept for API compatibility with the previous webview-based store — no native page to hide any more. */
  export let visible: boolean = true;

  const inTauri = SystemBridge.isTauri();
  const STORE_URL = 'https://store.kde.org/browse?cat=132&ord=latest';
  const SORTS: { id: StoreSort; key: string }[] = [
    { id: 'new', key: 'iconstore.sort_new' },
    { id: 'top', key: 'iconstore.sort_top' },
    { id: 'downloads', key: 'iconstore.sort_downloads' },
    { id: 'name', key: 'iconstore.sort_name' },
  ];

  let rootW = 0;
  $: compact = isCompactWidth(rootW);

  // ── catalogue ──
  let query = '';
  let sort: StoreSort = 'new';
  let items: StoreIcon[] = [];
  let page = 0;
  let hasMore = false;
  let total = 0;
  let loading = false;
  let loadError = '';
  let seq = 0;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let scroller: HTMLDivElement;
  let sentinel: HTMLDivElement;
  let observer: IntersectionObserver | null = null;
  let started = false;

  // ── installed / installing ──
  let installed: IconThemeInfo[] = [];
  let installedIds: Record<string, string[]> = {};
  let showInstalled = true;
  let busy: Record<string, boolean> = {};
  let progress: Record<string, InstallProgress> = {};
  let status: { kind: 'info' | 'ok' | 'error'; text: string; apply?: string[] } | null = null;

  // ── details ──
  let selected: StoreIcon | null = null;
  let detailLoading = false;
  let previewIdx = 0;

  const unlisten: (() => void)[] = [];

  const dedupe = (list: StoreIcon[]) => {
    const seen = new Set<string>();
    return list.filter((i) => (seen.has(i.id) ? false : (seen.add(i.id), true)));
  };

  async function load(reset: boolean) {
    if (!inTauri) return;
    if (!reset && (loading || !hasMore)) return;
    const my = ++seq;
    loading = true;
    loadError = '';
    const next = reset ? 0 : page + 1;
    try {
      const res = await iconStore.search(query, sort, next);
      if (my !== seq) return;
      items = reset ? dedupe(res.items) : dedupe([...items, ...res.items]);
      page = res.page;
      hasMore = res.hasMore;
      total = res.total;
    } catch (e: any) {
      if (my === seq) loadError = String(e?.message ?? e);
    } finally {
      if (my === seq) { loading = false; await tick(); fillViewport(); }
    }
  }

  /** The sentinel only reports *changes*; if a page was too short to push it out of view, ask for more. */
  function fillViewport() {
    if (!scroller || !sentinel || loading || !hasMore || loadError) return;
    const s = sentinel.getBoundingClientRect();
    const c = scroller.getBoundingClientRect();
    if (s.top < c.bottom + 200) load(false);
  }

  function onQueryInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => load(true), 350);
  }
  function clearQuery() { query = ''; if (searchTimer) clearTimeout(searchTimer); load(true); }
  function onSortChange() { load(true); }

  async function refreshInstalled() {
    installed = await iconThemes.list();
    installedIds = await iconStore.installedMap();
  }

  const isInstalled = (item: StoreIcon) => !!installedIds[item.id]?.length;

  function replaceItem(updated: StoreIcon) {
    items = items.map((i) => (i.id === updated.id ? updated : i));
    if (selected?.id === updated.id) selected = updated;
  }

  async function installItem(item: StoreIcon, file?: StoreDownload) {
    if (busy[item.id]) return;
    busy = { ...busy, [item.id]: true };
    status = { kind: 'info', text: $t('iconstore.installing', { name: item.name }) };
    try {
      let it = item;
      let f = file ?? primaryDownload(it);
      if (!f) {
        // List answers can omit the file list — ask for the full item.
        it = await iconStore.details(item.id);
        replaceItem(it);
        f = primaryDownload(it);
      }
      if (!f) {
        status = { kind: 'error', text: $t('iconstore.no_archive') };
        if (!selected) openDetails(it);
        return;
      }
      const names = await iconStore.install(it, f);
      status = { kind: 'ok', text: $t('iconstore.installed_ok', { themes: names.join(', ') }), apply: names };
      await refreshInstalled();
    } catch (e: any) {
      status = { kind: 'error', text: $t('iconstore.install_failed', { error: String(e?.message ?? e) }) };
    } finally {
      const { [item.id]: _b, ...restBusy } = busy; busy = restBusy;
      const { [item.id]: _p, ...restProg } = progress; progress = restProg;
    }
  }

  async function openDetails(item: StoreIcon) {
    selected = item;
    previewIdx = 0;
    if (!item.description || item.downloads.length === 0) {
      detailLoading = true;
      try {
        const full = await iconStore.details(item.id);
        if (selected?.id === item.id) { replaceItem({ ...item, ...full }); }
      } catch { /* the card data is enough */ }
      finally { detailLoading = false; }
    }
  }
  const closeDetails = () => { selected = null; };

  async function apply(name: string | null) {
    try {
      await iconThemes.apply(name);
      status = { kind: 'ok', text: $t('iconstore.applied', { name: name ?? $t('iconstore.system_theme') }) };
    } catch (e: any) { status = { kind: 'error', text: String(e?.message ?? e) }; }
    refreshInstalled();
  }
  async function remove(th: IconThemeInfo) {
    try {
      await iconThemes.remove(th.name);
      status = { kind: 'info', text: $t('iconstore.removed', { name: th.name }) };
    } catch (e: any) { status = { kind: 'error', text: String(e?.message ?? e) }; }
    refreshInstalled();
  }

  const openStorePage = (url: string) => openInBlueWeb(url);
  const openExternal = () => window.open(STORE_URL, '_blank', 'noopener');

  function percentText(id: string): string {
    const p = progress[id];
    if (!p) return $t('iconstore.preparing');
    if (p.phase !== 'download') return $t('iconstore.unpacking');
    const pct = progressPercent(p);
    return pct === null ? $t('iconstore.preparing') : $t('iconstore.downloading', { percent: pct });
  }

  function start() {
    if (started || !inTauri) return;
    started = true;
    load(true);
  }
  // Lazy: the first visit to the section triggers the first request.
  $: if (visible) start();

  onMount(async () => {
    refreshInstalled();
    const onChange = () => refreshInstalled();
    window.addEventListener(ICON_THEMES_CHANGED, onChange);
    window.addEventListener(ICON_THEME_APPLIED, onChange);
    unlisten.push(() => { window.removeEventListener(ICON_THEMES_CHANGED, onChange); window.removeEventListener(ICON_THEME_APPLIED, onChange); });
    if (inTauri) {
      unlisten.push(await iconStore.onProgress((p) => { progress = { ...progress, [p.id]: p }; }));
      if (typeof IntersectionObserver !== 'undefined' && sentinel) {
        observer = new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) load(false); }, { root: scroller, rootMargin: '300px' });
        observer.observe(sentinel);
      }
    }
  });
  onDestroy(() => {
    if (searchTimer) clearTimeout(searchTimer);
    observer?.disconnect();
    unlisten.forEach((u) => u());
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && selected) { e.stopPropagation(); closeDetails(); }
  }
</script>

<svelte:window on:keydown={onKey} />

<div bind:clientWidth={rootW} class="relative flex flex-col h-full min-h-0 bg-slate-900" class:hidden={!visible}>
  {#if !inTauri}
    <div class="flex-1 flex flex-col items-center justify-center gap-3 text-slate-400 text-sm p-6 text-center">
      <PackageOpen size={36} class="opacity-50" />
      <p>{$t('iconstore.needs_desktop')}</p>
      <button class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-xs" on:click={openExternal}><ExternalLink size={13} /> {$t('iconstore.open_in_browser')}</button>
    </div>
  {:else}
    <!-- toolbar -->
    <div class="flex items-center gap-2 px-3 py-2 border-b border-white/5 shrink-0 flex-wrap">
      <div class="relative flex-1 min-w-[10rem]">
        <Search size={14} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none" />
        <input
          type="search" bind:value={query} on:input={onQueryInput}
          placeholder={$t('iconstore.search_ph')}
          class="w-full pl-8 pr-8 py-1.5 rounded-lg bg-slate-800 border border-white/5 text-sm text-slate-200 placeholder-slate-500 outline-none focus:border-blue-500/60"
        />
        {#if query}
          <button class="absolute right-1.5 top-1/2 -translate-y-1/2 p-1 rounded-md text-slate-500 hover:text-slate-200 hover:bg-white/10" on:click={clearQuery} aria-label={$t('iconstore.close')}><X size={13} /></button>
        {/if}
      </div>
      <select bind:value={sort} on:change={onSortChange} class="py-1.5 px-2 rounded-lg bg-slate-800 border border-white/5 text-xs text-slate-300 outline-none">
        {#each SORTS as s (s.id)}<option value={s.id}>{$t(s.key)}</option>{/each}
      </select>
      <button class="p-1.5 rounded-lg hover:bg-white/10 text-slate-300 disabled:opacity-40" title={$t('iconstore.reload')} disabled={loading} on:click={() => load(true)}>
        <RefreshCw size={15} class={loading ? 'animate-spin' : ''} />
      </button>
      <button class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs transition-colors {showInstalled ? 'bg-blue-600/30 text-blue-200' : 'hover:bg-white/10 text-slate-300'}" on:click={() => (showInstalled = !showInstalled)}>
        <Palette size={14} /> {$t('iconstore.installed')} ({installed.length})
      </button>
    </div>

    <div class="flex flex-1 min-h-0">
      <!-- catalogue -->
      <div bind:this={scroller} class="flex-1 min-w-0 overflow-y-auto p-3">
        {#if loadError && items.length === 0}
          <div class="h-full flex flex-col items-center justify-center gap-3 text-sm text-slate-400 text-center p-6">
            <AlertTriangle size={30} class="text-amber-400" />
            <p class="max-w-md break-words">{$t('iconstore.load_failed', { error: loadError })}</p>
            <button class="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 text-xs" on:click={() => load(true)}>{$t('iconstore.retry')}</button>
          </div>
        {:else if !loading && items.length === 0}
          <div class="h-full flex flex-col items-center justify-center gap-2 text-sm text-slate-500"><PackageOpen size={30} class="opacity-50" /><p>{$t('iconstore.no_results')}</p></div>
        {:else}
          {#if total > 0}<p class="text-[11px] text-slate-500 mb-2">{$t('iconstore.results_n', { n: total })}</p>{/if}
          <div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax({compact ? 150 : 190}px, 1fr));">
            {#each items as item (item.id)}
              <div class="group flex flex-col rounded-xl bg-slate-800/50 border border-white/5 overflow-hidden hover:border-blue-500/40 transition-colors">
                <button class="block w-full aspect-[4/3] text-left" on:click={() => openDetails(item)} title={item.name}>
                  <IconStoreThumb url={item.thumb} alt={item.name} />
                </button>
                <div class="p-2.5 flex flex-col gap-1.5 flex-1">
                  <button class="text-left min-w-0" on:click={() => openDetails(item)}>
                    <div class="text-sm font-medium text-slate-100 truncate">{item.name}</div>
                    <div class="text-[11px] text-slate-500 truncate">{item.author ? $t('iconstore.by_author', { author: item.author }) : ''}</div>
                  </button>
                  <div class="flex items-center gap-3 text-[11px] text-slate-400">
                    <span class="flex items-center gap-1" title={$t('iconstore.downloads_n', { n: item.downloadsCount })}><Download size={11} /> {formatCount(item.downloadsCount)}</span>
                    {#if item.score > 0}<span class="flex items-center gap-1" title={$t('iconstore.rating', { score: item.score })}><Star size={11} class="text-amber-400" /> {item.score}%</span>{/if}
                  </div>
                  <div class="mt-auto pt-1">
                    {#if busy[item.id]}
                      {@const pct = progressPercent(progress[item.id])}
                      <div class="text-[11px] text-slate-300 mb-1 flex items-center gap-1.5"><Loader2 size={11} class="animate-spin" /> {percentText(item.id)}</div>
                      <div class="h-1.5 rounded-full bg-slate-700 overflow-hidden">
                        <div class="h-full bg-blue-500 transition-[width] duration-150 {pct === null ? 'animate-pulse w-full' : ''}" style={pct === null ? '' : `width:${pct}%`}></div>
                      </div>
                    {:else}
                      <button class="w-full flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-lg text-xs font-medium transition-colors {isInstalled(item) ? 'bg-white/10 hover:bg-white/15 text-slate-200' : 'bg-blue-600 hover:bg-blue-500 text-white'}" on:click={() => installItem(item)}>
                        {#if isInstalled(item)}<Check size={13} class="text-emerald-400" /> {$t('iconstore.reinstall')}{:else}<Download size={13} /> {$t('iconstore.install')}{/if}
                      </button>
                    {/if}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}

        <!-- infinite scroll sentinel -->
        <div bind:this={sentinel} class="h-8 flex items-center justify-center">
          {#if loading && items.length > 0}
            <Loader2 size={16} class="animate-spin text-slate-500" />
          {:else if loadError && items.length > 0}
            <button class="px-3 py-1 rounded-lg bg-white/10 hover:bg-white/15 text-xs text-slate-300" on:click={() => load(false)}>{$t('iconstore.retry')}</button>
          {:else if hasMore}
            <button class="px-3 py-1 rounded-lg bg-white/5 hover:bg-white/10 text-xs text-slate-400" on:click={() => load(false)}>{$t('iconstore.load_more')}</button>
          {/if}
        </div>
        {#if loading && items.length === 0}
          <div class="flex items-center justify-center py-16 text-slate-500"><Loader2 size={26} class="animate-spin" /></div>
        {/if}
      </div>

      <!-- installed themes -->
      {#if showInstalled}
        <aside class="{compact ? 'w-40' : 'w-64'} shrink-0 border-l border-white/5 flex flex-col min-h-0 bg-slate-900">
          <div class="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-slate-500">{$t('iconstore.installed')}</div>
          <div class="flex-1 overflow-y-auto px-2 pb-2 space-y-1">
            <button class="w-full flex items-center gap-2 px-2.5 py-2 rounded-lg text-left text-xs hover:bg-white/5 {installed.every((x) => !x.active) ? 'bg-blue-600/20' : ''}" on:click={() => apply(null)}>
              <span class="flex-1 truncate">{$t('iconstore.system_theme')}</span>
              {#if installed.every((x) => !x.active)}<Check size={13} class="text-blue-400" />{/if}
            </button>
            {#each installed as th (th.name)}
              <div class="group flex items-center gap-1 rounded-lg {th.active ? 'bg-blue-600/20' : 'hover:bg-white/5'}">
                <button class="flex-1 min-w-0 flex items-center gap-2 px-2.5 py-2 text-left text-xs" on:click={() => apply(th.name)} title={th.path}>
                  <span class="flex-1 truncate">{th.name}</span>
                  {#if th.active}<Check size={13} class="text-blue-400 shrink-0" />{/if}
                </button>
                {#if th.removable}
                  <button class="p-1.5 mr-1 rounded-md text-slate-500 hover:text-red-400 hover:bg-white/5 opacity-0 group-hover:opacity-100" title={$t('iconstore.remove')} on:click={() => remove(th)}><Trash2 size={13} /></button>
                {/if}
              </div>
            {:else}
              <p class="px-2.5 py-3 text-xs text-slate-600">{$t('iconstore.none')}</p>
            {/each}
          </div>
        </aside>
      {/if}
    </div>

    <!-- status line -->
    <div class="shrink-0 border-t border-white/5 px-3 py-2 text-xs flex items-center gap-2 min-h-[2.25rem] {status?.kind === 'error' ? 'text-red-300' : status?.kind === 'ok' ? 'text-emerald-300' : 'text-slate-400'}">
      {#if Object.keys(busy).length > 0}<Loader2 size={13} class="animate-spin shrink-0" />{/if}
      <span class="truncate flex-1">{status ? status.text : $t('iconstore.hint')}</span>
      {#if status?.apply}
        {#each status.apply as name (name)}
          <button class="shrink-0 px-2.5 py-1 rounded-md bg-blue-600 hover:bg-blue-500 text-white" on:click={() => apply(name)}>{$t('iconstore.apply')} {status.apply.length > 1 ? name : ''}</button>
        {/each}
      {/if}
    </div>

    <!-- details -->
    {#if selected}
      {@const sel = selected}
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
      <div class="absolute inset-0 z-20 bg-black/60 flex items-center justify-center p-3" on:click|self={closeDetails}>
        <div class="w-full max-w-3xl max-h-full flex flex-col rounded-2xl bg-slate-900 border border-white/10 shadow-2xl overflow-hidden" role="dialog" aria-label={sel.name}>
          <div class="flex items-center gap-2 px-4 py-3 border-b border-white/5 shrink-0">
            <div class="flex-1 min-w-0">
              <div class="text-base font-semibold text-slate-100 truncate">{sel.name}</div>
              <div class="text-[11px] text-slate-500 truncate">
                {[sel.author ? $t('iconstore.by_author', { author: sel.author }) : '', sel.version ? $t('iconstore.version', { v: sel.version }) : '', sel.changed ? $t('iconstore.updated', { date: shortDate(sel.changed) }) : ''].filter(Boolean).join(' · ')}
              </div>
            </div>
            <button class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400" on:click={closeDetails} aria-label={$t('iconstore.close')}><X size={16} /></button>
          </div>
          <div class="flex-1 min-h-0 overflow-y-auto p-4 space-y-4">
            {#if sel.previews.length > 0 || sel.thumb}
              <div class="relative rounded-xl overflow-hidden bg-slate-800/60 aspect-video">
                {#key (sel.previews[previewIdx] ?? sel.thumb)}
                  <IconStoreThumb url={sel.previews[previewIdx] ?? sel.thumb} alt={sel.name} eager fit="contain" />
                {/key}
                {#if sel.previews.length > 1}
                  <button class="absolute left-2 top-1/2 -translate-y-1/2 p-1.5 rounded-full bg-black/50 hover:bg-black/70 text-white" on:click={() => (previewIdx = (previewIdx - 1 + sel.previews.length) % sel.previews.length)}><ChevronLeft size={16} /></button>
                  <button class="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 rounded-full bg-black/50 hover:bg-black/70 text-white" on:click={() => (previewIdx = (previewIdx + 1) % sel.previews.length)}><ChevronRight size={16} /></button>
                  <span class="absolute bottom-2 right-2 px-2 py-0.5 rounded-full bg-black/60 text-[10px] text-white">{previewIdx + 1} / {sel.previews.length}</span>
                {/if}
              </div>
            {/if}

            <div class="flex items-center gap-4 text-xs text-slate-400">
              <span class="flex items-center gap-1"><Download size={12} /> {$t('iconstore.downloads_n', { n: formatCount(sel.downloadsCount) })}</span>
              {#if sel.score > 0}<span class="flex items-center gap-1"><Star size={12} class="text-amber-400" /> {$t('iconstore.rating', { score: sel.score })}</span>{/if}
              <button class="ml-auto flex items-center gap-1.5 px-2.5 py-1 rounded-lg hover:bg-white/10 text-slate-300" on:click={() => openStorePage(sel.detailUrl)}><ExternalLink size={12} /> {$t('iconstore.open_store_page')}</button>
            </div>

            {#if sel.description || sel.summary}
              <p class="text-sm text-slate-300 whitespace-pre-line break-words">{sel.description || sel.summary}</p>
            {:else if detailLoading}
              <div class="flex items-center gap-2 text-xs text-slate-500"><Loader2 size={13} class="animate-spin" /> {$t('iconstore.loading')}</div>
            {/if}

            <div>
              <div class="text-[10px] font-semibold uppercase tracking-wider text-slate-500 mb-2">{$t('iconstore.files')}</div>
              {#if sel.downloads.length === 0}
                <p class="text-xs text-slate-500">{detailLoading ? $t('iconstore.loading') : $t('iconstore.no_archive')}</p>
              {:else}
                <div class="space-y-1.5">
                  {#each sel.downloads as f (f.index)}
                    <div class="flex items-center gap-3 px-3 py-2 rounded-lg bg-slate-800/50 border border-white/5">
                      <div class="flex-1 min-w-0">
                        <div class="text-xs text-slate-200 truncate">{f.name}</div>
                        <div class="text-[11px] text-slate-500">{formatSize(f.sizeKb)}{f.isArchive ? '' : ` · ${$t('iconstore.not_archive_file')}`}</div>
                      </div>
                      {#if busy[sel.id]}
                        <span class="text-[11px] text-slate-400 flex items-center gap-1.5 shrink-0"><Loader2 size={12} class="animate-spin" /> {percentText(sel.id)}</span>
                      {:else}
                        <button class="shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-40 disabled:cursor-not-allowed" disabled={!f.isArchive} on:click={() => installItem(sel, f)}>
                          <Download size={13} /> {isInstalled(sel) ? $t('iconstore.reinstall') : $t('iconstore.install')}
                        </button>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        </div>
      </div>
    {/if}
  {/if}
</div>
