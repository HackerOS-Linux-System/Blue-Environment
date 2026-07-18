<script lang="ts">
  import { Command, X } from 'lucide-svelte';
  import type { CommandEntry } from './types';
  import { createEventDispatcher } from 'svelte';

  export let visible: boolean;
  export let input: string;
  export let commands: CommandEntry[];

  const dispatch = createEventDispatcher<{ close: void; input: string }>();

  $: filtered = commands.filter((c) => c.label.toLowerCase().includes(input.toLowerCase()));
</script>

{#if visible}
  <div class="absolute inset-0 bg-black/50 flex items-start justify-center z-50 pt-20" on:click={() => dispatch('close')}>
    <div class="bg-slate-800 rounded-xl w-96 p-4 shadow-2xl" on:click|stopPropagation>
      <div class="flex items-center gap-2 mb-3">
        <Command size={14} class="text-slate-500" />
        <input type="text" value={input} on:input={(e) => dispatch('input', e.currentTarget.value)}
          placeholder="Type a command…" class="flex-1 bg-transparent text-sm text-white focus:outline-none" autofocus />
        <button on:click={() => dispatch('close')} class="text-slate-500 hover:text-white"><X size={13} /></button>
      </div>
      <div class="space-y-0.5 max-h-56 overflow-y-auto">
        {#each filtered as cmd (cmd.id)}
          <button on:click={() => { cmd.action(); dispatch('close'); }} class="w-full flex items-center justify-between px-3 py-2 hover:bg-white/10 rounded text-sm text-left">
            <span>{cmd.label}</span>
            {#if cmd.shortcut}<span class="text-xs text-slate-500 font-mono">{cmd.shortcut}</span>{/if}
          </button>
        {/each}
        {#if filtered.length === 0}<p class="text-xs text-slate-600 text-center py-4">No matching commands</p>{/if}
      </div>
    </div>
  </div>
{/if}
