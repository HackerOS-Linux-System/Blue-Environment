<script lang="ts">
  import { run as run_1, stopPropagation } from 'svelte/legacy';

  /**
   * Presentation editor for Blue Docs — replaces the previous
   * "Presentation editor coming soon" placeholder.
   *
   * ── What this covers ─────────────────────────────────────────────
   * A deliberately minimal slide model (see `Presentation`/`Slide` in
   * types.ts): each slide is a title, a flat list of bullet points
   * (supporting simple `**bold**`/`*italic*` inline markdown, parsed by
   * `pptxFile.ts`'s `parseInlineFormatting` — shared with the real
   * `.pptx` export path so the same formatting round-trips), a
   * background color, and zero or more images. Slide list sidebar with
   * reorder/add/delete, an editing pane for the active slide, and a
   * fullscreen "Present" mode that steps through slides with the
   * keyboard and renders bold/italic runs and images for real. Content
   * round-trips through `DocFile.content` as JSON, the same
   * "plain-string-field, structured-JSON-inside" convention
   * SpreadsheetEditor.svelte already uses.
   *
   * ── What this doesn't cover ──────────────────────────────────────
   * - No color/font/size/link/underline formatting beyond bold/italic,
   *   no multiple text boxes, no slide layouts/templates, no
   *   transitions — a genuinely minimal outline-style deck, not a
   *   PowerPoint-equivalent editor.
   * - No per-image position/size control — images are placed in a
   *   simple fixed-size row under the bullet text (both here and in
   *   the exported `.pptx` — see `pptxFile.ts`'s doc for the same
   *   limitation on the export side).
   * - Real `.pptx` import/export now exists (`pptxFile.ts`, wired
   *   through `document.ts`'s `openDocFromPath`/`saveDoc`) but only
   *   round-trips what this model can represent — see that module's
   *   doc for the full list of what's lossy or unsupported.
   * - Print-to-PDF (Ctrl+P / the toolbar's print button, both already
   *   wired at the BlueDocsApp.svelte level) prints the currently
   *   active slide only, not the full deck one-slide-per-page — nice-
   *   to-have, not attempted here.
   */
  import { createEventDispatcher } from 'svelte';
  import { Plus, Trash2, ChevronUp, ChevronDown, Play, X, ChevronLeft, ChevronRight, Image as ImageIcon } from 'lucide-svelte';
  import type { Presentation, Slide } from './types';
  import { emptyPresentation } from './types';
  import { parseInlineFormatting } from './pptxFile';

  interface Props {
    content: string;
  }

  let { content }: Props = $props();

  const dispatch = createEventDispatcher<{ change: string }>();

  const BACKGROUNDS = ['#0f172a', '#1e1b4b', '#052e2b', '#3f1d1d', '#1c1917', '#0c4a6e'];

  function parse(raw: string): Presentation {
    try {
      const parsed = JSON.parse(raw);
      if (parsed && Array.isArray(parsed.slides) && parsed.slides.length > 0) return parsed;
    } catch { /* fall through to a fresh deck below */ }
    return emptyPresentation();
  }

  let deck = $state<Presentation>(parse(content));
  let activeSlideId = $state(deck.slides[0]?.id);
  let presenting = $state(false);
  let showNotes = $state(false);

  // Re-parse if the parent ever swaps in different content wholesale
  // (e.g. switching tabs to a different presentation doc) — mirrors
  // SpreadsheetEditor.svelte's own `$: if (content) ...` re-sync
  // pattern rather than only reading `content` once at mount.
  let lastContent = $state(content);
  run_1(() => {
    if (content !== lastContent) {
      lastContent = content;
      deck = parse(content);
      if (!deck.slides.some((s) => s.id === activeSlideId)) activeSlideId = deck.slides[0]?.id;
    }
  });

  let activeSlide = $derived(deck.slides.find((s) => s.id === activeSlideId) ?? deck.slides[0]);
  let activeIndex = $derived(deck.slides.findIndex((s) => s.id === activeSlideId));

  function commit() {
    deck = deck; // trigger reactivity for in-place mutations below
    const json = JSON.stringify(deck);
    lastContent = json;
    dispatch('change', json);
  }

  function addSlide() {
    const slide: Slide = {
      id: `slide-${Date.now()}`,
      title: `Slide ${deck.slides.length + 1}`,
      bullets: [],
      background: BACKGROUNDS[deck.slides.length % BACKGROUNDS.length],
      images: [],
    };
    deck.slides = [...deck.slides, slide];
    activeSlideId = slide.id;
    commit();
  }

  function deleteSlide(id: string) {
    if (deck.slides.length <= 1) return; // always keep at least one slide
    const idx = deck.slides.findIndex((s) => s.id === id);
    deck.slides = deck.slides.filter((s) => s.id !== id);
    if (activeSlideId === id) {
      activeSlideId = deck.slides[Math.max(0, idx - 1)]?.id;
    }
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

  function updateTitle(value: string) {
    if (!activeSlide) return;
    activeSlide.title = value;
    commit();
  }

  function updateBulletsText(value: string) {
    if (!activeSlide) return;
    activeSlide.bullets = value.split('\n');
    commit();
  }

  // Named handlers instead of inline `(e) => (e.currentTarget as X).value`
  // in the markup below — Svelte's template-expression parser doesn't
  // reliably accept a TS `as` type-assertion inside an inline arrow
  // function the way plain `<script>` code does, which surfaced as a
  // "svelte-check: Unexpected token" parse error at the call site.
  // Casting inside a real script-block function avoids that entirely.
  function onTitleInput(e: Event) {
    updateTitle((e.currentTarget as HTMLInputElement).value);
  }
  function onBulletsInput(e: Event) {
    updateBulletsText((e.currentTarget as HTMLTextAreaElement).value);
  }

  function setBackground(color: string) {
    if (!activeSlide) return;
    activeSlide.background = color;
    commit();
  }

  /** Reads a picked image file as a data URL and appends it to the
   * active slide's `images` — see this component's and pptxFile.ts's
   * doc for the "simple row, no position control" layout that results
   * both here and in an exported `.pptx`. */
  function addImage(e: Event) {
    if (!activeSlide) return;
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      if (!activeSlide || typeof reader.result !== 'string') return;
      activeSlide.images = [...activeSlide.images, reader.result];
      commit();
    };
    reader.readAsDataURL(file);
    input.value = ''; // allow picking the same file again later
  }

  function removeImage(index: number) {
    if (!activeSlide) return;
    activeSlide.images = activeSlide.images.filter((_, i) => i !== index);
    commit();
  }

  function startPresenting() {
    activeSlideId = deck.slides[0]?.id;
    presenting = true;
  }

  function nextSlide() {
    if (activeIndex < deck.slides.length - 1) activeSlideId = deck.slides[activeIndex + 1].id;
  }
  function prevSlide() {
    if (activeIndex > 0) activeSlideId = deck.slides[activeIndex - 1].id;
  }

  // ── speaker notes & table ────────────────────────────────────────────
  function onNotesInput(e: Event) {
    if (!activeSlide) return;
    const v = (e.currentTarget as HTMLTextAreaElement).value;
    if (v) activeSlide.notes = v; else delete activeSlide.notes;
    commit();
  }
  function addTable() {
    if (!activeSlide) return;
    activeSlide.table = [['Header 1', 'Header 2', 'Header 3'], ['', '', ''], ['', '', '']];
    commit();
  }
  function removeTable() {
    if (!activeSlide) return;
    delete activeSlide.table;
    commit();
  }
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

  function handlePresentKeydown(e: KeyboardEvent) {
    if (e.key === 'n' || e.key === 'N') { showNotes = !showNotes; return; }
    if (e.key === 'Escape') presenting = false;
    else if (e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') nextSlide();
    else if (e.key === 'ArrowLeft' || e.key === 'PageUp') prevSlide();
  }
</script>

<svelte:window onkeydown={presenting ? handlePresentKeydown : undefined} />

{#if presenting && activeSlide}
  <div class="fixed inset-0 z-50 flex items-center justify-center" style="background:{activeSlide.background};">
    <button class="absolute top-4 right-4 p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors" onclick={() => (presenting = false)} title="Exit (Esc)">
      <X size={18} />
    </button>
    <button class="absolute left-4 top-1/2 -translate-y-1/2 p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors disabled:opacity-20" onclick={prevSlide} disabled={activeIndex === 0}>
      <ChevronLeft size={20} />
    </button>
    <button class="absolute right-4 top-1/2 -translate-y-1/2 p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors disabled:opacity-20" onclick={nextSlide} disabled={activeIndex === deck.slides.length - 1}>
      <ChevronRight size={20} />
    </button>
    <div class="max-w-3xl w-full px-16 text-center">
      <h1 class="text-4xl font-bold text-white mb-8">
        {#each parseInlineFormatting(activeSlide.title) as run}
          <span class:font-bold={run.bold} class:italic={run.italic}>{run.text}</span>
        {/each}
      </h1>
      {#if activeSlide.bullets.filter((b) => b.trim()).length}
        <ul class="text-xl text-slate-200 space-y-3 text-left inline-block">
          {#each activeSlide.bullets.filter((b) => b.trim()) as bullet}
            <li class="flex gap-3">
              <span class="opacity-50">•</span>
              <span>
                {#each parseInlineFormatting(bullet) as run}
                  <span class:font-bold={run.bold} class:italic={run.italic}>{run.text}</span>
                {/each}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
      {#if activeSlide.table?.length}
        <table class="mx-auto mt-6 text-lg text-slate-100 border-collapse">
          {#each activeSlide.table as row, r}
            <tr>{#each row as cell}
              {#if r === 0}<th class="border border-white/25 px-4 py-1.5 font-semibold">{cell}</th>{:else}<td class="border border-white/25 px-4 py-1.5">{cell}</td>{/if}
            {/each}</tr>
          {/each}
        </table>
      {/if}
      {#if activeSlide.images.length}
        <div class="flex justify-center gap-3 mt-8">
          {#each activeSlide.images as img}
            <img src={img} alt="" class="max-h-48 rounded-lg shadow-lg object-contain" />
          {/each}
        </div>
      {/if}
    </div>
    {#if showNotes && activeSlide.notes}
      <div class="absolute bottom-10 left-1/2 -translate-x-1/2 max-w-2xl w-[90%] max-h-40 overflow-y-auto rounded-xl bg-black/70 text-slate-100 text-sm p-4 whitespace-pre-wrap">{activeSlide.notes}</div>
    {/if}
    <div class="absolute bottom-4 text-xs text-white/40">{activeIndex + 1} / {deck.slides.length}{#if activeSlide.notes} · N = notes{/if}</div>
  </div>
{:else}
  <div class="flex-1 flex overflow-hidden">
    <!-- Slide list -->
    <div class="w-48 shrink-0 border-r border-white/5 flex flex-col overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-white/5">
        <span class="text-xs font-medium text-slate-400">Slides</span>
        <div class="flex items-center gap-1">
          <button class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors" onclick={addSlide} title="Add slide"><Plus size={13} /></button>
          <button class="p-1 rounded hover:bg-white/10 text-emerald-400 hover:text-emerald-300 transition-colors" onclick={startPresenting} title="Present"><Play size={13} /></button>
        </div>
      </div>
      <div class="flex-1 overflow-y-auto p-2 space-y-1.5">
        {#each deck.slides as slide, i (slide.id)}
          <div
            class="group relative rounded-md border p-2 cursor-pointer transition-colors {slide.id === activeSlideId ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 hover:border-white/20'}"
            onclick={() => (activeSlideId = slide.id)}
            role="button" tabindex="0"
            onkeydown={(e) => { if (e.key === 'Enter') activeSlideId = slide.id; }}
          >
            <div class="h-14 rounded flex items-center justify-center text-[10px] text-white/70 mb-1 px-1 text-center overflow-hidden" style="background:{slide.background};">
              {slide.title || 'Untitled'}
            </div>
            <div class="flex items-center justify-between">
              <span class="text-[10px] text-slate-500">{i + 1}</span>
              <div class="opacity-0 group-hover:opacity-100 flex items-center gap-0.5 transition-opacity">
                <button class="p-0.5 hover:text-white text-slate-500" onclick={stopPropagation(() => moveSlide(slide.id, -1))} disabled={i === 0}><ChevronUp size={11} /></button>
                <button class="p-0.5 hover:text-white text-slate-500" onclick={stopPropagation(() => moveSlide(slide.id, 1))} disabled={i === deck.slides.length - 1}><ChevronDown size={11} /></button>
                <button class="p-0.5 hover:text-red-400 text-slate-500" onclick={stopPropagation(() => deleteSlide(slide.id))} disabled={deck.slides.length <= 1}><Trash2 size={11} /></button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <!-- Editing pane -->
    {#if activeSlide}
      <div class="flex-1 flex flex-col overflow-y-auto p-6 gap-4">
        <input
          value={activeSlide.title}
          oninput={onTitleInput}
          placeholder="Slide title"
          class="text-2xl font-bold bg-transparent border-b border-white/10 focus:border-blue-500 outline-none pb-2 text-white placeholder:text-slate-600"
        />
        <textarea
          value={activeSlide.bullets.join('\n')}
          oninput={onBulletsInput}
          placeholder={"One bullet point per line… use **bold** or *italic* for emphasis"}
          class="flex-1 resize-none bg-slate-900/50 border border-white/10 rounded-lg p-4 text-sm text-slate-200 outline-none focus:border-blue-500 leading-relaxed"
></textarea>
        <div>
          <div class="flex items-center gap-2 mb-2">
            <span class="text-xs text-slate-500">Images</span>
            <label class="cursor-pointer p-1.5 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 transition-colors">
              <ImageIcon size={13} />
              <input type="file" accept="image/png,image/jpeg" class="hidden" onchange={addImage} />
            </label>
          </div>
          {#if activeSlide.images.length}
            <div class="flex flex-wrap gap-2">
              {#each activeSlide.images as img, i}
                <div class="relative group">
                  <img src={img} alt="" class="h-16 w-24 object-cover rounded-md border border-white/10" />
                  <button
                    class="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full bg-red-500 text-white flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                    onclick={() => removeImage(i)}
                    title="Remove image"
                  >
                    <X size={11} />
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
        <div>
          <div class="flex items-center gap-2 mb-2">
            <span class="text-xs text-slate-500">Table</span>
            {#if !activeSlide.table}
              <button onclick={addTable} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">Insert table</button>
            {:else}
              <button onclick={addRow} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">+ row</button>
              <button onclick={delRow} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">− row</button>
              <button onclick={addCol} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">+ col</button>
              <button onclick={delCol} class="px-2 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs">− col</button>
              <button onclick={removeTable} class="ml-auto px-2 py-1 rounded-md bg-red-500/20 hover:bg-red-500/30 text-red-300 text-xs">Remove table</button>
            {/if}
          </div>
          {#if activeSlide.table}
            <div class="overflow-x-auto">
              <table class="border-collapse">
                {#each activeSlide.table as row, r}
                  <tr>{#each row as cell, c}
                    <td class="p-0"><input value={cell} oninput={(e) => onCellInput(e, r, c)} class="w-28 bg-slate-900/50 border border-white/10 px-2 py-1 text-xs text-slate-200 outline-none focus:border-blue-500 {r === 0 ? 'font-semibold' : ''}" /></td>
                  {/each}</tr>
                {/each}
              </table>
            </div>
          {/if}
        </div>
        <div>
          <span class="text-xs text-slate-500 block mb-1">Speaker notes</span>
          <textarea value={activeSlide.notes ?? ''} oninput={onNotesInput} rows="3" placeholder="Notes for the presenter (press N while presenting)…"
            class="w-full resize-y bg-slate-900/50 border border-white/10 rounded-lg p-3 text-sm text-slate-200 outline-none focus:border-blue-500"></textarea>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-xs text-slate-500">Background</span>
          {#each BACKGROUNDS as color}
            <button
              class="w-6 h-6 rounded-full border-2 transition-transform hover:scale-110 {activeSlide.background === color ? 'border-white' : 'border-transparent'}"
              style="background:{color};"
              onclick={() => setBackground(color)}
              title={color}
></button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
{/if}
