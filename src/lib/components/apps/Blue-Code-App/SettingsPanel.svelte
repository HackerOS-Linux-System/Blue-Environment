<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { X, RotateCcw } from 'lucide-svelte';
  import { blueCodeSettings } from './blueCodeSettings';

  export let visible = false;
  export let editorTheme: string;
  export let fontSize: number;

  const dispatch = createEventDispatcher<{ close: void; theme: string; fontSize: number }>();
</script>

{#if visible}
  <div class="absolute inset-0 z-40 bg-black/50 flex items-start justify-center pt-16"
    on:click|self={() => dispatch('close')} on:keydown={(e) => e.key === 'Escape' && dispatch('close')} role="presentation">
    <div class="w-[420px] max-w-[92%] bg-slate-800 border border-white/10 rounded-2xl shadow-2xl overflow-hidden" role="dialog" aria-label="Blue Code settings">
      <div class="flex items-center justify-between px-4 py-3 border-b border-white/10">
        <span class="font-medium">Blue Code — Settings</span>
        <button on:click={() => dispatch('close')} class="p-1 hover:bg-white/10 rounded"><X size={14} /></button>
      </div>

      <div class="p-4 space-y-4 text-sm">
        <label class="flex items-start justify-between gap-4 cursor-pointer">
          <span>
            <span class="block">Open files with a single click</span>
            <span class="block text-xs text-slate-400">Off = double-click to open (classic behaviour).</span>
          </span>
          <input type="checkbox" class="mt-1 accent-blue-500" checked={$blueCodeSettings.openOnSingleClick}
            on:change={(e) => blueCodeSettings.patch({ openOnSingleClick: e.currentTarget.checked })} />
        </label>

        <label class="flex items-center justify-between gap-4 cursor-pointer">
          <span>Minimap</span>
          <input type="checkbox" class="accent-blue-500" checked={$blueCodeSettings.minimap}
            on:change={(e) => blueCodeSettings.patch({ minimap: e.currentTarget.checked })} />
        </label>

        <label class="flex items-center justify-between gap-4 cursor-pointer">
          <span>Word wrap</span>
          <input type="checkbox" class="accent-blue-500" checked={$blueCodeSettings.wordWrap}
            on:change={(e) => blueCodeSettings.patch({ wordWrap: e.currentTarget.checked })} />
        </label>

        <label class="flex items-center justify-between gap-4 cursor-pointer">
          <span>Show terminal on start</span>
          <input type="checkbox" class="accent-blue-500" checked={$blueCodeSettings.showTerminalOnStart}
            on:change={(e) => blueCodeSettings.patch({ showTerminalOnStart: e.currentTarget.checked })} />
        </label>

        <div class="flex items-center justify-between gap-4">
          <span>Tab size</span>
          <select class="bg-slate-700 rounded px-2 py-1" value={$blueCodeSettings.tabSize}
            on:change={(e) => blueCodeSettings.patch({ tabSize: Number(e.currentTarget.value) })}>
            {#each [2, 4, 8] as n}<option value={n}>{n}</option>{/each}
          </select>
        </div>

        <div class="flex items-center justify-between gap-4">
          <span>Font size</span>
          <input type="number" min="10" max="28" class="w-16 bg-slate-700 rounded px-2 py-1" value={fontSize}
            on:change={(e) => dispatch('fontSize', Math.min(28, Math.max(10, Number(e.currentTarget.value) || 13)))} />
        </div>

        <div class="flex items-center justify-between gap-4">
          <span>Theme</span>
          <select class="bg-slate-700 rounded px-2 py-1" value={editorTheme} on:change={(e) => dispatch('theme', e.currentTarget.value)}>
            <option value="blue-dark">Blue Dark</option>
            <option value="blue-light">Blue Light</option>
            <option value="blue-ocean">Blue Ocean</option>
          </select>
        </div>
      </div>

      <div class="px-4 py-3 border-t border-white/10 flex justify-end">
        <button on:click={() => blueCodeSettings.reset()} class="flex items-center gap-1 text-xs text-slate-400 hover:text-white">
          <RotateCcw size={12} /> Restore defaults
        </button>
      </div>
    </div>
  </div>
{/if}
