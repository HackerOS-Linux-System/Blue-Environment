<script lang="ts">
  /**
   * Community store inside Blue Software: Apps / Plugins / Themes.
   * Entries come from the JSON indexes (config/stores/*.json via
   * `store_fetch_index`); install/uninstall/update run through the same
   * Blue Store backend (download → sha256 verify → privileged install) that
   * Settings → Plugins/Themes use, so everything stays consistent.
   */
  import { onMount, onDestroy } from 'svelte';
  import { Search, RefreshCw, Download, Trash2, Loader2, AlertCircle, Package, ArrowUpCircle, CheckCircle2 } from 'lucide-svelte';
  import {
    fetchStoreIndex, installPackage, uninstallPackage, listInstalled, checkUpdates, onStoreChanged, storeErrorMessage,
    type PackageKind, type StoreIndexEntry, type Receipt, type UpdateInfo,
  } from '../../../utils/blueStore';
  import { SystemBridge } from '../../../utils/systemBridge';

  export let kind: PackageKind;

  let entries: StoreIndexEntry[] = [];
  let warnings: string[] = [];
  let installed: Receipt[] = [];
  let updates: UpdateInfo[] = [];
  let loading = true;
  let error = '';
  let query = '';
  type Filter = 'all' | 'installed' | 'updates';
  let filter: Filter = 'all';
  const FILTERS: [Filter, string][] = [['all', 'All'], ['installed', 'Installed'], ['updates', 'Updates']];
  let busy: Record<string, { pct: number; message: string } | undefined> = {};
  let icons: Record<string, string> = {};
  let unlisten: (() => void) | null = null;
  let seq = 0;

  const LABEL: Record<PackageKind, string> = { app: 'applications', plugin: 'plugins', theme: 'themes' };

  // Index entries have no id; the installed receipt does. Match on name (what the
  // manifest's `name` is by convention) — good enough for display state.
  const norm = (s: string) => s.trim().toLowerCase();
  const receiptFor = (e: StoreIndexEntry) => installed.find((r) => norm(r.name) === norm(e.name));
  const updateFor = (e: StoreIndexEntry) => { const r = receiptFor(e); return r ? updates.find((u) => u.id === r.id) : undefined; };

  async function load() {
    const my = ++seq;
    loading = true; error = '';
    try {
      const idx = await fetchStoreIndex(kind);
      if (my !== seq) return;
      entries = idx.entries; warnings = idx.warnings;
      loading = false;                 // show the list right away…
      loadIcons(idx.entries);          // …icons stream in afterwards
    } catch (e) {
      if (my !== seq) return;
      error = storeErrorMessage(e); entries = [];
      loading = false;
    }
    refreshInstalled();
  }

  async function refreshInstalled() {
    try { installed = await listInstalled(kind); } catch { installed = []; }
    try { updates = await checkUpdates(kind); } catch { updates = []; }
  }

  // Remote icons can't be used directly (CSP blocks https images) → fetched
  // via the backend as data: URLs, a few at a time so the UI stays smooth.
  async function loadIcons(list: StoreIndexEntry[]) {
    const todo = list.filter((e) => e.icon && /^https:/i.test(e.icon) && !icons[e.icon]);
    for (let i = 0; i < todo.length; i += 4) {
      await Promise.all(todo.slice(i, i + 4).map(async (e) => {
        try { icons = { ...icons, [e.icon]: await SystemBridge.invoke<string>('store_fetch_image', { url: e.icon }) }; } catch { /* generic icon */ }
      }));
    }
  }

  async function install(e: StoreIndexEntry) {
    busy = { ...busy, [e.downloadUrl]: { pct: 0, message: 'Starting…' } };
    error = '';
    try {
      await installPackage(e.downloadUrl, kind, (pct, message) => { busy = { ...busy, [e.downloadUrl]: { pct, message } }; });
    } catch (err) { error = `${e.name}: ${storeErrorMessage(err)}`; }
    finally { const b = { ...busy }; delete b[e.downloadUrl]; busy = b; refreshInstalled(); }
  }

  async function remove(e: StoreIndexEntry) {
    const r = receiptFor(e); if (!r) return;
    busy = { ...busy, [e.downloadUrl]: { pct: 50, message: 'Removing…' } };
    try { await uninstallPackage(kind, r.id); } catch (err) { error = `${e.name}: ${storeErrorMessage(err)}`; }
    finally { const b = { ...busy }; delete b[e.downloadUrl]; busy = b; refreshInstalled(); }
  }

  $: shown = entries.filter((e) => {
    const q = query.toLowerCase();
    if (q && !(e.name.toLowerCase().includes(q) || e.description.toLowerCase().includes(q) || e.author.toLowerCase().includes(q))) return false;
    if (filter === 'installed') return !!receiptFor(e);
    if (filter === 'updates') return !!updateFor(e);
    return true;
  });
  $: kind, load();

  onMount(async () => { unlisten = await onStoreChanged(refreshInstalled); });
  onDestroy(() => unlisten?.());
</script>

<div class="flex flex-col h-full min-h-0">
  <div class="flex items-center gap-3 px-4 py-3 shrink-0">
    <div class="relative flex-1">
      <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
      <input type="text" bind:value={query} placeholder={`Search ${LABEL[kind]}…`}
        class="w-full bg-slate-800 border border-white/10 rounded-lg py-2 pl-9 pr-4 text-sm focus:outline-none focus:border-blue-500/50" />
    </div>
    <div class="flex rounded-lg overflow-hidden border border-white/10 text-xs">
      {#each FILTERS as [id, label]}
        <button on:click={() => (filter = id)} class="px-3 py-1.5 {filter === id ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/5'}">{label}</button>
      {/each}
    </div>
    <button on:click={load} class="p-2 hover:bg-white/10 rounded-full" title="Refresh"><RefreshCw size={15} class={loading ? 'animate-spin' : ''} /></button>
  </div>

  {#if error}
    <div class="mx-4 mb-2 flex items-start gap-2 text-xs text-red-300 bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2">
      <AlertCircle size={14} class="shrink-0 mt-0.5" /><span>{error}</span>
    </div>
  {/if}

  <div class="flex-1 overflow-auto px-4 pb-4">
    {#if loading}
      <div class="flex items-center justify-center py-16 text-slate-400"><Loader2 size={22} class="animate-spin mr-2" /> Loading the community index…</div>
    {:else if shown.length === 0}
      <div class="text-center py-16 text-slate-500 text-sm">
        {entries.length === 0 && !error ? `The community ${LABEL[kind]} index is empty.` : 'Nothing matches.'}
      </div>
    {:else}
      <div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));">
        {#each shown as e (e.downloadUrl)}
          {@const r = receiptFor(e)}
          {@const up = updateFor(e)}
          {@const b = busy[e.downloadUrl]}
          <div class="bg-slate-800/60 border border-white/5 rounded-xl p-3 flex flex-col gap-2">
            <div class="flex items-start gap-3">
              <div class="w-11 h-11 rounded-xl bg-slate-700/60 flex items-center justify-center overflow-hidden shrink-0">
                {#if icons[e.icon]}<img src={icons[e.icon]} alt="" class="w-full h-full object-cover" />{:else}<Package size={22} class="text-blue-300" />{/if}
              </div>
              <div class="min-w-0 flex-1">
                <div class="font-medium truncate">{e.name}</div>
                <div class="text-xs text-slate-400 truncate">{e.author || 'Unknown author'}{#if r} · v{r.version}{/if}</div>
              </div>
              {#if r && !up}<CheckCircle2 size={16} class="text-green-400 shrink-0" />{/if}
            </div>
            <p class="text-xs text-slate-300 line-clamp-3 min-h-[2.4rem]">{e.description || 'No description.'}</p>

            {#if b}
              <div class="mt-auto">
                <div class="h-1.5 rounded bg-white/10 overflow-hidden"><div class="h-full bg-blue-500 transition-all" style="width:{b.pct}%"></div></div>
                <div class="text-[11px] text-slate-400 mt-1 truncate">{b.message}</div>
              </div>
            {:else}
              <div class="mt-auto flex gap-2">
                {#if !r}
                  <button on:click={() => install(e)} class="flex-1 flex items-center justify-center gap-1.5 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-sm"><Download size={14} /> Install</button>
                {:else}
                  {#if up}<button on:click={() => install(e)} class="flex-1 flex items-center justify-center gap-1.5 py-1.5 rounded-lg bg-orange-500/80 hover:bg-orange-500 text-sm"><ArrowUpCircle size={14} /> Update to v{up.latestVersion}</button>{/if}
                  <button on:click={() => remove(e)} class="{up ? '' : 'flex-1'} px-3 flex items-center justify-center gap-1.5 py-1.5 rounded-lg bg-white/10 hover:bg-red-500/30 text-sm text-red-300"><Trash2 size={14} /> {up ? '' : 'Uninstall'}</button>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
      {#if warnings.length}<div class="text-[11px] text-slate-500 mt-3">{warnings.length} index entr{warnings.length === 1 ? 'y was' : 'ies were'} skipped (invalid).</div>{/if}
    {/if}
  </div>
</div>
