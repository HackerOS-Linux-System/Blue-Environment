<script lang="ts">
  import type { SwitcherItem } from '../stores/windowManager';
  import { APPS } from '../constants';
  import AppIconGlyph from './AppIconGlyph.svelte';

  export let windows: SwitcherItem[] = [];
  export let selectedIndex = 0;
  export let isVisible = false;

  // Resolves to whatever AppIconGlyph.svelte itself accepts: a lucide
  // component for a real Blue Environment app, or a `file://`/`http(s)`
  // URL string for an external window's icon already resolved by the
  // backend (window_tracker.rs/icon_resolver.rs) — AppIconGlyph already
  // handles both cases (plus a lettered-fallback for neither), so this
  // just picks the right one instead of duplicating that logic here.
  function iconFor(item: SwitcherItem) {
    return item.isExternal ? (item.iconPath || '') : APPS[item.appId!]?.icon;
  }
</script>

{#if isVisible && windows.length > 0}
  <div class="absolute inset-0 z-[200] flex items-center justify-center pointer-events-none">
    <div class="absolute inset-0 bg-black/40 backdrop-blur-sm" />
    <div class="relative bg-slate-900/95 border border-white/10 p-5 rounded-2xl shadow-2xl shadow-black/50 flex flex-col items-center gap-4 max-w-[88vw]">
      <div class="flex items-center gap-2 text-xs text-slate-500 mb-1">
        <kbd class="px-2 py-0.5 bg-slate-800 border border-white/10 rounded text-slate-400">Alt</kbd>
        <span>+</span>
        <kbd class="px-2 py-0.5 bg-slate-800 border border-white/10 rounded text-slate-400">Tab</kbd>
        <span class="text-slate-600">to cycle •</span>
        <span class="text-slate-600">release</span>
        <kbd class="px-2 py-0.5 bg-slate-800 border border-white/10 rounded text-slate-400">Alt</kbd>
        <span class="text-slate-600">to switch</span>
      </div>

      <div class="flex gap-3 overflow-x-auto max-w-[80vw] pb-1">
        {#each windows as win, index (win.id)}
          {@const isSelected = index === selectedIndex}
          <div class="flex flex-col items-center gap-2 p-3 rounded-xl transition-all duration-150 w-28 shrink-0
            {isSelected ? 'bg-blue-600/80 text-white scale-105 shadow-lg shadow-blue-500/30 ring-2 ring-blue-400/50' : 'bg-slate-800/60 text-slate-300'}">
            <div class="w-12 h-12 flex items-center justify-center">
              <AppIconGlyph icon={iconFor(win)} name={win.title} size={32} />
            </div>
            <span class="text-xs font-medium truncate w-full text-center leading-tight">{win.title}</span>
            <div class="flex items-center gap-1">
              {#if win.isMinimized}
                <span class="text-[10px] bg-slate-700 text-slate-400 px-1.5 rounded-full">hidden</span>
              {/if}
              {#if win.isExternal}
                <span class="text-[10px] bg-slate-700 text-slate-400 px-1.5 rounded-full" title="Running outside Blue Environment">external</span>
              {/if}
            </div>
            <span class="text-[10px] text-slate-500">WS {(win.workspace ?? 0) + 1}</span>
          </div>
        {/each}
      </div>

      <div class="text-sm font-semibold text-white/80">{windows[selectedIndex]?.title ?? ''}</div>
    </div>
  </div>
{/if}
