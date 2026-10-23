<script lang="ts">
  import { Plus, Trash2, AlertTriangle } from 'lucide-svelte';
  import type { PartitionPlanEntry } from '../types';

  interface Props {
    partitions: PartitionPlanEntry[];
    diskSizeBytes: number;
    validate: (plan: PartitionPlanEntry[]) => string | null;
    onChange: (plan: PartitionPlanEntry[]) => void;
  }

  let {
    partitions,
    diskSizeBytes,
    validate,
    onChange
  }: Props = $props();

  const FS_OPTIONS: PartitionPlanEntry['filesystem'][] = ['ext4', 'btrfs', 'xfs', 'fat32', 'swap'];
  const ROLE_OPTIONS: PartitionPlanEntry['role'][] = ['esp', 'root', 'home', 'swap', 'other'];

  let error = $derived(validate(partitions));
  let usedMiB = $derived(partitions.reduce((sum, p) => sum + (p.sizeMiB ?? 0), 0));
  let diskMiB = $derived(Math.floor(diskSizeBytes / (1024 * 1024)));
  let remainingMiB = $derived(Math.max(0, diskMiB - usedMiB));

  function update(id: string, patch: Partial<PartitionPlanEntry>) {
    onChange(partitions.map((p) => (p.id === id ? { ...p, ...patch } : p)));
  }
  // Inline `as` casts don't parse inside Svelte markup expressions (they're
  // parsed as plain JS, not TS), so the casting happens here instead.
  function onRoleChange(id: string, e: Event) {
    update(id, { role: (e.currentTarget as HTMLSelectElement).value as PartitionPlanEntry['role'] });
  }
  function onFilesystemChange(id: string, e: Event) {
    update(id, { filesystem: (e.currentTarget as HTMLSelectElement).value as PartitionPlanEntry['filesystem'] });
  }
  function addPartition() {
    onChange([...partitions, {
      id: `p-${Date.now()}`, role: 'other', filesystem: 'ext4', mountpoint: '', sizeMiB: 1024,
    }]);
  }
  function removePartition(id: string) {
    onChange(partitions.filter((p) => p.id !== id));
  }
</script>

<div class="mt-4 max-w-2xl bg-slate-800/50 border border-white/5 rounded-xl p-4 space-y-3">
  <div class="flex items-center justify-between">
    <span class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Partition layout</span>
    <span class="text-[11px] text-slate-500">{remainingMiB.toLocaleString()} MiB unallocated of {diskMiB.toLocaleString()} MiB</span>
  </div>

  <div class="space-y-1.5">
    {#each partitions as p (p.id)}
      <div class="grid grid-cols-[70px_60px_1fr_100px_28px] gap-1.5 items-center bg-slate-900/60 rounded-lg p-1.5">
        <select value={p.role} onchange={(e) => onRoleChange(p.id, e)}
          class="bg-slate-800 border border-white/10 rounded px-1.5 py-1 text-[11px]">
          {#each ROLE_OPTIONS as r}<option value={r}>{r}</option>{/each}
        </select>
        <select value={p.filesystem} onchange={(e) => onFilesystemChange(p.id, e)}
          class="bg-slate-800 border border-white/10 rounded px-1.5 py-1 text-[11px]">
          {#each FS_OPTIONS as fs}<option value={fs}>{fs}</option>{/each}
        </select>
        <input value={p.mountpoint} oninput={(e) => update(p.id, { mountpoint: e.currentTarget.value })}
          placeholder="/mountpoint" class="bg-slate-800 border border-white/10 rounded px-1.5 py-1 text-[11px] font-mono" />
        {#if p.sizeMiB === null}
          <button onclick={() => update(p.id, { sizeMiB: 1024 })} class="text-[11px] text-blue-400 border border-blue-500/30 rounded px-1.5 py-1">Rest of disk</button>
        {:else}
          <input type="number" min="32" value={p.sizeMiB} oninput={(e) => update(p.id, { sizeMiB: parseInt(e.currentTarget.value) || 0 })}
            class="bg-slate-800 border border-white/10 rounded px-1.5 py-1 text-[11px] w-full" />
        {/if}
        <button onclick={() => removePartition(p.id)} class="p-1 hover:bg-red-500/20 rounded text-red-400 justify-self-center"><Trash2 size={12} /></button>
      </div>
    {/each}
  </div>

  <div class="flex items-center justify-between pt-1">
    <button onclick={addPartition} class="flex items-center gap-1 text-xs text-blue-400 hover:text-blue-300"><Plus size={13} /> Add partition</button>
    <button onclick={() => update(partitions.find((p) => p.mountpoint === '/')?.id ?? partitions[partitions.length - 1].id, { sizeMiB: null })}
      class="text-[11px] text-slate-500 hover:text-slate-300">Set root to "rest of disk"</button>
  </div>

  {#if error}
    <div class="flex items-center gap-1.5 text-[11px] text-amber-400"><AlertTriangle size={12} /> {error}</div>
  {/if}
</div>
