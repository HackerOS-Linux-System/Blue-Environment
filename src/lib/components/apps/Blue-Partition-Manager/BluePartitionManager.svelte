<script lang="ts">
  import { onMount } from 'svelte';
  import {
    HardDrive, RefreshCw, Disc3, Play, Square, AlertTriangle, Loader2,
    CheckCircle2, XCircle, Tag, Eraser, ChevronRight, Activity, Thermometer, Zap,
  } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { dialogConfirm, dialogPrompt } from '../../../stores/dialog';
  import type { BpmDevice, SmartStatus, BenchmarkResult } from './types';
  import { FS_OPTIONS, formatBytes } from './types';

  let disks: BpmDevice[] = [];
  let loading = false;
  let error = '';
  let selected: BpmDevice | null = null;
  let busyPath: string | null = null;

  interface Notif { id: string; type: 'success' | 'error' | 'info'; message: string; }
  let notifs: Notif[] = [];
  function notify(type: Notif['type'], message: string) {
    const id = Date.now().toString() + Math.random().toString(36).slice(2);
    notifs = [...notifs, { id, type, message }];
    setTimeout(() => (notifs = notifs.filter((n) => n.id !== id)), 4000);
  }

  async function refresh() {
    loading = true; error = '';
    try {
      disks = await SystemBridge.invokeCommand<BpmDevice[]>('bpm_list_devices');
      if (selected) {
        const flat = flatten(disks);
        selected = flat.find((d) => d.path === selected?.path) ?? null;
      }
    } catch (e: any) {
      error = e?.message ?? String(e);
      disks = [];
    } finally {
      loading = false;
    }
  }

  function flatten(list: BpmDevice[]): BpmDevice[] {
    return list.flatMap((d) => [d, ...flatten(d.children ?? [])]);
  }

  onMount(refresh);

  function select(d: BpmDevice) { selected = d; }

  async function mount(d: BpmDevice) {
    busyPath = d.path;
    try {
      await SystemBridge.invokeCommand('bpm_mount', { device: d.path });
      notify('success', `Mounted ${d.path}`);
      await refresh();
    } catch (e: any) {
      notify('error', e?.message ?? `Failed to mount ${d.path}`);
    } finally { busyPath = null; }
  }

  async function unmount(d: BpmDevice) {
    busyPath = d.path;
    try {
      await SystemBridge.invokeCommand('bpm_unmount', { device: d.path });
      notify('success', `Unmounted ${d.path}`);
      await refresh();
    } catch (e: any) {
      notify('error', e?.message ?? `Failed to unmount ${d.path}`);
    } finally { busyPath = null; }
  }

  let showFormatDialog = false;
  let formatFs: string = 'ext4';
  let formatLabel = '';

  function openFormatDialog(d: BpmDevice) {
    selected = d;
    formatFs = d.fstype && (FS_OPTIONS as readonly string[]).includes(d.fstype) ? d.fstype : 'ext4';
    formatLabel = d.label ?? '';
    showFormatDialog = true;
  }

  async function confirmFormat() {
    if (!selected) return;
    const target = selected;
    const ok = await dialogConfirm({
      title: 'Format partition',
      message: `This will PERMANENTLY ERASE all data on ${target.path} and create a new ${formatFs} filesystem. This cannot be undone. Continue?`,
      confirmLabel: 'Format',
      danger: true,
    });
    if (!ok) return;
    showFormatDialog = false;
    busyPath = target.path;
    try {
      await SystemBridge.invokeCommand('bpm_format', { device: target.path, fstype: formatFs, label: formatLabel || null });
      notify('success', `Formatted ${target.path} as ${formatFs}`);
      await refresh();
    } catch (e: any) {
      notify('error', e?.message ?? `Failed to format ${target.path}`);
    } finally { busyPath = null; }
  }

  async function renameLabel(d: BpmDevice) {
    if (!d.fstype) { notify('error', 'Partition has no filesystem to label'); return; }
    const name = await dialogPrompt({ title: 'Rename partition', label: 'New label', placeholder: d.label ?? '', defaultValue: d.label ?? '', confirmLabel: 'Rename' });
    if (name === null) return;
    busyPath = d.path;
    try {
      await SystemBridge.invokeCommand('bpm_set_label', { device: d.path, fstype: d.fstype, label: name });
      notify('success', `Renamed ${d.path}`);
      await refresh();
    } catch (e: any) {
      notify('error', e?.message ?? `Failed to rename ${d.path}`);
    } finally { busyPath = null; }
  }

  function usedFraction(d: BpmDevice, total: number): number {
    return total > 0 ? Math.min(1, d.size_bytes / total) : 0;
  }

  // ── Disk health (SMART) + benchmark ──────────────────────────────────
  // KDE Partition Manager has neither of these built in — you'd need a
  // separate tool. Expanded inline per-disk so you don't lose your place
  // in the device list.
  let healthOpenFor: string | null = null;
  let healthData: Record<string, SmartStatus> = {};
  let healthLoading: string | null = null;
  let benchmarkData: Record<string, BenchmarkResult> = {};
  let benchmarkRunning: string | null = null;

  async function toggleHealth(d: BpmDevice) {
    if (healthOpenFor === d.path) { healthOpenFor = null; return; }
    healthOpenFor = d.path;
    if (!healthData[d.path]) {
      healthLoading = d.path;
      try {
        healthData[d.path] = await SystemBridge.invokeCommand<SmartStatus>('bpm_smart_status', { device: d.path });
      } catch (e: any) {
        healthData[d.path] = {
          available: false, healthy: null, temperature_celsius: null, power_on_hours: null,
          power_cycle_count: null, attributes: [], model: null, serial: null,
          error: e?.message ?? String(e),
        };
      } finally {
        healthLoading = null;
      }
    }
  }

  async function runBenchmark(d: BpmDevice) {
    benchmarkRunning = d.path;
    try {
      benchmarkData[d.path] = await SystemBridge.invokeCommand<BenchmarkResult>('bpm_benchmark_read', { device: d.path });
    } catch (e: any) {
      notify('error', `Benchmark failed: ${e?.message ?? e}`);
    } finally {
      benchmarkRunning = null;
    }
  }
</script>

<div class="flex h-full bg-slate-900 text-white overflow-hidden">
  <!-- Notifications -->
  <div class="fixed top-16 right-4 z-50 flex flex-col gap-2 pointer-events-none">
    {#each notifs as n (n.id)}
      <div class="flex items-center gap-2 px-4 py-2 rounded-xl shadow-lg text-sm {n.type === 'success' ? 'bg-green-600/90' : n.type === 'error' ? 'bg-red-600/90' : 'bg-slate-700/90'}">
        {#if n.type === 'success'}<CheckCircle2 size={13} />{:else if n.type === 'error'}<XCircle size={13} />{/if}
        {n.message}
      </div>
    {/each}
  </div>

  <div class="flex-1 flex flex-col min-w-0">
    <!-- Toolbar -->
    <div class="h-12 border-b border-white/5 bg-slate-800/60 flex items-center px-3 gap-2 shrink-0">
      <HardDrive size={16} class="text-blue-400" />
      <span class="text-sm font-semibold">Blue Partition Manager</span>
      <button on:click={refresh} class="ml-auto p-1.5 hover:bg-white/10 rounded" title="Refresh">
        <RefreshCw size={14} class={loading ? 'animate-spin' : ''} />
      </button>
    </div>

    <div class="flex-1 overflow-auto p-3 space-y-3">
      {#if error}
        <div class="flex items-center gap-2 text-sm text-red-400 bg-red-500/10 border border-red-500/20 rounded-xl p-3">
          <AlertTriangle size={14} class="shrink-0" /> {error}
        </div>
      {/if}

      {#if loading && disks.length === 0}
        <div class="flex items-center justify-center h-40 text-slate-500"><Loader2 size={22} class="animate-spin" /></div>
      {:else if disks.length === 0 && !error}
        <div class="flex flex-col items-center justify-center h-40 text-slate-600 gap-2">
          <Disc3 size={32} /><span class="text-sm">No block devices found</span>
        </div>
      {/if}

      {#each disks as disk (disk.path)}
        <div class="bg-slate-800/60 border border-white/5 rounded-2xl overflow-hidden">
          <button on:click={() => select(disk)}
            class="w-full flex items-center gap-3 px-4 py-3 hover:bg-white/5 transition-colors text-left {selected?.path === disk.path ? 'bg-blue-600/10' : ''}">
            <HardDrive size={18} class="text-blue-400 shrink-0" />
            <div class="min-w-0 flex-1">
              <div class="text-sm font-medium truncate">{disk.model || disk.name} <span class="text-slate-500 font-normal">({disk.path})</span></div>
              <div class="text-xs text-slate-500">{formatBytes(disk.size_bytes)} · {disk.children?.length ?? 0} partition(s){disk.removable ? ' · removable' : ''}</div>
            </div>
            <span role="button" tabindex="0" on:click|stopPropagation={() => toggleHealth(disk)}
              class="flex items-center gap-1 px-2 py-1 rounded-lg text-xs shrink-0 {healthOpenFor === disk.path ? 'bg-blue-600/20 text-blue-400' : 'text-slate-400 hover:bg-white/10'}">
              <Activity size={12} /> Health
            </span>
          </button>

          {#if healthOpenFor === disk.path}
            <div class="mx-4 mb-3 bg-slate-900/60 border border-white/5 rounded-xl p-3 text-sm">
              {#if healthLoading === disk.path}
                <div class="flex items-center gap-2 text-slate-400"><Loader2 size={14} class="animate-spin" /> Reading SMART data…</div>
              {:else if healthData[disk.path]}
                {@const h = healthData[disk.path]}
                {#if !h.available}
                  <div class="flex items-center gap-2 text-slate-500 text-xs"><AlertTriangle size={13} /> {h.error}</div>
                {:else}
                  <div class="flex flex-wrap items-center gap-4 mb-2">
                    <div class="flex items-center gap-1.5">
                      {#if h.healthy === true}<CheckCircle2 size={14} class="text-green-400" /> <span class="text-green-400">Healthy</span>
                      {:else if h.healthy === false}<XCircle size={14} class="text-red-400" /> <span class="text-red-400">Failing — back up now</span>
                      {:else}<span class="text-slate-500">Health status unknown</span>{/if}
                    </div>
                    {#if h.temperature_celsius !== null}
                      <div class="flex items-center gap-1 text-slate-400"><Thermometer size={12} /> {h.temperature_celsius}°C</div>
                    {/if}
                    {#if h.power_on_hours !== null}
                      <div class="text-slate-400">{h.power_on_hours} hrs on</div>
                    {/if}
                    {#if h.power_cycle_count !== null}
                      <div class="text-slate-400">{h.power_cycle_count} power cycles</div>
                    {/if}
                  </div>
                  {#if h.attributes.length > 0}
                    <div class="max-h-32 overflow-y-auto text-xs space-y-0.5 mb-2">
                      {#each h.attributes.filter((a) => a.failing || ['Reallocated Sector Ct', 'Reallocated Sectors Count', 'Current Pending Sector'].some((n) => a.name.includes(n))) as a (a.id)}
                        <div class="flex justify-between {a.failing ? 'text-red-400' : 'text-slate-400'}">
                          <span>{a.name}</span><span class="font-mono">{a.raw}</span>
                        </div>
                      {/each}
                    </div>
                  {/if}
                {/if}
              {/if}
              <div class="flex items-center gap-2 pt-2 border-t border-white/5 mt-2">
                <button on:click={() => runBenchmark(disk)} disabled={benchmarkRunning === disk.path}
                  class="flex items-center gap-1.5 px-2.5 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs text-slate-300 disabled:opacity-50">
                  {#if benchmarkRunning === disk.path}<Loader2 size={12} class="animate-spin" /> Testing…{:else}<Zap size={12} /> Benchmark read speed{/if}
                </button>
                {#if benchmarkData[disk.path]}
                  <span class="text-xs text-blue-400 font-mono">{benchmarkData[disk.path].read_mb_per_sec.toFixed(0)} MB/s</span>
                {/if}
              </div>
            </div>
          {/if}

          {#if disk.children?.length}
            <!-- Visual usage bar -->
            <div class="px-4 pb-2 flex h-2 gap-0.5">
              {#each disk.children as p (p.path)}
                <div class="h-full rounded-full bg-blue-500/60 first:rounded-l-full last:rounded-r-full"
                  style="width:{Math.max(2, usedFraction(p, disk.size_bytes) * 100)}%"
                  title="{p.path}: {formatBytes(p.size_bytes)}" />
              {/each}
            </div>

            <div class="divide-y divide-white/5 border-t border-white/5">
              {#each disk.children as p (p.path)}
                <div class="flex items-center gap-3 px-4 py-2.5 hover:bg-white/5 {selected?.path === p.path ? 'bg-blue-600/10' : ''}">
                  <ChevronRight size={12} class="text-slate-600 shrink-0" />
                  <div class="min-w-0 flex-1 cursor-pointer" on:click={() => select(p)}>
                    <div class="text-sm truncate flex items-center gap-2">
                      <span class="font-mono text-slate-300">{p.path}</span>
                      {#if p.label}<span class="text-slate-500">"{p.label}"</span>{/if}
                      {#if p.fstype}<span class="text-[10px] bg-slate-700 px-1.5 py-0.5 rounded uppercase text-slate-400">{p.fstype}</span>{/if}
                    </div>
                    <div class="text-xs text-slate-500">
                      {formatBytes(p.size_bytes)}
                      {#if p.mountpoint}<span class="text-green-400 ml-1.5">mounted at {p.mountpoint}</span>{:else}<span class="text-slate-600 ml-1.5">not mounted</span>{/if}
                    </div>
                  </div>

                  <div class="flex items-center gap-1 shrink-0">
                    {#if busyPath === p.path}
                      <Loader2 size={14} class="animate-spin text-blue-400 mr-1" />
                    {:else}
                      {#if p.mountpoint}
                        <button on:click={() => unmount(p)} title="Unmount" class="p-1.5 hover:bg-white/10 rounded text-slate-400 hover:text-yellow-400"><Square size={13} /></button>
                      {:else}
                        <button on:click={() => mount(p)} title="Mount" class="p-1.5 hover:bg-white/10 rounded text-slate-400 hover:text-green-400"><Play size={13} /></button>
                      {/if}
                      <button on:click={() => renameLabel(p)} title="Rename label" class="p-1.5 hover:bg-white/10 rounded text-slate-400 hover:text-blue-400"><Tag size={13} /></button>
                      <button on:click={() => openFormatDialog(p)} title="Format" class="p-1.5 hover:bg-red-500/20 rounded text-slate-400 hover:text-red-400"><Eraser size={13} /></button>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <div class="px-4 pb-3 text-xs text-slate-600">No partitions detected on this device.</div>
          {/if}
        </div>
      {/each}
    </div>
  </div>

  {#if showFormatDialog && selected}
    <div class="fixed inset-0 z-[70] bg-black/50 flex items-center justify-center" on:click={() => (showFormatDialog = false)}>
      <div class="w-96 bg-slate-800 border border-white/10 rounded-2xl shadow-2xl p-5" on:click|stopPropagation>
        <div class="flex items-center gap-2 mb-3 text-red-400">
          <AlertTriangle size={16} />
          <h3 class="font-semibold text-base text-white">Format {selected.path}</h3>
        </div>
        <p class="text-xs text-slate-400 mb-4">All data on this partition will be permanently erased.</p>

        <label class="block text-xs text-slate-500 mb-1.5">Filesystem</label>
        <select bind:value={formatFs} class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white mb-4 focus:outline-none">
          {#each FS_OPTIONS as fs (fs)}<option value={fs}>{fs}</option>{/each}
        </select>

        <label class="block text-xs text-slate-500 mb-1.5">Label (optional)</label>
        <input bind:value={formatLabel} placeholder="e.g. DATA"
          class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white mb-5 focus:outline-none focus:border-blue-500/50" />

        <div class="flex justify-end gap-2">
          <button on:click={() => (showFormatDialog = false)} class="px-3.5 py-1.5 text-sm bg-slate-700 hover:bg-slate-600 rounded-lg text-slate-200">Cancel</button>
          <button on:click={confirmFormat} class="px-3.5 py-1.5 text-sm bg-red-600 hover:bg-red-500 rounded-lg text-white">Format</button>
        </div>
      </div>
    </div>
  {/if}
</div>
