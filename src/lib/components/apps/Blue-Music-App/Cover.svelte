<script lang="ts">
  import { run } from 'svelte/legacy';

  import { Music } from 'lucide-svelte';
  import { toAssetUrl } from '../../../utils/systemBridge';
  import { coverFile } from './musicIO';

  
  

  let src = $state<string | null>(null);
  let el = $state() as HTMLDivElement;
  let requested = $state('');

  // Loads when scrolled into view, and again when the track/album changes.
  function load() {
    if (!trackPath) { src = null; return; }
    const id = `${cacheKey ?? trackPath}`;
    if (requested === id) return;
    requested = id;
    coverFile(trackPath, cacheKey).then((p) => { if (requested === id) src = p ? toAssetUrl(p) : null; });
  }

  let visible = $state(false);
  import { onMount, onDestroy } from 'svelte';
  interface Props {
    /** A track of the album — its embedded art or the folder's cover.jpg is used. */
    trackPath: string | null;
    /** Shared lookup key (one request per album). */
    cacheKey?: string | undefined;
    size?: number;
    rounded?: string;
  }

  let {
    trackPath,
    cacheKey = undefined,
    size = 40,
    rounded = 'rounded-lg'
  }: Props = $props();
  let io: IntersectionObserver | undefined;
  onMount(() => {
    if (typeof IntersectionObserver === 'undefined') { visible = true; return; }
    io = new IntersectionObserver((entries) => { if (entries.some((e) => e.isIntersecting)) { visible = true; io?.disconnect(); } }, { rootMargin: '200px' });
    io.observe(el);
  });
  onDestroy(() => io?.disconnect());
  run(() => {
    if (trackPath !== undefined && visible) load();
  });
  run(() => {
    if (!trackPath) { src = null; requested = ''; }
  });
</script>

<div bind:this={el} class="shrink-0 overflow-hidden bg-slate-800 flex items-center justify-center {rounded}" style="width:{size}px;height:{size}px">
  {#if src}
    <img {src} alt="" class="w-full h-full object-cover" draggable="false" onerror={() => (src = null)} />
  {:else}
    <Music size={Math.max(12, Math.round(size / 2.6))} class="text-slate-600" />
  {/if}
</div>
