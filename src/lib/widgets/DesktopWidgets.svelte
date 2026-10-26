<script lang="ts">
  import { onMount } from 'svelte';
  import { X, Plus, Check, GripHorizontal } from 'lucide-svelte';
  import { widgets, widgetEditMode, loadWidgets, addWidget, addCatalogWidget, installedWidgets, loadInstalledWidgets, patchWidget, removeWidget, raiseWidget, savePositions } from './widgetStore';
  import { clampToViewport, snap, MIN_SIZE, BUILTIN_WIDGET_TYPES, WIDGET_LABEL, type WidgetType, type StoreWidgetType, type WidgetInstance } from './widgetModel';
  import { t } from '../stores/language';
  import ClockWidget from './ClockWidget.svelte';
  import WeatherWidget from './WeatherWidget.svelte';
  import NotesWidget from './NotesWidget.svelte';
  import SystemWidget from './SystemWidget.svelte';
  import CalendarWidget from './CalendarWidget.svelte';
  import WorldClockWidget from './WorldClockWidget.svelte';
  import CountdownWidget from './CountdownWidget.svelte';
  import QuoteWidget from './QuoteWidget.svelte';
  import LinksWidget from './LinksWidget.svelte';
  import TodoWidget from './TodoWidget.svelte';
  import TimerWidget from './TimerWidget.svelte';

  const COMPONENTS = { clock: ClockWidget, weather: WeatherWidget, notes: NotesWidget, system: SystemWidget, calendar: CalendarWidget,
    worldclock: WorldClockWidget, countdown: CountdownWidget, quote: QuoteWidget, links: LinksWidget, todo: TodoWidget, timer: TimerWidget } as const;

  const label = (w: WidgetInstance) => ($installedWidgets.find((e) => e.id === w.storeId)?.name) ?? (w.type in WIDGET_LABEL ? WIDGET_LABEL[w.type as StoreWidgetType] : $t('widgets.' + w.type));

  let vw = $state(typeof window === 'undefined' ? 1920 : window.innerWidth);
  let vh = $state(typeof window === 'undefined' ? 1080 : window.innerHeight);
  let editing = $derived($widgetEditMode);

  // live drag / resize state (kept local; committed to the store on release)
  let gesture = $state<null | { id: string; mode: 'move' | 'resize'; sx: number; sy: number; orig: WidgetInstance; live: WidgetInstance }>(null);

  onMount(() => {
    loadWidgets();
    loadInstalledWidgets();
    const onResize = () => { vw = window.innerWidth; vh = window.innerHeight; };
    window.addEventListener('resize', onResize);
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape' && $widgetEditMode) widgetEditMode.set(false); };
    window.addEventListener('keydown', onKey);
    return () => { window.removeEventListener('resize', onResize); window.removeEventListener('keydown', onKey); };
  });

  function begin(e: PointerEvent, w: WidgetInstance, mode: 'move' | 'resize') {
    if (!editing) return;
    e.preventDefault(); e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    raiseWidget(w.id);
    gesture = { id: w.id, mode, sx: e.clientX, sy: e.clientY, orig: w, live: w };
  }

  function move(e: PointerEvent) {
    if (!gesture) return;
    const dx = e.clientX - gesture.sx, dy = e.clientY - gesture.sy;
    const o = gesture.orig;
    const next = gesture.mode === 'move'
      ? { ...o, x: snap(o.x + dx), y: snap(o.y + dy) }
      : { ...o, w: Math.max(MIN_SIZE.w, snap(o.w + dx)), h: Math.max(MIN_SIZE.h, snap(o.h + dy)) };
    gesture = { ...gesture, live: clampToViewport(next, vw, vh) };
  }

  function end() {
    if (!gesture) return;
    const { id, live } = gesture;
    patchWidget(id, { x: live.x, y: live.y, w: live.w, h: live.h });
    gesture = null;
    savePositions();
  }

  const geom = (w: WidgetInstance) => (gesture && gesture.id === w.id ? gesture.live : clampToViewport(w, vw, vh));
</script>

<!-- Layer sits under the desktop icons; only the widgets themselves take pointer events. -->
<div class="absolute inset-0 pointer-events-none z-0" data-desktop-widgets>
  {#each $widgets as w (w.id)}
    {@const g = geom(w)}
    {@const Comp = COMPONENTS[w.type]}
    <div
      class="absolute pointer-events-auto rounded-2xl text-white overflow-hidden border shadow-xl backdrop-blur-xl
             {w.type === 'notes' ? 'bg-amber-900/40' : 'bg-slate-900/55'}
             {editing ? 'border-blue-400/60 ring-1 ring-blue-400/30' : 'border-white/10'}"
      style="left:{g.x}px; top:{g.y}px; width:{g.w}px; height:{g.h}px;"
      role="group"
      aria-label={label(w)}
      onmousedown={(e) => e.stopPropagation()}
      onclick={(e) => e.stopPropagation()}
      oncontextmenu={(e) => e.stopPropagation()}
      ondblclick={(e) => e.stopPropagation()}
    >
      <Comp widget={w} {editing} />

      {#if editing}
        <!-- drag handle -->
        <div
          class="absolute inset-x-0 top-0 h-6 flex items-center justify-center cursor-move bg-black/30 touch-none"
          role="presentation"
          onpointerdown={(e) => begin(e, w, 'move')}
          onpointermove={move} onpointerup={end} onpointercancel={end}
        ><GripHorizontal size={14} class="text-slate-300" /></div>
        <button
          class="absolute top-0.5 right-0.5 w-5 h-5 rounded-full bg-red-600/80 hover:bg-red-500 flex items-center justify-center"
          title={$t('widgets.remove')} aria-label={$t('widgets.remove')}
          onclick={() => removeWidget(w.id)}
        ><X size={11} /></button>
        <!-- resize grip -->
        <div
          class="absolute bottom-0 right-0 w-5 h-5 cursor-nwse-resize touch-none"
          role="presentation"
          onpointerdown={(e) => begin(e, w, 'resize')}
          onpointermove={move} onpointerup={end} onpointercancel={end}
        ><svg viewBox="0 0 10 10" class="w-full h-full text-blue-300"><path d="M9 3 3 9M9 6 6 9" stroke="currentColor" stroke-width="1.2" fill="none" /></svg></div>
      {/if}
    </div>
  {/each}

  {#if editing}
    <div class="absolute bottom-20 left-1/2 -translate-x-1/2 max-w-[94vw] flex-wrap justify-center pointer-events-auto flex items-center gap-2 px-3 py-2 rounded-2xl bg-slate-900/90 border border-white/10 backdrop-blur-xl shadow-2xl text-white text-xs"
         role="toolbar" tabindex="-1" aria-label={$t('widgets.palette')}
         onmousedown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
      <span class="text-slate-400 mr-1">{$t('widgets.add')}</span>
      {#each BUILTIN_WIDGET_TYPES as type (type)}
        <button onclick={() => addWidget(type as WidgetType)} class="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-white/10 hover:bg-blue-600 transition-colors">
          <Plus size={11} /> {$t('widgets.' + type)}
        </button>
      {/each}
      {#each $installedWidgets as entry (entry.id)}
        <button onclick={() => addCatalogWidget(entry)} title={entry.description} class="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-white/10 hover:bg-blue-600 transition-colors">
          <span>{entry.icon}</span> {entry.name}
        </button>
      {/each}
      <button onclick={() => widgetEditMode.set(false)} class="ml-2 flex items-center gap-1 px-3 py-1 rounded-lg bg-blue-600 hover:bg-blue-500 font-medium">
        <Check size={12} /> {$t('widgets.done')}
      </button>
    </div>
  {/if}
</div>
