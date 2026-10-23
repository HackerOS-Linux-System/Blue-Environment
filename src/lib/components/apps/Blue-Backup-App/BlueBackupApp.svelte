<script lang="ts">
  import { preventDefault } from 'svelte/legacy';

  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { History, Play, Settings2, Folder, File as FileIcon, Trash2, RotateCcw, CheckCircle2, AlertTriangle, Loader2, ChevronRight, Plus, X, ArchiveRestore, HardDrive } from 'lucide-svelte';
  import { t } from '../../../stores/language';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { dialogConfirm } from '../../../stores/dialog';
  import { formatBytes } from '../../../utils/mediaWriter';
  import { type BackupConfig, type Snapshot, type BackupEntry, breadcrumbs, joinRel, parentRel, originalPath, destinationProblem, toggleSource, parseSnapshotName } from '../../../utils/backupView';

  type Tab = 'overview' | 'browse' | 'settings';
  let tab = $state<Tab>('overview');
  const setTab = (id: string) => { tab = id as Tab; };

  let cfg = $state<BackupConfig>({ destination: '', sources: [], excludes: [], keepDaily: 7, keepWeekly: 4, keepMonthly: 6, schedule: 'off' });
  let snapshots = $state<Snapshot[]>([]);
  let running = $state(false);
  let phase = $state('');
  let pct = $state(0);
  let message = $state('');
  let error = $state('');
  let saved = $state(false);
  let newExclude = $state('');

  let selected = $state<Snapshot | null>(null);
  let rel = $state('');
  let entries = $state<BackupEntry[]>([]);
  let browseLoading = $state(false);

  let unlisten: UnlistenFn | undefined;

  let problem = $derived(destinationProblem(cfg));
  let last = $derived(snapshots[0] ?? null);
  const fmt = (name: string) => { const d = parseSnapshotName(name); return d ? d.toLocaleString() : name; };

  onMount(async () => {
    unlisten = await listen<{ phase: string; pct: number; bytes: number }>('blue-backup://progress', (e) => {
      phase = e.payload.phase; pct = e.payload.pct;
    });
    await reload();
    running = await SystemBridge.invokeCommand<boolean>('backup_is_running', {}).catch(() => false);
  });
  onDestroy(() => unlisten?.());

  async function reload() {
    try {
      cfg = await SystemBridge.invokeCommand<BackupConfig>('backup_get_config', {});
      snapshots = await SystemBridge.invokeCommand<Snapshot[]>('backup_list', {});
    } catch (e: any) { error = String(e?.message ?? e); }
  }

  async function runNow() {
    error = ''; message = ''; running = true; pct = 0; phase = 'prepare';
    try {
      const s = await SystemBridge.invokeCommand<Snapshot>('backup_run', {});
      message = s.warnings > 0 ? $t('backup.done_warn').replace('{n}', String(s.warnings)) : $t('backup.done');
    } catch (e: any) {
      const m = String(e?.message ?? e);
      error = m === 'Cancelled' ? $t('backup.cancelled') : m;
    } finally {
      running = false;
      await reload();
    }
  }

  const cancel = () => SystemBridge.invokeCommand('backup_cancel', {}).catch(() => {});

  async function saveConfig() {
    error = ''; saved = false;
    try {
      await SystemBridge.invokeCommand('backup_set_config', { config: cfg });
      saved = true; setTimeout(() => (saved = false), 2500);
    } catch (e: any) { error = String(e?.message ?? e); }
  }

  async function pickDestination() {
    const d = await SystemBridge.pickDirectory('blue-backup.destination');
    if (d) cfg = { ...cfg, destination: d };
  }
  async function addSource() {
    const d = await SystemBridge.pickDirectory('blue-backup.source');
    if (d && !cfg.sources.includes(d)) cfg = { ...cfg, sources: toggleSource(cfg.sources, d) };
  }
  function addExclude() {
    const v = newExclude.trim();
    if (v && !cfg.excludes.includes(v)) cfg = { ...cfg, excludes: [...cfg.excludes, v] };
    newExclude = '';
  }

  async function openSnapshot(s: Snapshot) {
    selected = s; rel = ''; tab = 'browse';
    await loadDir();
  }
  async function loadDir() {
    if (!selected) return;
    browseLoading = true; error = '';
    try { entries = await SystemBridge.invokeCommand<BackupEntry[]>('backup_browse', { snapshot: selected.name, path: rel }); }
    catch (e: any) { error = String(e?.message ?? e); entries = []; }
    finally { browseLoading = false; }
  }
  async function enter(e: BackupEntry) { if (e.isDir) { rel = joinRel(rel, e.name); await loadDir(); } }
  async function jump(path: string) { rel = path; await loadDir(); }

  async function restoreEntry(e: BackupEntry, inPlace: boolean) {
    if (!selected) return;
    const itemRel = joinRel(rel, e.name);
    if (inPlace) {
      const ok = await dialogConfirm({
        title: $t('backup.restore.inplace_title'),
        message: $t('backup.restore.inplace_body').replace('{path}', originalPath(itemRel)),
        confirmLabel: $t('backup.restore.inplace_confirm'), cancelLabel: $t('common.cancel'), danger: true,
      });
      if (!ok) return;
    }
    error = ''; message = '';
    try {
      const where = await SystemBridge.invokeCommand<string>('backup_restore', { snapshot: selected.name, path: itemRel, inPlace });
      message = $t('backup.restore.done').replace('{path}', where);
    } catch (err: any) { error = String(err?.message ?? err); }
  }

  async function deleteSnap(s: Snapshot) {
    const ok = await dialogConfirm({ title: $t('backup.delete.title'), message: $t('backup.delete.body').replace('{when}', fmt(s.name)), confirmLabel: $t('backup.delete.confirm'), cancelLabel: $t('common.cancel'), danger: true });
    if (!ok) return;
    try { await SystemBridge.invokeCommand('backup_delete', { snapshot: s.name }); if (selected?.name === s.name) { selected = null; entries = []; } await reload(); }
    catch (e: any) { error = String(e?.message ?? e); }
  }
</script>

<div class="flex flex-col h-full bg-slate-900 text-white">
  <div class="flex items-center gap-3 px-4 py-2.5 border-b border-white/5 shrink-0 bg-slate-800/50">
    <History size={18} class="text-blue-400" />
    <span class="font-semibold text-sm">{$t('backup.title')}</span>
    <div class="flex-1"></div>
    <div class="flex bg-slate-800 rounded-lg p-0.5 text-xs" role="tablist">
      {#each [['overview', 'backup.tab.overview'], ['browse', 'backup.tab.browse'], ['settings', 'backup.tab.settings']] as [id, label]}
        <button role="tab" aria-selected={tab === id} onclick={() => setTab(id)} class="px-3 py-1 rounded-md transition-colors {tab === id ? 'bg-blue-600' : 'text-slate-400 hover:text-white'}">{$t(label)}</button>
      {/each}
    </div>
  </div>

  <div class="flex-1 overflow-y-auto p-5">
    {#if error}
      <div class="mb-4 flex items-start gap-2 bg-red-500/10 border border-red-500/30 text-red-200 rounded-lg px-3 py-2 text-sm" role="alert"><AlertTriangle size={16} class="mt-0.5 shrink-0" /> <span class="break-words min-w-0">{error}</span></div>
    {/if}
    {#if message}
      <div class="mb-4 flex items-start gap-2 bg-green-500/10 border border-green-500/30 text-green-200 rounded-lg px-3 py-2 text-sm" role="status"><CheckCircle2 size={16} class="mt-0.5 shrink-0" /> <span class="break-words min-w-0">{message}</span></div>
    {/if}

    {#if tab === 'overview'}
      <div class="rounded-2xl bg-slate-800 p-5 mb-4">
        <div class="text-xs text-slate-400">{$t('backup.last')}</div>
        <div class="text-lg font-semibold mt-0.5">{last ? fmt(last.name) : $t('backup.never')}</div>
        {#if last}<div class="text-xs text-slate-400 mt-0.5">{$t('backup.new_data').replace('{size}', formatBytes(last.transferredBytes))}</div>{/if}
        {#if running}
          <div class="mt-4">
            <div class="flex items-center gap-2 text-sm mb-2"><Loader2 size={14} class="animate-spin text-blue-400" /> {$t(`backup.phase.${phase}`)} {phase === 'copy' ? `${pct}%` : ''}</div>
            <div class="h-2 bg-slate-700 rounded-full overflow-hidden" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct}><div class="h-full bg-blue-500 transition-all" style="width:{phase === 'copy' ? pct : 100}%"></div></div>
            <button onclick={cancel} class="mt-3 px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs">{$t('common.cancel')}</button>
          </div>
        {:else}
          <button onclick={runNow} disabled={!!problem} class="mt-4 flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-40"><Play size={15} /> {$t('backup.run')}</button>
          {#if problem}
            <div class="mt-2 text-xs text-amber-300">{$t(`backup.problem.${problem}`)} <button class="underline" onclick={() => (tab = 'settings')}>{$t('backup.tab.settings')}</button></div>
          {/if}
        {/if}
      </div>

      <h3 class="text-sm font-semibold mb-2">{$t('backup.snapshots')} <span class="text-slate-500 font-normal">({snapshots.length})</span></h3>
      {#if snapshots.length === 0}
        <div class="text-sm text-slate-500">{$t('backup.none')}</div>
      {:else}
        <div class="grid gap-1.5">
          {#each snapshots as s (s.name)}
            <div class="flex items-center gap-3 p-2.5 rounded-xl bg-slate-800 hover:bg-slate-700/70">
              <HardDrive size={16} class="text-slate-400 shrink-0" />
              <button class="flex-1 min-w-0 text-left" onclick={() => openSnapshot(s)}>
                <div class="text-sm truncate">{fmt(s.name)}</div>
                <div class="text-[11px] text-slate-400">{$t('backup.new_data').replace('{size}', formatBytes(s.transferredBytes))}{s.warnings ? ` · ${$t('backup.warnings').replace('{n}', String(s.warnings))}` : ''}</div>
              </button>
              <button onclick={() => openSnapshot(s)} class="text-xs text-blue-300 hover:text-blue-200 px-2">{$t('backup.open')}</button>
              <button onclick={() => deleteSnap(s)} class="p-1.5 rounded hover:bg-red-500/20 text-slate-400 hover:text-red-300" aria-label={$t('backup.delete.confirm')}><Trash2 size={14} /></button>
            </div>
          {/each}
        </div>
      {/if}

    {:else if tab === 'browse'}
      {#if !selected}
        <div class="text-sm text-slate-500">{$t('backup.browse.pick')}</div>
      {:else}
        <div class="flex items-center gap-1 text-xs mb-3 flex-wrap">
          <span class="text-slate-400">{fmt(selected.name)}</span><ChevronRight size={12} class="text-slate-600" />
          <button class="hover:underline" onclick={() => jump('')}>/</button>
          {#each breadcrumbs(rel) as b}<ChevronRight size={12} class="text-slate-600" /><button class="hover:underline" onclick={() => jump(b.path)}>{b.label}</button>{/each}
        </div>
        {#if rel}<button onclick={() => jump(parentRel(rel))} class="text-xs text-blue-300 mb-2">← {$t('backup.up')}</button>{/if}
        {#if browseLoading}
          <Loader2 size={18} class="animate-spin text-slate-400" />
        {:else}
          <div class="grid gap-1">
            {#each entries as e (e.name)}
              <div class="flex items-center gap-2 p-2 rounded-lg hover:bg-slate-800 group">
                <button class="flex items-center gap-2 flex-1 min-w-0 text-left" onclick={() => enter(e)} disabled={!e.isDir}>
                  {#if e.isDir}<Folder size={15} class="text-blue-400 shrink-0" />{:else}<FileIcon size={15} class="text-slate-400 shrink-0" />{/if}
                  <span class="text-sm truncate">{e.name}</span>
                  {#if !e.isDir}<span class="text-[11px] text-slate-500 ml-auto shrink-0">{formatBytes(e.size)}</span>{/if}
                </button>
                <button onclick={() => restoreEntry(e, false)} class="opacity-0 group-hover:opacity-100 focus:opacity-100 flex items-center gap-1 text-xs px-2 py-1 rounded bg-slate-700 hover:bg-slate-600" title={$t('backup.restore.copy_hint')}><ArchiveRestore size={12} /> {$t('backup.restore.copy')}</button>
                <button onclick={() => restoreEntry(e, true)} class="opacity-0 group-hover:opacity-100 focus:opacity-100 flex items-center gap-1 text-xs px-2 py-1 rounded bg-amber-600/80 hover:bg-amber-500" title={$t('backup.restore.inplace_hint')}><RotateCcw size={12} /> {$t('backup.restore.inplace')}</button>
              </div>
            {:else}
              <div class="text-sm text-slate-500">{$t('backup.empty_dir')}</div>
            {/each}
          </div>
        {/if}
      {/if}

    {:else}
      <div class="grid gap-5 max-w-xl">
        <section>
          <h3 class="text-sm font-semibold mb-1">{$t('backup.cfg.destination')}</h3>
          <p class="text-xs text-slate-400 mb-2">{$t('backup.cfg.destination_hint')}</p>
          <div class="flex gap-2"><input readonly value={cfg.destination} placeholder="/run/media/…" class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm" /><button onclick={pickDestination} class="px-3 py-2 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm">{$t('backup.choose')}</button></div>
        </section>

        <section>
          <h3 class="text-sm font-semibold mb-1">{$t('backup.cfg.sources')}</h3>
          <div class="grid gap-1 mb-2">
            {#each cfg.sources as s}
              <div class="flex items-center gap-2 bg-slate-800 rounded-lg px-3 py-1.5 text-sm"><Folder size={14} class="text-blue-400" /><span class="flex-1 truncate">{s}</span><button onclick={() => (cfg = { ...cfg, sources: toggleSource(cfg.sources, s) })} aria-label={$t('common.remove')} class="text-slate-400 hover:text-red-300"><X size={14} /></button></div>
            {/each}
          </div>
          <button onclick={addSource} class="flex items-center gap-1.5 text-xs px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg"><Plus size={13} /> {$t('backup.cfg.add_folder')}</button>
        </section>

        <section>
          <h3 class="text-sm font-semibold mb-1">{$t('backup.cfg.excludes')}</h3>
          <p class="text-xs text-slate-400 mb-2">{$t('backup.cfg.excludes_hint')}</p>
          <div class="flex flex-wrap gap-1.5 mb-2">
            {#each cfg.excludes as x}<span class="flex items-center gap-1 bg-slate-800 rounded-full pl-2.5 pr-1.5 py-0.5 text-xs">{x}<button onclick={() => (cfg = { ...cfg, excludes: cfg.excludes.filter((e) => e !== x) })} aria-label={$t('common.remove')}><X size={11} /></button></span>{/each}
          </div>
          <form onsubmit={preventDefault(addExclude)} class="flex gap-2"><input bind:value={newExclude} placeholder="*.iso" class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-sm outline-none focus:border-blue-500" /><button class="px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm">{$t('backup.cfg.add')}</button></form>
        </section>

        <section>
          <h3 class="text-sm font-semibold mb-1">{$t('backup.cfg.schedule')}</h3>
          <select bind:value={cfg.schedule} class="bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm">
            {#each ['off', 'hourly', 'daily', 'weekly'] as s}<option value={s}>{$t(`backup.schedule.${s}`)}</option>{/each}
          </select>
          <p class="text-xs text-slate-500 mt-1">{$t('backup.cfg.schedule_hint')}</p>
        </section>

        <section>
          <h3 class="text-sm font-semibold mb-1">{$t('backup.cfg.retention')}</h3>
          <p class="text-xs text-slate-400 mb-2">{$t('backup.cfg.retention_hint')}</p>
          <div class="grid grid-cols-3 gap-2 text-xs">
            <label class="grid gap-1">{$t('backup.cfg.daily')}<input type="number" min="0" max="365" bind:value={cfg.keepDaily} class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-sm" /></label>
            <label class="grid gap-1">{$t('backup.cfg.weekly')}<input type="number" min="0" max="260" bind:value={cfg.keepWeekly} class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-sm" /></label>
            <label class="grid gap-1">{$t('backup.cfg.monthly')}<input type="number" min="0" max="120" bind:value={cfg.keepMonthly} class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-sm" /></label>
          </div>
        </section>

        <div class="flex items-center gap-3">
          <button onclick={saveConfig} disabled={!!problem} class="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-40"><Settings2 size={15} /> {$t('common.save')}</button>
          {#if saved}<span class="text-xs text-green-300 flex items-center gap-1"><CheckCircle2 size={13} /> {$t('backup.saved')}</span>{/if}
          {#if problem}<span class="text-xs text-amber-300">{$t(`backup.problem.${problem}`)}</span>{/if}
        </div>
      </div>
    {/if}
  </div>
</div>
