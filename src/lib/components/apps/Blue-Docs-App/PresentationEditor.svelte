<script lang="ts">
  /**
   * Presentation editor for Blue Docs.
   *
   * ── What it covers ───────────────────────────────────────────────
   * - **Layouts**: title slide, title + content, section header, two
   *   columns, content + picture, title only, blank (`slideModel.ts`).
   *   Changing the layout keeps the slide's text.
   * - **Free text boxes and pictures** that can be dragged, resized (corner
   *   handles), nudged with the arrow keys, stacked (forward / backward)
   *   and removed; text is edited in place (double-click / Enter).
   * - **Look**: font family, size, colour, alignment, bold / italic /
   *   underline, bullets and a fill colour per text block; a background
   *   colour per slide (palette or any colour).
   * - **Transitions** (fade, push, wipe, zoom, split, cover) with direction
   *   and duration, plus optional auto-advance, played in the full-screen
   *   presentation; "apply to all slides".
   * - Speaker notes, a table per slide, thumbnails, duplicate / reorder.
   * - Real `.pptx` import/export (`pptxFile.ts`) round-trips all of the above.
   *
   * ── What it does not cover ───────────────────────────────────────
   * - No shapes/drawing, charts, SmartArt, animations of single objects,
   *   masters/themes (every slide is styled on its own), rotated objects or
   *   several tables per slide.
   * - Inline formatting inside a text block is `**bold**` / `*italic*` only.
   * - Print-to-PDF prints the active slide, not one slide per page.
   * - PowerPoint files with a non-16:9 page size are opened stretched onto
   *   the 16:9 canvas.
   *
   * The deck lives in `DocFile.content` as JSON (see `types.ts`); decks saved
   * by older builds are migrated on load by `normalizePresentation`.
   */
  import { onDestroy, tick } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import {
    Plus, Trash2, ChevronUp, ChevronDown, Play, X, ChevronLeft, ChevronRight, ImagePlus, Type, Bold, Italic,
    Underline, AlignLeft, AlignCenter, AlignRight, AlignJustify, List, Copy, Table as TableIcon,
  } from 'lucide-svelte';
  import type { Presentation, Rect, Slide, SlideLayoutId, SlideTextBox, SlideTextStyle, TransitionType } from './types';
  import { emptyPresentation } from './types';
  import SlideCanvas from './SlideCanvas.svelte';
  import {
    FONT_FAMILIES, LAYOUTS, LAYOUT_IDS, SLIDE_BACKGROUNDS, TRANSITIONS_WITH_DIRECTION, TRANSITION_LABELS,
    clampRect, newId, newImageBox, newSlide, newTextBox, normalizePresentation, resolveBoxStyle, resolveRect,
    resolveStyle, transitionDuration, transitionKeyframes,
  } from './slideModel';

  interface Props {
    content: string;
  }

  let { content }: Props = $props();

  const dispatch = createEventDispatcher<{ change: string }>();

  function parse(raw: string): Presentation {
    try {
      const parsed = JSON.parse(raw);
      if (parsed && Array.isArray(parsed.slides) && parsed.slides.length > 0) return normalizePresentation(parsed);
    } catch { /* fall through to a fresh deck below */ }
    return normalizePresentation(emptyPresentation());
  }

  let deck = $state<Presentation>(parse(content));
  let activeSlideId = $state<string | undefined>(deck.slides[0]?.id);
  let selectedId = $state<string | null>(null);
  let presenting = $state(false);
  let showNotes = $state(false);
  let presentWrap = $state<HTMLDivElement>();
  let advanceTimer: ReturnType<typeof setTimeout> | undefined;

  // Re-parse if the parent swaps in different content wholesale (e.g. switching
  // tabs to another presentation) — but not for our own `commit()` echoes.
  let lastContent = content;
  $effect(() => {
    if (content !== lastContent) {
      lastContent = content;
      deck = parse(content);
      selectedId = null;
      if (!deck.slides.some((s) => s.id === activeSlideId)) activeSlideId = deck.slides[0]?.id;
    }
  });

  let activeSlide = $derived(deck.slides.find((s) => s.id === activeSlideId) ?? deck.slides[0]);
  let activeIndex = $derived(deck.slides.findIndex((s) => s.id === activeSlide?.id));

  function commit() {
    const json = JSON.stringify(deck);
    lastContent = json;
    dispatch('change', json);
  }

  // ── slides ──────────────────────────────────────────────────────────
  let newLayout = $state<SlideLayoutId>('title-content');

  function selectSlide(id: string) {
    activeSlideId = id;
    selectedId = null;
  }

  function addSlide(layout: SlideLayoutId = newLayout) {
    const at = Math.max(0, activeIndex) + 1;
    const slide = newSlide(layout, deck.slides.length);
    if (activeSlide) slide.background = activeSlide.background;
    deck.slides = [...deck.slides.slice(0, at), slide, ...deck.slides.slice(at)];
    selectSlide(slide.id);
    commit();
  }

  function duplicateSlide(id: string) {
    const idx = deck.slides.findIndex((s) => s.id === id);
    if (idx < 0) return;
    const copy: Slide = JSON.parse(JSON.stringify(deck.slides[idx]));
    copy.id = newId('slide');
    copy.elements = (copy.elements ?? []).map((e) => ({ ...e, id: newId(e.kind) }));
    deck.slides = [...deck.slides.slice(0, idx + 1), copy, ...deck.slides.slice(idx + 1)];
    selectSlide(copy.id);
    commit();
  }

  function deleteSlide(id: string) {
    if (deck.slides.length <= 1) return; // always keep at least one slide
    const idx = deck.slides.findIndex((s) => s.id === id);
    deck.slides = deck.slides.filter((s) => s.id !== id);
    if (activeSlideId === id) selectSlide(deck.slides[Math.max(0, idx - 1)]?.id);
    commit();
  }

  function moveSlide(id: string, dir: -1 | 1) {
    const idx = deck.slides.findIndex((s) => s.id === id);
    const swapWith = idx + dir;
    if (swapWith < 0 || swapWith >= deck.slides.length) return;
    const next = [...deck.slides];
    [next[idx], next[swapWith]] = [next[swapWith], next[idx]];
    deck.slides = next;
    commit();
  }

  function setLayout(layout: SlideLayoutId) {
    if (!activeSlide) return;
    activeSlide.layout = layout;
    delete activeSlide.rects?.title;
    delete activeSlide.rects?.body;
    delete activeSlide.rects?.body2;
    commit();
  }

  function setBackground(color: string) {
    if (!activeSlide) return;
    activeSlide.background = color;
    commit();
  }

  // ── selection, geometry, text ───────────────────────────────────────
  type SelKind = 'title' | 'body' | 'body2' | 'table' | 'text' | 'image' | null;

  let selKind = $derived.by<SelKind>(() => {
    if (!activeSlide || !selectedId) return null;
    if (selectedId === 'title' || selectedId === 'body' || selectedId === 'body2' || selectedId === 'table') return selectedId;
    const el = activeSlide.elements?.find((e) => e.id === selectedId);
    return el ? el.kind : null;
  });

  let selElement = $derived(activeSlide?.elements?.find((e) => e.id === selectedId));
  let selTextBox = $derived(selElement && selElement.kind === 'text' ? selElement : undefined);

  function onRect(id: string, rect: Rect) {
    if (!activeSlide) return;
    const r = clampRect(rect);
    if (id === 'title' || id === 'body' || id === 'body2' || id === 'table') {
      activeSlide.rects = { ...(activeSlide.rects ?? {}), [id]: r };
    } else {
      const el = activeSlide.elements?.find((e) => e.id === id);
      if (!el) return;
      el.x = r.x; el.y = r.y; el.w = r.w; el.h = r.h;
    }
    commit();
  }

  function onText(id: string, value: string) {
    if (!activeSlide) return;
    if (id === 'title') activeSlide.title = value.replace(/\n/g, ' ');
    else if (id === 'body') activeSlide.bullets = value.split('\n');
    else if (id === 'body2') activeSlide.bullets2 = value.split('\n');
    else {
      const el = activeSlide.elements?.find((e) => e.id === id);
      if (el && el.kind === 'text') el.text = value;
    }
    commit();
  }

  function textOf(id: string | null): string {
    if (!activeSlide || !id) return '';
    if (id === 'title') return activeSlide.title;
    if (id === 'body') return activeSlide.bullets.join('\n');
    if (id === 'body2') return (activeSlide.bullets2 ?? []).join('\n');
    const el = activeSlide.elements?.find((e) => e.id === id);
    return el && el.kind === 'text' ? el.text : '';
  }

  function onRemove(id: string) {
    if (!activeSlide) return;
    if (id === 'table') {
      delete activeSlide.table;
      if (activeSlide.rects) delete activeSlide.rects.table;
    } else if (activeSlide.elements?.some((e) => e.id === id)) {
      activeSlide.elements = activeSlide.elements.filter((e) => e.id !== id);
    } else {
      // placeholders cannot be removed (change the layout instead) — clear their text
      onText(id, '');
      return;
    }
    selectedId = null;
    commit();
  }

  function stack(dir: -1 | 1) {
    if (!activeSlide?.elements || !selectedId) return;
    const list = [...activeSlide.elements];
    const i = list.findIndex((e) => e.id === selectedId);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    activeSlide.elements = list;
    commit();
  }

  function addTextBox() {
    if (!activeSlide) return;
    const box = newTextBox(activeSlide);
    activeSlide.elements = [...(activeSlide.elements ?? []), box];
    selectedId = box.id;
    commit();
  }

  function addImage(e: Event) {
    if (!activeSlide) return;
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ''; // allow picking the same file again later
    if (!file) return;
    const slideId = activeSlide.id;
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result !== 'string') return;
      const src = reader.result;
      const img = new Image();
      const place = (aspect: number) => {
        const slide = deck.slides.find((s) => s.id === slideId);
        if (!slide) return;
        const box = newImageBox(slide, src, aspect);
        slide.elements = [...(slide.elements ?? []), box];
        selectedId = box.id;
        commit();
      };
      img.onload = () => place(img.naturalWidth > 0 && img.naturalHeight > 0 ? img.naturalWidth / img.naturalHeight : 4 / 3);
      img.onerror = () => place(4 / 3);
      img.src = src;
    };
    reader.readAsDataURL(file);
  }

  // ── text style ──────────────────────────────────────────────────────
  type Resolved = ReturnType<typeof resolveStyle>;

  let selStyle = $derived.by<Resolved | null>(() => {
    if (!activeSlide) return null;
    if (selKind === 'title' || selKind === 'body' || selKind === 'body2') return resolveStyle(activeSlide, selKind);
    if (selTextBox) return resolveBoxStyle(activeSlide, selTextBox);
    return null;
  });

  function setStyle(patch: Partial<SlideTextStyle>) {
    if (!activeSlide) return;
    if (selKind === 'title') activeSlide.titleStyle = { ...(activeSlide.titleStyle ?? {}), ...patch };
    else if (selKind === 'body') activeSlide.bodyStyle = { ...(activeSlide.bodyStyle ?? {}), ...patch };
    else if (selKind === 'body2') activeSlide.body2Style = { ...(activeSlide.body2Style ?? {}), ...patch };
    else if (selTextBox) selTextBox.style = { ...selTextBox.style, ...patch };
    else return;
    commit();
  }

  function setFill(color: string | undefined) {
    if (!selTextBox) return;
    if (color) selTextBox.fill = color; else delete (selTextBox as SlideTextBox).fill;
    commit();
  }

  const onFont = (e: Event) => setStyle({ fontFamily: (e.currentTarget as HTMLSelectElement).value });
  const onSize = (e: Event) => {
    const n = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(n) && n >= 6 && n <= 200) setStyle({ fontSize: Math.round(n) });
  };
  const onColor = (e: Event) => setStyle({ color: (e.currentTarget as HTMLInputElement).value });
  const onFillInput = (e: Event) => setFill((e.currentTarget as HTMLInputElement).value);
  const onPanelText = (e: Event) => { if (selectedId) onText(selectedId, (e.currentTarget as HTMLTextAreaElement).value); };

  // ── speaker notes & table ───────────────────────────────────────────
  function onNotesInput(e: Event) {
    if (!activeSlide) return;
    const v = (e.currentTarget as HTMLTextAreaElement).value;
    if (v) activeSlide.notes = v; else delete activeSlide.notes;
    commit();
  }
  function addTable() {
    if (!activeSlide) return;
    activeSlide.table = [['Header 1', 'Header 2', 'Header 3'], ['', '', ''], ['', '', '']];
    selectedId = 'table';
    commit();
  }
  function removeTable() { onRemove('table'); }
  function setCell(r: number, c: number, v: string) {
    if (!activeSlide?.table) return;
    activeSlide.table[r][c] = v;
    commit();
  }
  function onCellInput(e: Event, r: number, c: number) { setCell(r, c, (e.currentTarget as HTMLInputElement).value); }
  function addRow() {
    if (!activeSlide?.table) return;
    activeSlide.table = [...activeSlide.table, Array(activeSlide.table[0]?.length ?? 1).fill('')];
    commit();
  }
  function addCol() {
    if (!activeSlide?.table) return;
    activeSlide.table = activeSlide.table.map((row) => [...row, '']);
    commit();
  }
  function delRow() {
    if (!activeSlide?.table || activeSlide.table.length <= 1) return;
    activeSlide.table = activeSlide.table.slice(0, -1);
    commit();
  }
  function delCol() {
    if (!activeSlide?.table || (activeSlide.table[0]?.length ?? 0) <= 1) return;
    activeSlide.table = activeSlide.table.map((row) => row.slice(0, -1));
    commit();
  }

  // ── transitions ─────────────────────────────────────────────────────
  const TRANSITION_TYPES = Object.keys(TRANSITION_LABELS) as TransitionType[];
  const DIRECTIONS: { v: 'l' | 'r' | 'u' | 'd'; label: string }[] = [
    { v: 'l', label: 'Towards left' }, { v: 'r', label: 'Towards right' }, { v: 'u', label: 'Upwards' }, { v: 'd', label: 'Downwards' },
  ];

  let tr = $derived(activeSlide?.transition);

  function patchTransition(patch: Partial<NonNullable<Slide['transition']>>) {
    if (!activeSlide) return;
    const next = { type: 'none' as TransitionType, ...(activeSlide.transition ?? {}), ...patch };
    if (next.type === 'none' && !next.advanceAfterMs) delete activeSlide.transition;
    else activeSlide.transition = next;
    commit();
  }
  const onTransType = (e: Event) => patchTransition({ type: (e.currentTarget as HTMLSelectElement).value as TransitionType });
  const onTransDir = (e: Event) => patchTransition({ direction: (e.currentTarget as HTMLSelectElement).value as 'l' | 'r' | 'u' | 'd' });
  const onTransDur = (e: Event) => patchTransition({ durationMs: Math.round(Number((e.currentTarget as HTMLInputElement).value) * 1000) });
  function onAutoAdvance(e: Event) {
    const on = (e.currentTarget as HTMLInputElement).checked;
    patchTransition({ advanceAfterMs: on ? 5000 : undefined });
  }
  function onAdvanceSeconds(e: Event) {
    const n = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(n) && n > 0) patchTransition({ advanceAfterMs: Math.round(n * 1000) });
  }
  function applyTransitionToAll() {
    if (!activeSlide) return;
    const t = activeSlide.transition ? { ...activeSlide.transition } : undefined;
    for (const s of deck.slides) {
      if (t) s.transition = { ...t }; else delete s.transition;
    }
    commit();
  }

  // ── presenting ──────────────────────────────────────────────────────
  function clearAdvance() {
    if (advanceTimer) { clearTimeout(advanceTimer); advanceTimer = undefined; }
  }

  async function showSlide(id: string | undefined) {
    if (!id) return;
    clearAdvance();
    activeSlideId = id;
    await tick();
    const t = activeSlide?.transition;
    const keyframes = transitionKeyframes(t);
    const reduce = typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    if (keyframes && presentWrap && !reduce) {
      presentWrap.animate(keyframes, { duration: transitionDuration(t), easing: 'ease-in-out' });
    }
    if (presenting && t?.advanceAfterMs && activeIndex < deck.slides.length - 1) {
      advanceTimer = setTimeout(nextSlide, t.advanceAfterMs + (keyframes ? transitionDuration(t) : 0));
    }
  }

  function startPresenting(fromCurrent = false) {
    selectedId = null;
    presenting = true;
    void showSlide(fromCurrent ? activeSlide?.id : deck.slides[0]?.id);
  }
  function stopPresenting() {
    clearAdvance();
    presenting = false;
  }
  function nextSlide() {
    if (activeIndex < deck.slides.length - 1) void showSlide(deck.slides[activeIndex + 1].id);
  }
  function prevSlide() {
    if (activeIndex > 0) void showSlide(deck.slides[activeIndex - 1].id);
  }
  onDestroy(clearAdvance);

  function handlePresentKeydown(e: KeyboardEvent) {
    if (e.key === 'n' || e.key === 'N') { showNotes = !showNotes; return; }
    if (e.key === 'Escape') stopPresenting();
    else if (e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown' || e.key === 'Enter') { e.preventDefault(); nextSlide(); }
    else if (e.key === 'ArrowLeft' || e.key === 'PageUp' || e.key === 'Backspace') { e.preventDefault(); prevSlide(); }
    else if (e.key === 'Home') void showSlide(deck.slides[0]?.id);
    else if (e.key === 'End') void showSlide(deck.slides[deck.slides.length - 1]?.id);
  }

  const btn = 'p-1.5 rounded-md hover:bg-white/10 text-slate-300 transition-colors disabled:opacity-30';
  const btnOn = 'p-1.5 rounded-md bg-blue-500/30 text-blue-200';
  const field = 'bg-slate-900/60 border border-white/10 rounded-md px-2 py-1 text-xs text-slate-200 outline-none focus:border-blue-500';
</script>

<svelte:window onkeydown={presenting ? handlePresentKeydown : undefined} />

{#if presenting && activeSlide}
  <div class="fixed inset-0 z-50 bg-black flex items-center justify-center overflow-hidden">
    <button class="absolute top-4 right-4 z-10 p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors" onclick={stopPresenting} title="Exit (Esc)">
      <X size={18} />
    </button>
    <button class="absolute left-4 top-1/2 -translate-y-1/2 z-10 p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors disabled:opacity-20" onclick={prevSlide} disabled={activeIndex === 0}>
      <ChevronLeft size={20} />
    </button>
    <button class="absolute right-4 top-1/2 -translate-y-1/2 z-10 p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors disabled:opacity-20" onclick={nextSlide} disabled={activeIndex === deck.slides.length - 1}>
      <ChevronRight size={20} />
    </button>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div bind:this={presentWrap} style="width:min(100vw, calc(100vh * 16 / 9));" onclick={nextSlide}>
      <SlideCanvas slide={activeSlide} />
    </div>
    {#if showNotes && activeSlide.notes}
      <div class="absolute bottom-10 left-1/2 -translate-x-1/2 max-w-2xl w-[90%] max-h-40 overflow-y-auto rounded-xl bg-black/70 text-slate-100 text-sm p-4 whitespace-pre-wrap z-10">{activeSlide.notes}</div>
    {/if}
    <div class="absolute bottom-3 text-xs text-white/40 z-10">{activeIndex + 1} / {deck.slides.length}{#if activeSlide.notes} · N = notes{/if}</div>
  </div>
{:else}
  <div class="flex-1 flex overflow-hidden min-h-0">
    <!-- Slide list -->
    <div class="w-44 shrink-0 border-r border-white/5 flex flex-col overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-white/5">
        <span class="text-xs font-medium text-slate-400">Slides</span>
        <div class="flex items-center gap-1">
          <button class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors" onclick={() => addSlide()} title="Add slide ({LAYOUTS[newLayout].label})"><Plus size={13} /></button>
          <button class="p-1 rounded hover:bg-white/10 text-emerald-400 hover:text-emerald-300 transition-colors" onclick={() => startPresenting(false)} title="Present from the start"><Play size={13} /></button>
        </div>
      </div>
      <div class="flex-1 overflow-y-auto p-2 space-y-1.5">
        {#each deck.slides as slide, i (slide.id)}
          <div
            class="group relative rounded-md border p-1.5 cursor-pointer transition-colors {slide.id === activeSlide?.id ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 hover:border-white/20'}"
            onclick={() => selectSlide(slide.id)}
            role="button" tabindex="0"
            onkeydown={(e) => { if (e.key === 'Enter') selectSlide(slide.id); }}
          >
            <div class="rounded overflow-hidden pointer-events-none"><SlideCanvas {slide} /></div>
            <div class="flex items-center justify-between mt-1">
              <span class="text-[10px] text-slate-500">{i + 1}{#if slide.transition && slide.transition.type !== 'none'} · ✦{/if}</span>
              <div class="opacity-0 group-hover:opacity-100 flex items-center gap-0.5 transition-opacity">
                <button class="p-0.5 hover:text-white text-slate-500" onclick={(e) => { e.stopPropagation(); moveSlide(slide.id, -1); }} disabled={i === 0} title="Move up"><ChevronUp size={11} /></button>
                <button class="p-0.5 hover:text-white text-slate-500" onclick={(e) => { e.stopPropagation(); moveSlide(slide.id, 1); }} disabled={i === deck.slides.length - 1} title="Move down"><ChevronDown size={11} /></button>
                <button class="p-0.5 hover:text-white text-slate-500" onclick={(e) => { e.stopPropagation(); duplicateSlide(slide.id); }} title="Duplicate"><Copy size={11} /></button>
                <button class="p-0.5 hover:text-red-400 text-slate-500" onclick={(e) => { e.stopPropagation(); deleteSlide(slide.id); }} disabled={deck.slides.length <= 1} title="Delete"><Trash2 size={11} /></button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <!-- Editing pane -->
    {#if activeSlide}
      <div class="flex-1 min-w-0 flex flex-col overflow-y-auto p-4 gap-3">
        <div class="flex items-center gap-1 flex-wrap">
          <button class={btn} onclick={addTextBox} title="Add a text box"><Type size={15} /></button>
          <label class="{btn} cursor-pointer" title="Add a picture (PNG / JPEG)">
            <ImagePlus size={15} />
            <input type="file" accept="image/png,image/jpeg" class="hidden" onchange={addImage} />
          </label>
          <button class={btn} onclick={addTable} disabled={!!activeSlide.table} title="Insert a table"><TableIcon size={15} /></button>
          <span class="w-px h-5 bg-white/10 mx-1"></span>
          <button class={btn} onclick={() => stack(1)} disabled={!selElement} title="Bring forward"><ChevronUp size={15} /></button>
          <button class={btn} onclick={() => stack(-1)} disabled={!selElement} title="Send backward"><ChevronDown size={15} /></button>
          <button class={btn} onclick={() => selectedId && onRemove(selectedId)} disabled={!selectedId} title="Delete selected (Del)"><Trash2 size={15} /></button>
          <span class="ml-auto"></span>
          <button class="px-2.5 py-1 rounded-md bg-emerald-600/80 hover:bg-emerald-500 text-white text-xs flex items-center gap-1.5" onclick={() => startPresenting(true)}><Play size={12} /> From this slide</button>
        </div>

        <div class="w-full max-w-4xl mx-auto rounded-lg overflow-hidden shadow-xl ring-1 ring-white/10">
          <SlideCanvas
            slide={activeSlide}
            interactive
            {selectedId}
            onselect={(id) => (selectedId = id)}
            onrect={onRect}
            ontext={onText}
            onremove={onRemove}
          />
        </div>
        <p class="text-[11px] text-slate-500 text-center -mt-1">Click to select · drag to move · corners to resize · double-click to edit text · arrows nudge · Del removes</p>

        <div class="w-full max-w-4xl mx-auto flex flex-col gap-3">
          {#if activeSlide.table}
            <div>
              <div class="flex items-center gap-2 mb-2">
                <span class="text-xs text-slate-500">Table</span>
                <button onclick={addRow} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">+ row</button>
                <button onclick={delRow} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">− row</button>
                <button onclick={addCol} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">+ col</button>
                <button onclick={delCol} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">− col</button>
                <button onclick={removeTable} class="ml-auto px-2 py-1 rounded-md bg-red-500/20 hover:bg-red-500/30 text-red-300 text-xs">Remove table</button>
              </div>
              <div class="overflow-x-auto">
                <table class="border-collapse">
                  <tbody>
                    {#each activeSlide.table as row, r}
                      <tr>{#each row as cell, c}
                        <td class="p-0"><input value={cell} oninput={(e) => onCellInput(e, r, c)} class="w-28 bg-slate-900/50 border border-white/10 px-2 py-1 text-xs text-slate-200 outline-none focus:border-blue-500 {r === 0 ? 'font-semibold' : ''}" /></td>
                      {/each}</tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            </div>
          {/if}
          <div>
            <span class="text-xs text-slate-500 block mb-1">Speaker notes</span>
            <textarea value={activeSlide.notes ?? ''} oninput={onNotesInput} rows="3" placeholder="Notes for the presenter (press N while presenting)…"
              class="w-full resize-y bg-slate-900/50 border border-white/10 rounded-lg p-3 text-sm text-slate-200 outline-none focus:border-blue-500"></textarea>
          </div>
        </div>
      </div>

      <!-- Properties -->
      <div class="w-60 shrink-0 border-l border-white/5 overflow-y-auto p-3 space-y-4 text-slate-300">
        <section>
          <h3 class="text-[11px] uppercase tracking-wide text-slate-500 mb-2">Layout</h3>
          <div class="grid grid-cols-2 gap-1.5">
            {#each LAYOUT_IDS as id}
              {@const L = LAYOUTS[id]}
              <button
                class="rounded-md border p-1 text-left transition-colors {(activeSlide.layout ?? 'title-content') === id ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 hover:border-white/25'}"
                onclick={() => setLayout(id)}
                onfocus={() => (newLayout = id)}
                onmouseenter={() => (newLayout = id)}
                title={L.label}
              >
                <div class="relative w-full rounded-sm bg-slate-800" style="aspect-ratio:16/9;">
                  {#each [L.title, L.body, L.body2] as r, k}
                    {#if r}<div class="absolute rounded-[1px] {k === 0 ? 'bg-slate-400/70' : 'bg-slate-600'}" style="left:{r.x}%;top:{r.y}%;width:{r.w}%;height:{r.h}%;"></div>{/if}
                  {/each}
                  {#if id === 'content-image'}<div class="absolute rounded-[1px] border border-dashed border-slate-400" style="left:56%;top:24%;width:38%;height:66%;"></div>{/if}
                </div>
                <div class="text-[10px] mt-1 leading-tight text-slate-400">{L.label}</div>
              </button>
            {/each}
          </div>
          <button class="mt-2 w-full px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-xs" onclick={() => addSlide(activeSlide?.layout ?? 'title-content')}>New slide with this layout</button>
        </section>

        <section>
          <h3 class="text-[11px] uppercase tracking-wide text-slate-500 mb-2">
            {#if selKind === 'title'}Title{:else if selKind === 'body'}Content{:else if selKind === 'body2'}Second column{:else if selKind === 'text'}Text box{:else if selKind === 'image'}Picture{:else if selKind === 'table'}Table{:else}Selection{/if}
          </h3>
          {#if !selKind}
            <p class="text-[11px] text-slate-500">Select something on the slide to change its look.</p>
          {:else if selStyle}
            <div class="space-y-2">
              <textarea value={textOf(selectedId)} oninput={onPanelText} rows={selKind === 'title' ? 2 : 4} class="w-full resize-y {field}"></textarea>
              <select value={selStyle.fontFamily} onchange={onFont} class="w-full {field}">
                {#each FONT_FAMILIES as f}<option value={f} style="font-family:{f}">{f}</option>{/each}
                {#if !FONT_FAMILIES.includes(selStyle.fontFamily as typeof FONT_FAMILIES[number])}<option value={selStyle.fontFamily}>{selStyle.fontFamily}</option>{/if}
              </select>
              <div class="flex items-center gap-2">
                <input type="number" min="6" max="200" value={selStyle.fontSize} onchange={onSize} class="w-16 {field}" title="Font size (pt)" />
                <input type="color" value={selStyle.color} oninput={onColor} class="h-7 w-9 rounded bg-transparent border border-white/10 cursor-pointer" title="Text colour" />
                {#if selTextBox}
                  <input type="color" value={selTextBox.fill ?? '#000000'} oninput={onFillInput} class="h-7 w-9 rounded bg-transparent border border-white/10 cursor-pointer" title="Box fill" />
                  <button class="text-[10px] text-slate-400 hover:text-white" onclick={() => setFill(undefined)} disabled={!selTextBox.fill} title="No fill">No fill</button>
                {/if}
              </div>
              <div class="flex items-center gap-0.5">
                <button class={selStyle.bold ? btnOn : btn} onclick={() => setStyle({ bold: !selStyle!.bold })} title="Bold"><Bold size={14} /></button>
                <button class={selStyle.italic ? btnOn : btn} onclick={() => setStyle({ italic: !selStyle!.italic })} title="Italic"><Italic size={14} /></button>
                <button class={selStyle.underline ? btnOn : btn} onclick={() => setStyle({ underline: !selStyle!.underline })} title="Underline"><Underline size={14} /></button>
                <button class={selStyle.bullets ? btnOn : btn} onclick={() => setStyle({ bullets: !selStyle!.bullets })} title="Bullets"><List size={14} /></button>
              </div>
              <div class="flex items-center gap-0.5">
                <button class={selStyle.align === 'left' ? btnOn : btn} onclick={() => setStyle({ align: 'left' })} title="Align left"><AlignLeft size={14} /></button>
                <button class={selStyle.align === 'center' ? btnOn : btn} onclick={() => setStyle({ align: 'center' })} title="Centre"><AlignCenter size={14} /></button>
                <button class={selStyle.align === 'right' ? btnOn : btn} onclick={() => setStyle({ align: 'right' })} title="Align right"><AlignRight size={14} /></button>
                <button class={selStyle.align === 'justify' ? btnOn : btn} onclick={() => setStyle({ align: 'justify' })} title="Justify"><AlignJustify size={14} /></button>
              </div>
              <p class="text-[10px] text-slate-500">Inline: <code>**bold**</code>, <code>*italic*</code></p>
            </div>
          {:else if selKind === 'image'}
            <p class="text-[11px] text-slate-500">Drag to move, drag a corner to resize. Use the arrows in the toolbar to change stacking.</p>
          {:else if selKind === 'table'}
            <p class="text-[11px] text-slate-500">Drag to move or resize the table; edit its cells below the slide.</p>
          {/if}
          {#if selectedId}
            {@const r = selElement ?? resolveRect(activeSlide, selectedId as 'title' | 'body' | 'body2' | 'table')}
            {#if r}<p class="mt-2 text-[10px] text-slate-600">x {Math.round(r.x)}% · y {Math.round(r.y)}% · {Math.round(r.w)} × {Math.round(r.h)}%</p>{/if}
          {/if}
        </section>

        <section>
          <h3 class="text-[11px] uppercase tracking-wide text-slate-500 mb-2">Background</h3>
          <div class="flex flex-wrap items-center gap-1.5">
            {#each SLIDE_BACKGROUNDS as color}
              <button
                class="w-6 h-6 rounded-full border-2 transition-transform hover:scale-110 {activeSlide.background === color ? 'border-white' : 'border-white/15'}"
                style="background:{color};"
                onclick={() => setBackground(color)}
                title={color}
              ></button>
            {/each}
            <input type="color" value={activeSlide.background} oninput={(e) => setBackground((e.currentTarget as HTMLInputElement).value)} class="h-6 w-8 rounded bg-transparent border border-white/10 cursor-pointer" title="Any colour" />
          </div>
        </section>

        <section>
          <h3 class="text-[11px] uppercase tracking-wide text-slate-500 mb-2">Transition</h3>
          <div class="space-y-2">
            <select value={tr?.type ?? 'none'} onchange={onTransType} class="w-full {field}">
              {#each TRANSITION_TYPES as t}<option value={t}>{TRANSITION_LABELS[t]}</option>{/each}
            </select>
            {#if tr && TRANSITIONS_WITH_DIRECTION.includes(tr.type)}
              <select value={tr.direction ?? 'l'} onchange={onTransDir} class="w-full {field}">
                {#each DIRECTIONS as d}<option value={d.v}>{d.label}</option>{/each}
              </select>
            {/if}
            {#if tr && tr.type !== 'none'}
              <label class="flex items-center gap-2 text-[11px] text-slate-400">
                Duration
                <input type="range" min="0.2" max="3" step="0.1" value={(tr.durationMs ?? 600) / 1000} oninput={onTransDur} class="flex-1" />
                <span class="w-8 text-right">{((tr.durationMs ?? 600) / 1000).toFixed(1)}s</span>
              </label>
            {/if}
            <label class="flex items-center gap-2 text-[11px] text-slate-400">
              <input type="checkbox" checked={!!tr?.advanceAfterMs} onchange={onAutoAdvance} />
              Advance automatically after
              <input type="number" min="1" step="1" disabled={!tr?.advanceAfterMs} value={Math.round((tr?.advanceAfterMs ?? 5000) / 1000)} onchange={onAdvanceSeconds} class="w-12 {field}" /> s
            </label>
            <button class="w-full px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-xs" onclick={applyTransitionToAll}>Apply to all slides</button>
          </div>
        </section>
      </div>
    {/if}
  </div>
{/if}
