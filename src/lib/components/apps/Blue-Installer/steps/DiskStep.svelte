<script lang="ts">
  import { onMount } from 'svelte';
  import { HardDrive, AlertTriangle, Loader2, RefreshCw } from 'lucide-svelte';
  import type { InstallState } from '../installState';
  import { defaultPartitionPlan } from '../installState';
  import type { PartitionPlanEntry } from '../types';
  import PartitionEditor from './PartitionEditor.svelte';

  export let state: InstallState;
  const { config, disks, disksLoading, validatePartitionPlan } = state;

  onMount(() => state.loadDisks());

  function selectMode(mode: 'erase' | 'manual') {
    config.update((c) => ({
      ...c,
      diskMode: mode,
      partitions: mode === 'manual' && c.partitions.length === 0 ? defaultPartitionPlan() : c.partitions,
    }));
  }

  // Named function instead of an inline typed arrow function in markup —
  // Svelte parses template expressions as plain JS (acorn), so a parameter
  // type annotation here (`(plan: PartitionPlanEntry[]) => ...`) is a parse
  // error even inside a `<script lang="ts">` component. Same root cause as
  // the `as` casts fixed earlier in PartitionEditor.svelte.
  function handlePartitionsChange(plan: PartitionPlanEntry[]) {
    config.update((c) => ({ ...c, partitions: plan }));
  }
</script>

<div class="flex-1 flex flex-col px-10 py-8 overflow-y-auto">
  <div class="flex items-center justify-between mb-2">
    <div class="flex items-center gap-2"><HardDrive size={20} class="text-blue-400" /><h2 class="text-xl font-semibold text-white">Select installation disk</h2></div>
    <button on:click={state.loadDisks} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-500"><RefreshCw size={14} class={$disksLoading ? 'animate-spin' : ''} /></button>
  </div>
  <p class="text-sm text-slate-500 mb-4 max-w-xl">The selected disk will be <strong class="text-red-400">completely erased</strong>. Make sure you've backed up anything important.</p>

  {#if $disksLoading}
    <div class="flex items-center gap-2 text-slate-500 text-sm"><Loader2 size={16} class="animate-spin" /> Scanning disks…</div>
  {:else if $disks.length === 0}
    <div class="text-slate-600 text-sm">No disks found.</div>
  {:else}
    <div class="space-y-2 max-w-xl">
      {#each $disks as d (d.path)}
        <button on:click={() => config.update((c) => ({ ...c, disk: d }))}
          class="w-full flex items-center gap-4 p-4 rounded-xl border transition-colors {$config.disk?.path === d.path ? 'bg-blue-600/15 border-blue-500/40' : 'bg-slate-800/50 border-white/5 hover:bg-white/5'}">
          <HardDrive size={22} class={$config.disk?.path === d.path ? 'text-blue-400' : 'text-slate-500'} />
          <div class="flex-1 text-left">
            <div class="text-white text-sm font-medium">{d.model}</div>
            <div class="text-slate-500 text-xs font-mono">{d.path} · {d.sizeLabel}{d.removable ? ' · removable' : ''}</div>
          </div>
        </button>
      {/each}
    </div>
  {/if}

  {#if $config.disk}
    <div class="mt-6 max-w-xl flex items-start gap-2 bg-red-500/5 border border-red-500/20 rounded-xl p-3 text-sm text-red-300">
      <AlertTriangle size={16} class="shrink-0 mt-0.5" />
      <span>Everything on <strong class="font-mono">{$config.disk.path}</strong> ({$config.disk.sizeLabel}) will be erased and replaced.</span>
    </div>
  {/if}

  <div class="mt-6 max-w-2xl">
    <div class="text-xs text-slate-500 mb-2">Partitioning mode</div>
    <div class="flex gap-2">
      <button on:click={() => selectMode('erase')}
        class="flex-1 px-3 py-2 rounded-lg text-xs font-medium transition-colors {$config.diskMode === 'erase' ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}">
        Erase disk (recommended)
      </button>
      <button on:click={() => selectMode('manual')}
        class="flex-1 px-3 py-2 rounded-lg text-xs font-medium transition-colors {$config.diskMode === 'manual' ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}">
        Manual partitioning
      </button>
    </div>

    {#if $config.diskMode === 'manual' && $config.disk}
      <PartitionEditor
        partitions={$config.partitions}
        diskSizeBytes={$config.disk.sizeBytes}
        validate={validatePartitionPlan}
        onChange={handlePartitionsChange}
      />
    {:else if $config.diskMode === 'manual'}
      <p class="text-xs text-slate-500 mt-3">Select a disk above to edit its partition layout.</p>
    {/if}
  </div>
</div>
