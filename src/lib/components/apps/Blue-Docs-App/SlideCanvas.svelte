<script lang="ts">
  /**
   * One slide, drawn on a fixed 960×540 logical stage that is scaled to the
   * width it is given — so thumbnails, the editing canvas and the full-screen
   * presentation all share exactly the same rendering (what you edit is what
   * you present and what `pptxFile.ts` exports).
   *
   * With `interactive` the blocks (title / body placeholders, the table, free
   * text boxes and pictures) can be selected, dragged, resized with the corner
   * handles, nudged with the arrow keys and edited in place (double-click or
   * Enter). Moves are applied once, when the pointer is released (`onrect`), so
   * dragging never re-serialises the whole deck on every pixel.
   */
  import { tick } from 'svelte';
  import type { Rect, Slide } from './types';
  import {
    SLIDE_H, SLIDE_W, clampRect, cssFontStack, resolveBoxStyle, resolveRect, resolveStyle, round1,
  } from './slideModel';
  import { parseInlineFormatting } from './pptxFile';

  interface Props {
    slide: Slide;
    interactive?: boolean;
    selectedId?: string | null;
    onselect?: (id: string | null) => void;
    onrect?: (id: string, rect: Rect) => void;
    ontext?: (id: string, value: string) => void;
    onremove?: (id: string) => void;
  }

  let { slide, interactive = false, selectedId = null, onselect, onrect, ontext, onremove }: Props = $props();

  type Style = ReturnType<typeof resolveStyle>;
  type Block =
    | { id: string; kind: 'text'; role: 'title' | 'body' | 'body2' | 'box'; rect: Rect; style: Style; lines: string[]; raw: string; anchor: string; fill?: string; hint: string }
    | { id: string; kind: 'image'; rect: Rect; src: string }
    | { id: string; kind: 'table'; rect: Rect; rows: string[][]; color: string };

  let width = $state(0);
  let wrapEl = $state<HTMLDivElement>();
  let editingId = $state<string | null>(null);
  let drag = $state<{ id: string; mode: string; startX: number; startY: number; start: Rect; rect: Rect; moved: boolean } | null>(null);

  let scale = $derived(width > 0 ? width / SLIDE_W : 0.5);

  let blocks = $derived.by<Block[]>(() => {
    const out: Block[] = [];
    const placeholder = (key: 'title' | 'body' | 'body2', raw: string, hint: string) => {
      const rect = resolveRect(slide, key);
      if (!rect) return;
      const lines = key === 'title' ? (raw ? [raw] : []) : raw.split('\n').filter((l) => l.trim().length > 0);
      out.push({ id: key, kind: 'text', role: key, rect, style: resolveStyle(slide, key), lines, raw, anchor: key === 'title' ? 'center' : 'flex-start', hint });
    };
    placeholder('title', slide.title, 'Click to add a title');
    placeholder('body', slide.bullets.join('\n'), 'Double-click to add text');
    placeholder('body2', (slide.bullets2 ?? []).join('\n'), 'Double-click to add text');

    if (slide.table?.length) {
      const rect = resolveRect(slide, 'table');
      if (rect) out.push({ id: 'table', kind: 'table', rect, rows: slide.table, color: resolveStyle(slide, 'body').color });
    }
    for (const el of slide.elements ?? []) {
      if (el.kind === 'text') {
        out.push({
          id: el.id, kind: 'text', role: 'box', rect: { x: el.x, y: el.y, w: el.w, h: el.h },
          style: resolveBoxStyle(slide, el), lines: el.text.split('\n'), raw: el.text, anchor: 'flex-start', fill: el.fill, hint: 'Text',
        });
      } else {
        out.push({ id: el.id, kind: 'image', rect: { x: el.x, y: el.y, w: el.w, h: el.h }, src: el.src });
      }
    }
    return out;
  });

  const rectOf = (b: Block): Rect => (drag && drag.id === b.id ? drag.rect : b.rect);

  function startDrag(e: PointerEvent, b: Block, mode: string) {
    if (!interactive) return;
    e.stopPropagation();
    onselect?.(b.id);
    if (editingId === b.id) return;
    editingId = null;
    e.preventDefault();
    wrapEl?.focus({ preventScroll: true });
    drag = { id: b.id, mode, startX: e.clientX, startY: e.clientY, start: { ...b.rect }, rect: { ...b.rect }, moved: false };
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag || !wrapEl) return;
    const box = wrapEl.getBoundingClientRect();
    if (box.width === 0 || box.height === 0) return;
    const dx = ((e.clientX - drag.startX) / box.width) * 100;
    const dy = ((e.clientY - drag.startY) / box.height) * 100;
    let { x, y, w, h } = drag.start;
    switch (drag.mode) {
      case 'move': x += dx; y += dy; break;
      case 'se': w += dx; h += dy; break;
      case 'sw': x += dx; w -= dx; h += dy; break;
      case 'ne': y += dy; w += dx; h -= dy; break;
      case 'nw': x += dx; y += dy; w -= dx; h -= dy; break;
    }
    const next = clampRect({ x: round1(x), y: round1(y), w: round1(w), h: round1(h) });
    drag.rect = next;
    if (Math.abs(dx) + Math.abs(dy) > 0.2) drag.moved = true;
  }

  function onPointerUp() {
    if (drag?.moved) onrect?.(drag.id, drag.rect);
    drag = null;
  }

  function beginEdit(b: Block) {
    if (!interactive || b.kind !== 'text') return;
    onselect?.(b.id);
    editingId = b.id;
  }

  function onKeydown(e: KeyboardEvent) {
    if (!interactive || editingId || !selectedId) return;
    const b = blocks.find((x) => x.id === selectedId);
    if (!b) return;
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      onremove?.(b.id);
    } else if (e.key === 'Enter' && b.kind === 'text') {
      e.preventDefault();
      editingId = b.id;
    } else if (e.key === 'Escape') {
      onselect?.(null);
    } else if (e.key.startsWith('Arrow')) {
      e.preventDefault();
      const step = e.shiftKey ? 5 : 1;
      const dx = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0;
      const dy = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0;
      onrect?.(b.id, clampRect({ ...b.rect, x: b.rect.x + dx, y: b.rect.y + dy }));
    }
  }

  function focusAtEnd(node: HTMLTextAreaElement) {
    tick().then(() => {
      node.focus();
      node.setSelectionRange(node.value.length, node.value.length);
    });
  }

  function onEditKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') { editingId = null; wrapEl?.focus({ preventScroll: true }); }
  }

  function textCss(st: Style): string {
    return `font-family:${cssFontStack(st.fontFamily)};font-size:${st.fontSize}px;line-height:1.2;color:${st.color};text-align:${st.align};`
      + `font-weight:${st.bold ? 700 : 400};font-style:${st.italic ? 'italic' : 'normal'};text-decoration:${st.underline ? 'underline' : 'none'};`;
  }

  function boxCss(b: Block): string {
    const r = rectOf(b);
    return `left:${r.x}%;top:${r.y}%;width:${r.w}%;height:${r.h}%;`;
  }
</script>

<svelte:window onpointermove={onPointerMove} onpointerup={onPointerUp} onpointercancel={onPointerUp} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={wrapEl}
  bind:clientWidth={width}
  class="relative w-full overflow-hidden select-none outline-none"
  style="aspect-ratio:16/9;background:{slide.background};"
  tabindex={interactive ? 0 : -1}
  role={interactive ? 'application' : 'img'}
  aria-label="Slide"
  onkeydown={onKeydown}
  onpointerdown={() => { if (interactive) { onselect?.(null); editingId = null; } }}
>
  <div class="absolute left-0 top-0 origin-top-left" style="width:{SLIDE_W}px;height:{SLIDE_H}px;transform:scale({scale});">
    {#each blocks as b (b.id)}
      {@const selected = interactive && selectedId === b.id}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="absolute {interactive ? 'cursor-move' : ''}"
        style="{boxCss(b)}{selected ? 'outline:2px solid #3b82f6;outline-offset:1px;' : interactive ? 'outline:1px dashed rgba(148,163,184,.35);' : ''}"
        onpointerdown={(e) => startDrag(e, b, 'move')}
        ondblclick={() => beginEdit(b)}
      >
        {#if b.kind === 'text'}
          {#if editingId === b.id}
            <textarea
              use:focusAtEnd
              value={b.raw}
              oninput={(e) => ontext?.(b.id, (e.currentTarget as HTMLTextAreaElement).value)}
              onkeydown={onEditKeydown}
              onblur={() => { if (editingId === b.id) editingId = null; }}
              onpointerdown={(e) => e.stopPropagation()}
              class="w-full h-full resize-none bg-transparent outline-none cursor-text"
              style="{textCss(b.style)}padding:4px 8px;{b.fill ? `background:${b.fill};` : ''}"
            ></textarea>
          {:else}
            <div class="w-full h-full overflow-hidden flex flex-col" style="justify-content:{b.anchor};padding:4px 8px;{textCss(b.style)}{b.fill ? `background:${b.fill};` : ''}">
              {#if b.lines.length === 0 || (b.lines.length === 1 && !b.lines[0].trim())}
                {#if interactive}<span style="opacity:.35;font-style:italic;font-weight:400;">{b.hint}</span>{/if}
              {:else}
                {#each b.lines as line}
                  <div style="{b.style.bullets ? 'padding-left:1em;text-indent:-1em;' : ''}min-height:1.2em;{b.style.align === 'justify' ? 'text-align:justify;' : ''}">
                    {#if b.style.bullets}<span>•&nbsp;</span>{/if}{#each parseInlineFormatting(line) as run}<span style="{run.bold ? 'font-weight:700;' : ''}{run.italic ? 'font-style:italic;' : ''}">{run.text}</span>{/each}
                  </div>
                {/each}
              {/if}
            </div>
          {/if}
        {:else if b.kind === 'image'}
          <img src={b.src} alt="" draggable="false" class="w-full h-full pointer-events-none" style="object-fit:fill;" />
        {:else}
          <div class="w-full h-full overflow-hidden" style="font-family:{cssFontStack(undefined)};">
            <table class="w-full h-full border-collapse" style="color:{b.color};font-size:16px;">
              <tbody>
                {#each b.rows as row, r}
                  <tr>
                    {#each row as cell}
                      <svelte:element this={r === 0 ? 'th' : 'td'} style="border:1px solid {b.color}55;padding:2px 8px;{r === 0 ? 'font-weight:700;' : ''}text-align:left;">{cell}</svelte:element>
                    {/each}
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

        {#if selected && editingId !== b.id}
          {#each ['nw', 'ne', 'sw', 'se'] as corner}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="absolute bg-white border-2 border-blue-500 rounded-sm"
              style="width:14px;height:14px;{corner.includes('n') ? 'top:-8px;' : 'bottom:-8px;'}{corner.includes('w') ? 'left:-8px;' : 'right:-8px;'}cursor:{corner === 'nw' || corner === 'se' ? 'nwse-resize' : 'nesw-resize'};"
              onpointerdown={(e) => startDrag(e, b, corner)}
            ></div>
          {/each}
        {/if}
      </div>
    {/each}
  </div>
</div>
