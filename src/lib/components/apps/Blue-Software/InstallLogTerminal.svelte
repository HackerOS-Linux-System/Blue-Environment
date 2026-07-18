<script lang="ts">
  import { Terminal, Check, AlertCircle, Loader2, X } from 'lucide-svelte';
  import type { InstallLog } from './types';
  import { createEventDispatcher } from 'svelte';

  export let log: InstallLog;
  const dispatch = createEventDispatcher<{ close: void }>();
</script>

<div class="border-t border-white/5 bg-slate-950 shrink-0" style="max-height:160px;">
  <div class="flex items-center justify-between px-3 py-1.5 border-b border-white/5">
    <span class="text-xs text-slate-400 flex items-center gap-1.5">
      <Terminal size={12} />
      {#if log.done}
        {#if log.success}
          <span class="text-green-400 flex items-center gap-1"><Check size={11} /> Done</span>
        {:else}
          <span class="text-red-400 flex items-center gap-1"><AlertCircle size={11} /> Failed</span>
        {/if}
      {:else}
        <span class="flex items-center gap-1"><Loader2 size={11} class="animate-spin" /> Working…</span>
      {/if}
    </span>
    {#if log.done}
      <button on:click={() => dispatch('close')} class="text-slate-500 hover:text-white"><X size={12} /></button>
    {/if}
  </div>
  <div class="overflow-y-auto p-3 font-mono text-xs text-slate-300 space-y-0.5" style="max-height:110px;">
    {#each log.lines as line, i (i)}
      <div>{line}</div>
    {/each}
  </div>
</div>
