<script lang="ts">
  import { AlertCircle } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';

  export let languageLabel: string;
  export let line: number;
  export let col: number;
  export let errors: number;
  export let warnings: number;
  export let editorTheme: string;
  export let fontSize: number;

  const dispatch = createEventDispatcher<{ toggleTheme: void; fontSize: number }>();
</script>

<div class="h-6 bg-slate-800 border-t border-white/5 flex items-center px-3 gap-4 text-[11px] text-slate-500 shrink-0">
  <span>{languageLabel}</span>
  <span>Ln {line}, Col {col}</span>
  {#if errors > 0}<span class="text-red-400 flex items-center gap-1"><AlertCircle size={11} /> {errors}</span>{/if}
  {#if warnings > 0}<span class="text-yellow-400 flex items-center gap-1"><AlertCircle size={11} /> {warnings}</span>{/if}
  <div class="flex-1" />
  <button on:click={() => dispatch('toggleTheme')} class="hover:text-white">{editorTheme === 'blue-dark' ? '\u2600' : '\u25CF'}</button>
  <button on:click={() => dispatch('fontSize', Math.max(10, fontSize - 1))} class="hover:text-white">A-</button>
  <span>{fontSize}px</span>
  <button on:click={() => dispatch('fontSize', Math.min(24, fontSize + 1))} class="hover:text-white">A+</button>
</div>
