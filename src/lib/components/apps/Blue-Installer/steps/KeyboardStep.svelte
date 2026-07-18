<script lang="ts">
  import { Check, Keyboard } from 'lucide-svelte';
  import { KEYBOARD_LAYOUTS } from '../types';
  import type { InstallState } from '../installState';

  export let state: InstallState;
  const { config } = state;
  let testInput = '';
</script>

<div class="flex-1 flex flex-col px-10 py-8 overflow-y-auto">
  <div class="flex items-center gap-2 mb-6"><Keyboard size={20} class="text-blue-400" /><h2 class="text-xl font-semibold text-white">Keyboard layout</h2></div>
  <div class="grid grid-cols-3 gap-2 max-w-xl mb-6">
    {#each KEYBOARD_LAYOUTS as k (k.id)}
      <button on:click={() => config.update((c) => ({ ...c, keyboardLayout: k.id }))}
        class="flex items-center justify-between px-3 py-2.5 rounded-xl border transition-colors {$config.keyboardLayout === k.id ? 'bg-blue-600/15 border-blue-500/40 text-white' : 'bg-slate-800/50 border-white/5 text-slate-300 hover:bg-white/5'}">
        <span class="text-sm">{k.label}</span>
        {#if $config.keyboardLayout === k.id}<Check size={13} class="text-blue-400" />{/if}
      </button>
    {/each}
  </div>
  <div class="max-w-xl">
    <label class="block text-xs text-slate-500 mb-1.5">Type here to test your layout</label>
    <input bind:value={testInput} placeholder="Type something…" class="w-full bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 text-white focus:outline-none focus:border-blue-500/50" />
  </div>
</div>
