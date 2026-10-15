<script lang="ts">
  /**
   * A store preview image. The webview's CSP only allows `data:` images, so the
   * backend fetches the picture (and caches it on disk); this component asks for
   * it only once it is about to scroll into view, so a page of 24 cards does not
   * fire 24 requests at once.
   */
  import { onMount, onDestroy } from 'svelte';
  import { ImageOff, Loader2 } from 'lucide-svelte';
  import { loadThumbnail } from '../../../utils/iconStore';

  export let url = '';
  export let alt = '';
  /** Load immediately instead of waiting to become visible (detail view). */
  export let eager = false;
  export let fit: 'cover' | 'contain' = 'cover';

  let el: HTMLDivElement;
  let src = '';
  let state: 'idle' | 'loading' | 'ok' | 'error' = 'idle';
  let mounted = false;
  let wanted = false;
  let current = '';
  let io: IntersectionObserver | null = null;

  async function start() {
    if (!url) { state = 'error'; return; }
    const asked = url;
    current = asked;
    state = 'loading';
    try {
      const data = await loadThumbnail(asked);
      if (current === asked) { src = data; state = 'ok'; }
    } catch {
      if (current === asked) state = 'error';
    }
  }

  // The card is reused for another item (list re-sorted / filtered).
  $: if (mounted && url !== current) {
    src = ''; state = 'idle';
    if (wanted) start();
  }

  onMount(() => {
    mounted = true;
    if (eager || typeof IntersectionObserver === 'undefined') { wanted = true; start(); return; }
    io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) { wanted = true; io?.disconnect(); start(); }
    }, { rootMargin: '240px' });
    io.observe(el);
  });
  onDestroy(() => io?.disconnect());
</script>

<div bind:this={el} class="relative w-full h-full bg-slate-800/60 overflow-hidden">
  {#if state === 'ok'}
    <img {src} {alt} draggable="false" class="absolute inset-0 w-full h-full {fit === 'contain' ? 'object-contain' : 'object-cover'}" />
  {:else if state === 'error'}
    <div class="absolute inset-0 flex items-center justify-center text-slate-600"><ImageOff size={26} /></div>
  {:else}
    <div class="absolute inset-0 flex items-center justify-center text-slate-600"><Loader2 size={20} class="animate-spin" /></div>
  {/if}
</div>
