<script lang="ts">
  import { t } from '../../../stores/language';
  import { onMount } from 'svelte';
  import { Globe, Info } from 'lucide-svelte';
  import { SPEED_DIALS } from './types';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { createEventDispatcher } from 'svelte';

  export let error: string | null = null;
  /** Display name of the selected search engine (placeholder text). */
  export let engineName = 'DuckDuckGo';

  const dispatch = createEventDispatcher<{ navigate: string }>();
  let inputEl: HTMLInputElement;
  let input = '';

  onMount(() => { setTimeout(() => inputEl?.focus(), 80); });

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' && input.trim()) dispatch('navigate', input);
  }
</script>

<div class="flex-1 flex flex-col items-center justify-center gap-8 p-8 overflow-y-auto">
  <div class="text-center">
    <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center mx-auto mb-4 shadow-lg shadow-blue-500/30">
      <Globe size={32} class="text-white" />
    </div>
    <h1 class="text-xl font-bold text-white mb-1">Blue Web</h1>
    <p class="text-slate-400 text-sm">{$t('blueweb.subtitle')}</p>
  </div>

  <div class="w-full max-w-xl">
    <input bind:this={inputEl} bind:value={input} on:keydown={handleKeyDown}
      placeholder={$t('blueweb.search_placeholder', { engine: engineName })}
      class="w-full bg-slate-800 border border-white/10 rounded-2xl px-5 py-3.5 text-sm text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/50" />
  </div>

  <div class="grid grid-cols-4 gap-3 w-full max-w-xl">
    {#each SPEED_DIALS as sd (sd.url)}
      <button on:click={() => dispatch('navigate', sd.url)} class="flex flex-col items-center gap-2 p-3 bg-slate-800/50 hover:bg-slate-700/60 border border-white/5 rounded-xl transition-colors group">
        <span class="text-2xl">{sd.icon}</span>
        <span class="text-[11px] text-slate-400 group-hover:text-white text-center leading-tight">{sd.label}</span>
      </button>
    {/each}
  </div>

  {#if error}
    <div class="flex items-start gap-2 bg-red-500/10 border border-red-500/20 rounded-xl p-3 max-w-lg text-xs text-red-300">
      <Info size={14} class="shrink-0 mt-0.5" /><div>{error}</div>
    </div>
  {/if}

  <div class="flex items-center gap-2 text-xs text-slate-600">
    <Globe size={11} />
    {SystemBridge.isTauri() ? $t('blueweb.status_native') : $t('blueweb.status_dev')}
  </div>
</div>
