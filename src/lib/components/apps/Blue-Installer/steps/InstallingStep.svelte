<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Loader2 } from 'lucide-svelte';
  import type { InstallState } from '../installState';
  import { tauriListen } from '../../terminal/tauriBridge';

  export let state: InstallState;
  const { progressPct, progressLabel, installLog } = state;

  let unlisten: (() => void) | undefined;

  onMount(async () => {
    unlisten = await tauriListen('installer-progress', (payload: { pct: number; label: string; line?: string }) => {
      progressPct.set(payload.pct);
      progressLabel.set(payload.label);
      if (payload.line) installLog.update((l) => [...l.slice(-200), payload.line!]);
      if (payload.pct >= 100) state.next();
    });
  });
  onDestroy(() => unlisten?.());
</script>

<div class="flex-1 flex flex-col items-center justify-center px-12 gap-6">
  <Loader2 size={40} class="text-blue-400 animate-spin" />
  <div class="text-center">
    <h2 class="text-xl font-semibold text-white mb-1">Installing Blue Environment…</h2>
    <p class="text-slate-500 text-sm">{$progressLabel || 'Preparing…'}</p>
  </div>
  <div class="w-full max-w-md h-2 bg-slate-800 rounded-full overflow-hidden">
    <div class="h-full bg-blue-500 transition-all duration-300" style="width:{$progressPct}%;" />
  </div>
  <span class="text-xs text-slate-600 font-mono">{$progressPct}%</span>

  {#if $installLog.length > 0}
    <div class="w-full max-w-md h-32 overflow-y-auto bg-slate-950 rounded-xl border border-white/5 p-3 font-mono text-[10px] text-slate-500">
      {#each $installLog as line, i (i)}<div>{line}</div>{/each}
    </div>
  {/if}

  <p class="text-xs text-slate-700 max-w-sm text-center">Do not turn off your computer during this process.</p>
</div>
