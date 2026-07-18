<script lang="ts">
  import type { CpuInfo } from './types';
  import Bar from './Bar.svelte';

  export let cpu: CpuInfo | null;

  function coreColor(u: number) { return u > 90 ? '#ef4444' : u > 70 ? '#f59e0b' : '#3b82f6'; }
</script>

{#if cpu}
  <div class="grid gap-2" style="grid-template-columns:repeat(auto-fill, minmax(80px, 1fr));">
    {#each cpu.cores as core (core.id)}
      <div class="bg-slate-800/60 rounded-xl p-2.5 border border-white/5 text-center">
        <div class="text-xs text-slate-500 mb-1.5">C{core.id}</div>
        <div class="text-sm font-medium tabular-nums" style="color:{coreColor(core.usage)};">{core.usage.toFixed(0)}%</div>
        {#if core.freq}<div class="text-[10px] text-slate-700 mt-0.5">{core.freq} MHz</div>{/if}
        <div class="mt-1.5"><Bar value={core.usage} color={coreColor(core.usage)} height={3} /></div>
      </div>
    {/each}
  </div>
{/if}
