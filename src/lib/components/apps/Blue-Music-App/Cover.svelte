<script lang="ts">
  import { Music } from 'lucide-svelte';
  import { toAssetUrl } from '../../../utils/systemBridge';
  import { coverFile } from './musicIO';

  /** A track of the album — its embedded art or the folder's cover.jpg is used. */
  export let trackPath: string | null;
  /** Shared lookup key (one request per album). */
  export let cacheKey: string | undefined = undefined;
  export let size = 40;
  export let rounded = 'rounded-lg';

  let src: string | null = null;
  let el: HTMLDivElement;
  let requested = '';

  // Loads when scrolled into view, and again when the track/album changes.
  function load() {
    if (!trackPath) { src = null; return; }
    const id = `${cacheKey ?? trackPath}`;
    if (requested === id) return;
    requested = id;
    coverFile(trackPath, cacheKey).then((p) => { if (requested === id) src = p ? toAssetUrl(p) : null; });
  }
  $: if (trackPath !== undefined && visible) load();
  $: if (!trackPath) { src = null; requested = ''; }

  let visible = false;
  import { onMount, onDestroy } from 'svelte';
  let io: IntersectionObserver | undefined;
  onMount(() => {
    if (typeof IntersectionObserver === 'undefined') { visible = true; return; }
    io = new IntersectionObserver((entries) => { if (entries.some((e) => e.isIntersecting)) { visible = true; io?.disconnect(); } }, { rootMargin: '200px' });
    io.observe(el);
  });
  onDestroy(() => io?.disconnect());
</script>

<div bind:this={el} class="shrink-0 overflow-hidden bg-slate-800 flex items-center justify-center {rounded}" style="width:{size}px;height:{size}px">
  {#if src}
    <img {src} alt="" class="w-full h-full object-cover" draggable="false" on:error={() => (src = null)} />
  {:else}
    <Music size={Math.max(12, Math.round(size / 2.6))} class="text-slate-600" />
  {/if}
</div>
