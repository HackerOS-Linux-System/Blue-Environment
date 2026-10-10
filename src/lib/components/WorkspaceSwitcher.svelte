<script lang="ts">
  import { afterUpdate } from 'svelte';

  export let currentWorkspace: number;
  export let workspaceCount: number;
  export let windowCounts: number[] = [];

  let visible = false;
  let prevWorkspace = currentWorkspace;
  let hideTimer: ReturnType<typeof setTimeout>;

  $: if (currentWorkspace !== prevWorkspace) {
    prevWorkspace = currentWorkspace;
    visible = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (visible = false), 1400);
  }
</script>

{#if visible}
  <div data-shell-occluder="workspace-switcher" class="absolute bottom-16 left-1/2 -translate-x-1/2 z-[300] pointer-events-none animate-fade-in">
    <div class="bg-slate-900/95 border border-white/10 rounded-2xl px-6 py-4 shadow-2xl backdrop-blur-xl flex flex-col items-center gap-3">
      <span class="text-xs text-slate-400 font-medium uppercase tracking-widest">Workspace</span>
      <div class="flex gap-3 items-center">
        {#each Array.from({ length: workspaceCount }, (_, i) => i) as i (i)}
          <div class="flex flex-col items-center gap-1.5">
            <div class="w-14 h-10 rounded-lg border-2 transition-all flex items-end justify-center pb-1 gap-0.5
              {i === currentWorkspace ? 'border-blue-500 bg-blue-600/20 shadow-[0_0_12px_rgba(59,130,246,0.4)]' : 'border-white/10 bg-white/5'}">
              {#each Array.from({ length: Math.min(windowCounts[i] ?? 0, 3) }, (_, wi) => wi) as wi (wi)}
                <div class="h-3 w-3 rounded-sm {i === currentWorkspace ? 'bg-blue-400/60' : 'bg-slate-600'}" />
              {/each}
            </div>
            <span class="text-[11px] font-bold {i === currentWorkspace ? 'text-blue-400' : 'text-slate-500'}">{i + 1}</span>
          </div>
        {/each}
      </div>
      <div class="flex items-center gap-2 text-xs text-slate-500">
        <kbd class="px-1.5 py-0.5 bg-slate-800 rounded border border-white/10 text-slate-400">Super</kbd>
        <span>+</span>
        <kbd class="px-1.5 py-0.5 bg-slate-800 rounded border border-white/10 text-slate-400">1–4</kbd>
      </div>
    </div>
  </div>
{/if}
