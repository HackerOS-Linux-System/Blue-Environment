<script lang="ts">
  import { run, preventDefault } from 'svelte/legacy';

  import { onMount, onDestroy, tick } from 'svelte';
  import { FileText, FolderOpen, ZoomIn, ZoomOut, ChevronUp, ChevronDown, Search, PanelLeft, Copy, MoveHorizontal, Loader2, X, AlertTriangle } from 'lucide-svelte';
  import { t } from '../../../stores/language';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { dialogPrompt } from '../../../stores/dialog';
  import { clampPage, currentPage, dpiFor, fitWidthZoom, pageSizePx, parsePageInput, visiblePages, zoomIn, zoomOut, clampZoom } from '../../../utils/pdfView';

  interface Props {
    openPath?: string | undefined;
  }

  let { openPath = undefined }: Props = $props();

  interface PdfInfo { pages: number; title: string; author: string; producer: string; encrypted: boolean; pageWidth: number; pageHeight: number }
  interface PdfHit { page: number; snippet: string }

  let path = $state('');
  let password: string | null = null;
  let info = $state<PdfInfo | null>(null);
  let loading = $state(false);
  let error = $state('');

  let zoom = $state(1);
  let page = $state(1);
  let pageInput = $state('1');
  let showThumbs = $state(true);
  let showSearch = $state(false);
  let query = $state('');
  let hits = $state<PdfHit[]>([]);
  let searching = $state(false);

  let scroller = $state<HTMLDivElement | undefined>();
  let viewH = $state(600);
  let scrollTop = 0;

  /** rendered page images, keyed by page number; discarded on zoom change. */
  let images = $state<Record<number, string>>({});
  /** actual heights (px) of pages once rendered — documents can mix page sizes. */
  let heights = $state<number[]>([]);
  let thumbs = $state<Record<number, string>>({});
  let inflight = new Set<number>();
  let renderToken = 0;

  let name = $derived(path.split('/').pop() ?? '');
  let baseSize = $derived(info ? pageSizePx(info.pageWidth || 595, info.pageHeight || 842, zoom) : { w: 793, h: 1123 });
  run(() => {
    if (info && heights.length !== info.pages) heights = Array.from({ length: info.pages }, () => baseSize.h);
  });
  const GAP = 12;
  let tops = $derived(heights.reduce<number[]>((acc, h, i) => { acc.push(i === 0 ? 0 : acc[i - 1] + heights[i - 1] + GAP); return acc; }, []));
  let totalH = $derived(heights.length ? tops[tops.length - 1] + heights[heights.length - 1] : 0);

  onMount(async () => { if (openPath) await load(openPath); });
  onDestroy(() => { renderToken++; });

  async function chooseFile() {
    const p = await SystemBridge.pickFile([{ name: 'PDF', extensions: ['pdf'] }], $t('pdf.open'), 'blue-pdf.open');
    if (p) await load(p);
  }

  async function load(p: string, pw: string | null = null) {
    loading = true; error = ''; renderToken++;
    images = {}; thumbs = {}; hits = []; heights = []; inflight = new Set();
    try {
      const i = await SystemBridge.invokeCommand<PdfInfo>('pdf_info', { path: p, password: pw });
      path = p; password = pw; info = i;
      page = 1; pageInput = '1';
      await tick();
      fitWidth();
      scheduleRender();
    } catch (e: any) {
      const msg = String(e?.message ?? e);
      if (msg === 'PASSWORD_REQUIRED') {
        const entered = await dialogPrompt({ title: $t('pdf.password.title'), label: $t('pdf.password.label'), inputType: 'password', confirmLabel: $t('pdf.password.open'), cancelLabel: $t('common.cancel') });
        if (entered) { loading = false; await load(p, entered); return; }
        error = $t('pdf.err.password');
      } else {
        error = msg;
      }
      info = null;
    } finally { loading = false; }
  }

  function fitWidth() {
    if (!scroller || !info) return;
    setZoom(fitWidthZoom(scroller.clientWidth - (showThumbs ? 0 : 0), info.pageWidth || 595));
  }

  function setZoom(z: number) {
    const next = clampZoom(z);
    if (next === zoom) return;
    const keep = page;
    zoom = next; renderToken++; images = {}; inflight = new Set();
    heights = [];
    tick().then(() => { goTo(keep, false); scheduleRender(); });
  }

  let raf = 0;
  function onScroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop; viewH = scroller.clientHeight;
    const p = currentPage(tops, heights, scrollTop, viewH);
    if (p !== page) { page = p; pageInput = String(p); }
    if (!raf) raf = requestAnimationFrame(() => { raf = 0; scheduleRender(); });
  }

  async function renderPage(n: number, dpi: number, token: number): Promise<string | null> {
    try {
      const src = await SystemBridge.invokeCommand<string>('pdf_render_page', { path, page: n, dpi, password });
      return token === renderToken ? src : null;
    } catch { return null; }
  }

  function scheduleRender() {
    if (!info || !scroller) return;
    const token = renderToken;
    const dpr = typeof window !== 'undefined' ? window.devicePixelRatio : 1;
    const wanted = visiblePages(tops, heights, scroller.scrollTop, scroller.clientHeight, 700);
    for (const n of wanted) {
      if (images[n] || inflight.has(n) || inflight.size >= 3) continue;
      inflight.add(n);
      renderPage(n, dpiFor(zoom, dpr), token).then((src) => {
        inflight.delete(n);
        if (!src || token !== renderToken) return;
        images = { ...images, [n]: src };
        scheduleRender();
      });
    }
    if (showThumbs) {
      for (const n of wanted.slice(0, 8)) {
        if (thumbs[n] || inflight.has(-n)) continue;
        inflight.add(-n);
        renderPage(n, 36, token).then((src) => { inflight.delete(-n); if (src) thumbs = { ...thumbs, [n]: src }; });
      }
    }
  }

  function onImgLoad(n: number, e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    const dpr = typeof window !== 'undefined' ? window.devicePixelRatio : 1;
    // natural size is at (dpi) — convert back to CSS px at this zoom
    const h = Math.round(img.naturalHeight / Math.max(1, dpr) * (96 * zoom / dpiFor(zoom, dpr)) * Math.max(1, dpr));
    if (h > 0 && heights[n - 1] !== h) { heights[n - 1] = h; heights = heights; }
  }

  function goTo(n: number, smooth = true) {
    if (!info || !scroller) return;
    const p = clampPage(n, info.pages);
    page = p; pageInput = String(p);
    scroller.scrollTo({ top: tops[p - 1] ?? 0, behavior: smooth ? 'smooth' : 'auto' });
  }

  function submitPage() {
    const p = info ? parsePageInput(pageInput, info.pages) : null;
    if (p) goTo(p); else pageInput = String(page);
  }

  async function runSearch() {
    if (!info || !query.trim()) { hits = []; return; }
    searching = true;
    try { hits = await SystemBridge.invokeCommand<PdfHit[]>('pdf_search', { path, query, password }); }
    catch (e: any) { error = String(e?.message ?? e); }
    finally { searching = false; }
  }

  async function copyPageText() {
    try {
      const txt = await SystemBridge.invokeCommand<string>('pdf_page_text', { path, page, password });
      await navigator.clipboard.writeText(txt);
    } catch { /* clipboard unavailable */ }
  }

  function onKey(e: KeyboardEvent) {
    if (!info) return;
    const target = e.target as HTMLElement;
    const typing = target?.tagName === 'INPUT';
    if ((e.ctrlKey || e.metaKey) && (e.key === '=' || e.key === '+')) { e.preventDefault(); setZoom(zoomIn(zoom)); }
    else if ((e.ctrlKey || e.metaKey) && e.key === '-') { e.preventDefault(); setZoom(zoomOut(zoom)); }
    else if ((e.ctrlKey || e.metaKey) && e.key === '0') { e.preventDefault(); fitWidth(); }
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') { e.preventDefault(); showSearch = true; }
    else if (!typing && (e.key === 'PageDown' || e.key === 'ArrowRight')) { e.preventDefault(); goTo(page + 1); }
    else if (!typing && (e.key === 'PageUp' || e.key === 'ArrowLeft')) { e.preventDefault(); goTo(page - 1); }
    else if (!typing && e.key === 'Home') { e.preventDefault(); goTo(1); }
    else if (!typing && e.key === 'End') { e.preventDefault(); goTo(info.pages); }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="flex flex-col h-full bg-slate-900 text-white">
  <div class="flex items-center gap-1.5 px-3 py-2 border-b border-white/5 shrink-0 bg-slate-800/50 text-xs">
    <FileText size={16} class="text-red-400 shrink-0" />
    <span class="font-semibold truncate max-w-[14rem]" title={path}>{name || $t('pdf.title')}</span>
    <div class="flex-1"></div>
    {#if info}
      <button class="p-1.5 rounded hover:bg-white/10" onclick={() => (showThumbs = !showThumbs)} title={$t('pdf.thumbs')} aria-label={$t('pdf.thumbs')} aria-pressed={showThumbs}><PanelLeft size={15} /></button>
      <button class="p-1.5 rounded hover:bg-white/10 disabled:opacity-40" disabled={page <= 1} onclick={() => goTo(page - 1)} aria-label={$t('pdf.prev')}><ChevronUp size={15} /></button>
      <form onsubmit={preventDefault(submitPage)} class="flex items-center gap-1">
        <input bind:value={pageInput} class="w-10 text-center bg-slate-800 border border-white/10 rounded px-1 py-0.5 outline-none focus:border-blue-500" aria-label={$t('pdf.page')} />
        <span class="text-slate-400">/ {info.pages}</span>
      </form>
      <button class="p-1.5 rounded hover:bg-white/10 disabled:opacity-40" disabled={page >= info.pages} onclick={() => goTo(page + 1)} aria-label={$t('pdf.next')}><ChevronDown size={15} /></button>
      <div class="w-px h-5 bg-white/10 mx-1"></div>
      <button class="p-1.5 rounded hover:bg-white/10" onclick={() => setZoom(zoomOut(zoom))} aria-label={$t('pdf.zoom_out')}><ZoomOut size={15} /></button>
      <span class="w-10 text-center tabular-nums">{Math.round(zoom * 100)}%</span>
      <button class="p-1.5 rounded hover:bg-white/10" onclick={() => setZoom(zoomIn(zoom))} aria-label={$t('pdf.zoom_in')}><ZoomIn size={15} /></button>
      <button class="p-1.5 rounded hover:bg-white/10" onclick={fitWidth} title={$t('pdf.fit_width')} aria-label={$t('pdf.fit_width')}><MoveHorizontal size={15} /></button>
      <div class="w-px h-5 bg-white/10 mx-1"></div>
      <button class="p-1.5 rounded hover:bg-white/10 {showSearch ? 'bg-white/10' : ''}" onclick={() => (showSearch = !showSearch)} aria-label={$t('pdf.search')} aria-pressed={showSearch}><Search size={15} /></button>
      <button class="p-1.5 rounded hover:bg-white/10" onclick={copyPageText} title={$t('pdf.copy_text')} aria-label={$t('pdf.copy_text')}><Copy size={15} /></button>
    {/if}
    <button class="flex items-center gap-1.5 px-2.5 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg" onclick={chooseFile}><FolderOpen size={13} /> {$t('pdf.open')}</button>
  </div>

  {#if showSearch && info}
    <form onsubmit={preventDefault(runSearch)} class="flex items-center gap-2 px-3 py-2 border-b border-white/5 bg-slate-800/30 text-xs">
      <Search size={13} class="text-slate-400" />
      <!-- svelte-ignore a11y_autofocus -->
      <input autofocus bind:value={query} placeholder={$t('pdf.search_placeholder')} class="flex-1 bg-transparent outline-none" />
      {#if searching}<Loader2 size={13} class="animate-spin" />{:else}<span class="text-slate-400">{$t('pdf.hits').replace('{n}', String(hits.length))}</span>{/if}
      <button type="button" onclick={() => { showSearch = false; hits = []; }} class="p-1 rounded hover:bg-white/10" aria-label={$t('common.close')}><X size={13} /></button>
    </form>
    {#if hits.length}
      <div class="max-h-32 overflow-y-auto border-b border-white/5 text-xs">
        {#each hits as h}
          <button class="w-full text-left px-3 py-1 hover:bg-white/5 flex gap-2" onclick={() => goTo(h.page)}>
            <span class="text-blue-300 shrink-0 w-14">{$t('pdf.page')} {h.page}</span><span class="truncate text-slate-300">{h.snippet}</span>
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  <div class="flex flex-1 min-h-0">
    {#if info && showThumbs}
      <div class="w-28 shrink-0 overflow-y-auto border-r border-white/5 bg-slate-950/40 p-2 space-y-2" aria-label={$t('pdf.thumbs')}>
        {#each Array.from({ length: info.pages }, (_, i) => i + 1) as n}
          <button onclick={() => goTo(n)} class="block w-full rounded border {n === page ? 'border-blue-500' : 'border-transparent hover:border-white/20'}" aria-label={`${$t('pdf.page')} ${n}`}>
            {#if thumbs[n]}<img src={thumbs[n]} alt="" class="w-full bg-white" />{:else}<div class="w-full aspect-[3/4] bg-slate-800"></div>{/if}
            <div class="text-[10px] text-slate-400 py-0.5">{n}</div>
          </button>
        {/each}
      </div>
    {/if}

    <div class="flex-1 overflow-auto bg-slate-800/60 relative" bind:this={scroller} onscroll={onScroll} bind:clientHeight={viewH}>
      {#if loading}
        <div class="absolute inset-0 grid place-items-center text-slate-400"><Loader2 size={26} class="animate-spin" /></div>
      {:else if error}
        <div class="m-6 flex items-start gap-2 text-red-200 bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2 text-sm" role="alert"><AlertTriangle size={16} class="mt-0.5 shrink-0" /> {error}</div>
      {:else if !info}
        <div class="h-full grid place-items-center text-center text-slate-400 p-6">
          <div>
            <FileText size={44} class="mx-auto mb-3 text-slate-600" />
            <p class="text-sm mb-3">{$t('pdf.empty')}</p>
            <button class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm text-white" onclick={chooseFile}>{$t('pdf.open')}</button>
          </div>
        </div>
      {:else}
        <div class="relative mx-auto" style="width:{baseSize.w}px;height:{totalH}px">
          {#each heights as h, i}
            <div class="absolute left-0 bg-white shadow-lg" style="top:{tops[i]}px;width:{baseSize.w}px;height:{h}px" data-page={i + 1}>
              {#if images[i + 1]}
                <img src={images[i + 1]} alt={`${$t('pdf.page')} ${i + 1}`} draggable="false" class="w-full h-full select-none" onload={(e) => onImgLoad(i + 1, e)} />
              {:else}
                <div class="w-full h-full grid place-items-center text-slate-400"><Loader2 size={18} class="animate-spin" /></div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>
