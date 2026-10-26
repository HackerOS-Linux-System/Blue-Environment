<script lang="ts">
  /**
   * Widget Store inside Blue Software. Browse the catalogue
   * (`config/stores/widgets-store.json`), install presets into the desktop's
   * "add widget" palette, and put them on the desktop straight from here.
   * Built-in widgets stay available and are only listed for completeness.
   */
  import { onMount } from 'svelte';
  import { Search, RefreshCw, Download, Trash2, Loader2, CheckCircle2, AlertCircle, Plus, WifiOff, LayoutDashboard } from 'lucide-svelte';
  import { fetchWidgetCatalog } from '../../../utils/blueStore';
  import { CATALOG_CATEGORIES, searchCatalog, type CatalogEntry } from '../../../widgets/widgetCatalog';
  import { installedWidgets, loadInstalledWidgets, loadWidgets, installWidget, uninstallWidget, addCatalogWidget, widgets, widgetEditMode } from '../../../widgets/widgetStore';
  import WidgetPreview from './WidgetPreview.svelte';

  let entries = $state<CatalogEntry[]>([]);
  let warnings = $state<string[]>([]);
  let loading = $state(true);
  let offline = $state(false);
  let notice = $state('');
  let query = $state('');
  let category = $state('all');
  type Filter = 'all' | 'installed' | 'available';
  let filter = $state<Filter>('all');
  let busy = $state<string | null>(null);
  let flash = $state<string | null>(null);

  const FILTERS: [Filter, string][] = [['all', 'All'], ['installed', 'Installed'], ['available', 'Not installed']];

  let installedIds = $derived(new Set($installedWidgets.map((e) => e.id)));
  const isInstalled = (e: CatalogEntry) => !!e.builtin || installedIds.has(e.id);
  const onDesktop = (e: CatalogEntry) => $widgets.filter((w) => (e.builtin ? w.type === e.type && !w.storeId : w.storeId === e.id)).length;

  let shown = $derived(
    searchCatalog(entries, query, category).filter((e) => (filter === 'all' ? true : filter === 'installed' ? isInstalled(e) : !isInstalled(e))),
  );
  let categories = $derived(CATALOG_CATEGORIES.filter((c) => entries.some((e) => e.category === c)));
  let installedCount = $derived(entries.filter((e) => !e.builtin && installedIds.has(e.id)).length);

  async function load() {
    loading = true;
    const r = await fetchWidgetCatalog();
    entries = r.entries; warnings = r.warnings; offline = r.offline;
    notice = r.offline ? `Showing the catalogue bundled with this version${r.error ? ` (${r.error})` : ''}.` : '';
    loading = false;
  }

  onMount(() => { void loadInstalledWidgets(); void loadWidgets(); void load(); });

  async function install(e: CatalogEntry) {
    busy = e.id;
    await installWidget(e);
    busy = null;
    flash = `${e.name} installed — find it in “Edit widgets” on the desktop.`;
    setTimeout(() => { if (flash?.startsWith(e.name)) flash = null; }, 4000);
  }
  async function remove(e: CatalogEntry) {
    busy = e.id;
    await uninstallWidget(e.id);
    busy = null;
  }
  function place(e: CatalogEntry) {
    addCatalogWidget(e);
    flash = `${e.name} added to the desktop.`;
    setTimeout(() => { if (flash?.startsWith(e.name)) flash = null; }, 3000);
  }
  function editDesktop() { widgetEditMode.set(true); flash = 'Desktop edit mode is on — close this window to arrange the widgets.'; }
</script>

<div class="h-full flex flex-col min-h-0">
  <div class="px-4 pt-3 pb-2 flex flex-wrap items-center gap-2 shrink-0">
    <div class="relative flex-1 min-w-[180px]">
      <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
      <input type="text" placeholder="Search widgets…" bind:value={query} class="w-full bg-slate-800 border border-white/10 rounded-lg py-2 pl-9 pr-3 text-sm text-white focus:outline-none focus:border-blue-500/50" />
    </div>
    <div class="flex gap-1 bg-slate-800 rounded-lg p-1">
      {#each FILTERS as [id, label]}
        <button onclick={() => (filter = id)} class="px-3 py-1 rounded text-xs transition-colors {filter === id ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/10'}">{label}{#if id === 'installed' && installedCount > 0}<span class="ml-1 opacity-70">+{installedCount}</span>{/if}</button>
      {/each}
    </div>
    <button onclick={load} disabled={loading} class="p-2 rounded-lg hover:bg-white/10 transition-colors disabled:opacity-50" title="Reload the catalogue"><RefreshCw size={15} class={loading ? 'animate-spin' : ''} /></button>
    <button onclick={editDesktop} class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs text-slate-200 transition-colors" title="Turn on the desktop's widget edit mode"><LayoutDashboard size={14} /> Edit desktop</button>
  </div>

  <div class="px-4 pb-2 flex gap-1.5 overflow-x-auto shrink-0">
    {#each ['all', ...categories] as c}
      <button onclick={() => (category = c)} class="px-3 py-1 rounded-full text-xs whitespace-nowrap border transition-colors {category === c ? 'bg-blue-500/20 border-blue-400/60 text-blue-200' : 'border-white/10 text-slate-400 hover:text-white hover:border-white/25'}">{c === 'all' ? 'All categories' : c}</button>
    {/each}
  </div>

  {#if notice}
    <div class="mx-4 mb-2 flex items-center gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-1.5 text-xs text-amber-200 shrink-0"><WifiOff size={13} /> {notice}</div>
  {/if}
  {#if flash}
    <div class="mx-4 mb-2 flex items-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-1.5 text-xs text-emerald-200 shrink-0"><CheckCircle2 size={13} /> {flash}</div>
  {/if}

  <div class="flex-1 overflow-y-auto px-4 pb-4">
    {#if loading}
      <div class="flex items-center justify-center gap-2 py-16 text-slate-400 text-sm"><Loader2 size={18} class="animate-spin" /> Loading widgets…</div>
    {:else if shown.length === 0}
      <div class="flex flex-col items-center gap-2 py-16 text-slate-500 text-sm"><AlertCircle size={22} /> No widgets match.</div>
    {:else}
      <div class="grid gap-3" style="grid-template-columns:repeat(auto-fill,minmax(210px,1fr));">
        {#each shown as e (e.id)}
          {@const installed = isInstalled(e)}
          {@const count = onDesktop(e)}
          <div class="group flex flex-col rounded-2xl overflow-hidden border border-white/10 bg-slate-800/60 hover:border-white/25 transition-colors">
            <div class="relative h-28" style="background:linear-gradient(135deg,{e.accent ?? '#3b82f6'}55,{e.accent ?? '#3b82f6'}12 70%,transparent);">
              <div class="absolute top-2 left-2 flex items-center gap-1.5 text-[11px] text-white/80"><span class="text-base leading-none">{e.icon}</span>{e.category}</div>
              {#if e.builtin}<span class="absolute top-2 right-2 rounded-full bg-white/15 px-2 py-0.5 text-[10px] text-white/80">Built-in</span>
              {:else if installed}<span class="absolute top-2 right-2 flex items-center gap-1 rounded-full bg-emerald-500/25 px-2 py-0.5 text-[10px] text-emerald-200"><CheckCircle2 size={10} /> Installed</span>{/if}
              <div class="absolute inset-x-3 bottom-2 top-8 rounded-xl bg-slate-900/55 backdrop-blur border border-white/10"><WidgetPreview entry={e} /></div>
            </div>
            <div class="flex-1 flex flex-col gap-1 p-3">
              <div class="flex items-baseline justify-between gap-2"><h3 class="text-sm font-semibold truncate">{e.name}</h3><span class="text-[10px] text-slate-500 shrink-0">v{e.version}</span></div>
              <p class="text-xs text-slate-400 leading-snug line-clamp-3 flex-1">{e.description}</p>
              <div class="flex items-center justify-between text-[10px] text-slate-500"><span class="truncate">{e.author}</span>{#if count > 0}<span class="text-blue-300">{count} on desktop</span>{/if}</div>
              <div class="flex gap-1.5 mt-1">
                {#if !installed}
                  <button onclick={() => install(e)} disabled={busy === e.id} class="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-50 py-1.5 text-xs font-medium transition-colors">{#if busy === e.id}<Loader2 size={12} class="animate-spin" />{:else}<Download size={12} />{/if} Install</button>
                {:else}
                  <button onclick={() => place(e)} class="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 py-1.5 text-xs font-medium transition-colors"><Plus size={12} /> Add to desktop</button>
                  {#if !e.builtin}
                    <button onclick={() => remove(e)} disabled={busy === e.id} class="rounded-lg bg-slate-700 hover:bg-red-600/80 px-2.5 text-slate-200 transition-colors disabled:opacity-50" title="Uninstall (also removes it from the desktop)" aria-label="Uninstall"><Trash2 size={13} /></button>
                  {/if}
                {/if}
              </div>
            </div>
          </div>
        {/each}
      </div>
      {#if warnings.length}
        <details class="mt-4 text-[11px] text-slate-500"><summary class="cursor-pointer">{warnings.length} catalogue {warnings.length === 1 ? 'entry was' : 'entries were'} skipped</summary><ul class="mt-1 list-disc pl-5">{#each warnings as w}<li>{w}</li>{/each}</ul></details>
      {/if}
    {/if}
  </div>
</div>
