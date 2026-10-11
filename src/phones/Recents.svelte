<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { fade } from 'svelte/transition';
  import { X } from 'lucide-svelte';
  import { t } from '../lib/stores/language';
  import { classifySwipe } from '../lib/altShell/gestures';
  import type { RecentItem } from '../lib/altShell/apps';
  import AppIcon from './AppIcon.svelte';

  export let items: RecentItem[] = [];

  const dispatch = createEventDispatcher<{ open: string; close: string; clear: void; dismiss: void }>();

  // Swipe a card up to close that app.
  let sx = 0; let sy = 0;
  function down(e: PointerEvent) { sx = e.clientX; sy = e.clientY; }
  function up(e: PointerEvent, id: string) {
    if (classifySwipe(e.clientX - sx, e.clientY - sy, { minDistance: 50 }) === 'up') dispatch('close', id);
  }
</script>

<div class="absolute inset-0 z-20 bg-black/60 backdrop-blur-md flex flex-col text-white" transition:fade={{ duration: 150 }}
  role="presentation" on:click|self={() => dispatch('dismiss')}>
  {#if items.length === 0}
    <div class="flex-1 flex items-center justify-center text-white/70" on:click={() => dispatch('dismiss')} role="presentation">{$t('altui.no_recent')}</div>
  {:else}
    <div class="flex-1 min-h-0 overflow-x-auto flex items-center gap-4 px-8 snap-x snap-mandatory" role="presentation">
      {#each items as item (item.id)}
        <div class="snap-center shrink-0 w-56" on:pointerdown={down} on:pointerup={(e) => up(e, item.id)} role="presentation">
          <div class="flex items-center gap-2 mb-2 px-1">
            <AppIcon icon={item.icon} size={18} title={item.title} />
            <span class="text-sm truncate flex-1">{item.title}</span>
            <button class="p-1 rounded-full active:bg-white/20" aria-label={$t('altui.close_app')} on:click|stopPropagation={() => dispatch('close', item.id)}><X size={16} /></button>
          </div>
          <button class="w-full h-72 rounded-2xl bg-white/10 border {item.active ? 'border-blue-400' : 'border-white/10'} flex items-center justify-center active:scale-[0.98] transition-transform"
            on:click={() => dispatch('open', item.id)} aria-label={item.title}>
            <AppIcon icon={item.icon} size={56} title={item.title} />
          </button>
        </div>
      {/each}
    </div>
    <div class="shrink-0 flex justify-center pb-4">
      <button class="px-5 h-10 rounded-full bg-white/15 active:bg-white/25 text-sm" on:click={() => dispatch('clear')}>{$t('altui.clear_all')}</button>
    </div>
  {/if}
</div>
