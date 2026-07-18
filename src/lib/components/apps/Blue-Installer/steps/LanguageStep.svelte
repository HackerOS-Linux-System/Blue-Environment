<script lang="ts">
  import { Check, Globe } from 'lucide-svelte';
  import { LOCALES } from '../types';
  import type { InstallState } from '../installState';

  export let state: InstallState;
  const { config } = state;
</script>

<div class="flex-1 flex flex-col px-10 py-8 overflow-y-auto">
  <div class="flex items-center gap-2 mb-6"><Globe size={20} class="text-blue-400" /><h2 class="text-xl font-semibold text-white">Choose your language</h2></div>
  <div class="grid grid-cols-2 gap-2 max-w-xl">
    {#each LOCALES as l (l.code)}
      <button on:click={() => config.update((c) => ({ ...c, locale: l.code }))}
        class="flex items-center justify-between px-4 py-3 rounded-xl border transition-colors {$config.locale === l.code ? 'bg-blue-600/15 border-blue-500/40 text-white' : 'bg-slate-800/50 border-white/5 text-slate-300 hover:bg-white/5'}">
        <span class="text-sm">{l.label}</span>
        {#if $config.locale === l.code}<Check size={14} class="text-blue-400" />{/if}
      </button>
    {/each}
  </div>
</div>
