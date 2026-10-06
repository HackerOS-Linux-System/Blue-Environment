<script lang="ts">
  /** Virtualised track list — only the rows in view are in the DOM, so a 20 000-track library scrolls smoothly. */
  import { createEventDispatcher } from 'svelte';
  import { Music } from 'lucide-svelte';
  import { t } from '../../../stores/language';
  import { fmtTime, type Track } from './musicLogic';

  export let tracks: Track[];
  export let currentPath: string | null = null;
  export let playing = false;
  /** Show the album column when there is room. */
  export let showAlbum = true;
  /** Show the track number (album view) instead of the row index. */
  export let numbered = false;
  export let failed: ReadonlySet<string> = new Set();

  const dispatch = createEventDispatcher<{ play: number; menu: { event: MouseEvent; index: number } }>();
  const ROW = 44;
  const OVERSCAN = 8;

  let scroller: HTMLDivElement;
  let scrollTop = 0;
  let height = 400;
  let width = 600;

  $: start = Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN);
  $: end = Math.min(tracks.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN);
  $: slice = tracks.slice(start, end);
  $: wide = width >= 560 && showAlbum;
  // a different list (new section/search) starts at the top
  let lastLen = -1;
  $: if (scroller && tracks.length !== lastLen) { lastLen = tracks.length; if (scroller.scrollTop > tracks.length * ROW) scroller.scrollTop = 0; }

  /** Brings the playing track into view (called by the parent). */
  export function revealCurrent() {
    const i = tracks.findIndex((x) => x.path === currentPath);
    if (i < 0 || !scroller) return;
    const top = i * ROW;
    if (top < scroller.scrollTop || top + ROW > scroller.scrollTop + height) scroller.scrollTop = Math.max(0, top - height / 2);
  }
  const unknown = () => $t('music.unknown_artist');
</script>

<div bind:this={scroller} bind:clientHeight={height} bind:clientWidth={width} on:scroll={() => (scrollTop = scroller.scrollTop)} class="h-full overflow-y-auto">
  <div style="height:{tracks.length * ROW}px; position:relative;">
    {#each slice as track, k (track.path + ':' + (start + k))}
      {@const i = start + k}
      {@const isCur = track.path === currentPath}
      <button
        on:click={() => dispatch('play', i)} on:contextmenu|preventDefault={(e) => dispatch('menu', { event: e, index: i })}
        class="absolute left-0 right-0 flex items-center gap-3 px-4 text-left hover:bg-white/5 transition-colors {isCur ? 'bg-blue-500/10' : ''} {failed.has(track.path) ? 'opacity-40' : ''}"
        style="top:{i * ROW}px; height:{ROW}px">
        <div class="w-6 shrink-0 text-center text-[11px] tabular-nums text-slate-500">
          {#if isCur && playing}
            <span class="inline-flex items-end gap-0.5 h-3"><span class="w-0.5 bg-blue-400 animate-pulse" style="height:60%" /><span class="w-0.5 bg-blue-400 animate-pulse" style="height:100%;animation-delay:150ms" /><span class="w-0.5 bg-blue-400 animate-pulse" style="height:40%;animation-delay:300ms" /></span>
          {:else if isCur}<Music size={12} class="inline text-blue-400" />
          {:else}{numbered && track.trackNo ? track.trackNo : i + 1}{/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-[13px] {isCur ? 'text-blue-300' : 'text-slate-100'}">{track.title}</div>
          <div class="truncate text-[11px] text-slate-500">{track.artist || unknown()}</div>
        </div>
        {#if wide}<div class="w-1/3 min-w-0 truncate text-[12px] text-slate-500">{track.album}</div>{/if}
        <div class="w-12 shrink-0 text-right text-[11px] tabular-nums text-slate-500">{track.durationSecs ? fmtTime(track.durationSecs) : ''}</div>
      </button>
    {/each}
  </div>
</div>
