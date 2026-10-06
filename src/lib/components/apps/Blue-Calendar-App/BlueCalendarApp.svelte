<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ChevronLeft, ChevronRight, Plus, Trash2, CalendarDays, Clock, Repeat, Bell, MoreHorizontal, Globe, Upload, Download, Lock } from 'lucide-svelte';
  import { createCalendarStore } from './calendarStore';
  import { EVENT_COLORS, type CalendarEvent, type CalendarView, type Occurrence } from './types';
  import { addDays, fromDayNum, dayNum, groupByDate, mondayOf, occurrencesInRange, parseIso, daysInMonth } from './recurrence';
  import EventEditor from './EventEditor.svelte';
  import SubscriptionsDialog from './SubscriptionsDialog.svelte';
  import TimeGrid from './TimeGrid.svelte';
  import { t, language } from '../../../stores/language';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { pickPaths } from '../../../stores/filePicker';

  export let windowId: string;

  const store = createCalendarStore();
  const { loading, error, subscriptions, allEvents } = store;

  const WEEKDAY_KEYS = ['cal.mon', 'cal.tue', 'cal.wed', 'cal.thu', 'cal.fri', 'cal.sat', 'cal.sun'];
  const MONTH_KEYS = Array.from({ length: 12 }, (_, i) => `cal.month.${i + 1}`);
  const MAX_CHIPS = 3;

  const pad = (n: number) => String(n).padStart(2, '0');
  const isoDate = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const today = isoDate(new Date());

  let view: CalendarView = 'month';
  let cursor = today;            // the day the current view is anchored to
  let selectedDate = today;
  let menuOpen = false;
  let showSubs = false;
  let status = '';
  let statusTimer: ReturnType<typeof setTimeout>;
  let syncTimer: ReturnType<typeof setInterval>;

  function flash(text: string) { status = text; clearTimeout(statusTimer); statusTimer = setTimeout(() => (status = ''), 6000); }

  onMount(async () => {
    await store.load();
    store.syncAll();                                           // refresh subscriptions in the background
    syncTimer = setInterval(() => store.syncAll(), 30 * 60_000);
  });
  onDestroy(() => { clearInterval(syncTimer); clearTimeout(statusTimer); });

  // ── Visible range ────────────────────────────────────────────────────
  $: cur = parseIso(cursor);
  $: monthFirst = `${cur.y}-${pad(cur.m)}-01`;
  $: monthLast = `${cur.y}-${pad(cur.m)}-${pad(daysInMonth(cur.y, cur.m))}`;
  $: gridStart = mondayOf(monthFirst);
  $: gridEnd = addDays(mondayOf(monthLast), 6);
  $: weekStart = mondayOf(cursor);
  $: days = view === 'month'
    ? Array.from({ length: dayNum(gridEnd) - dayNum(gridStart) + 1 }, (_, i) => fromDayNum(dayNum(gridStart) + i))
    : view === 'week' ? Array.from({ length: 7 }, (_, i) => addDays(weekStart, i)) : [cursor];
  $: occs = occurrencesInRange($allEvents, days[0], days[days.length - 1]);
  $: byDate = groupByDate(occs);
  $: selectedOccs = byDate[selectedDate] ?? [];

  $: title = (() => {
    if (view === 'month') return `${$t(MONTH_KEYS[cur.m - 1])} ${cur.y}`;
    if (view === 'day') return new Date(cursor + 'T00:00:00').toLocaleDateString($language, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
    const a = parseIso(days[0]), b = parseIso(days[6]);
    const ma = $t(MONTH_KEYS[a.m - 1]), mb = $t(MONTH_KEYS[b.m - 1]);
    return a.m === b.m ? `${a.d}–${b.d} ${ma} ${b.y}` : a.y === b.y ? `${a.d} ${ma} – ${b.d} ${mb} ${b.y}` : `${a.d} ${ma} ${a.y} – ${b.d} ${mb} ${b.y}`;
  })();

  function shift(dir: -1 | 1) {
    if (view === 'month') {
      const total = (cur.y * 12 + (cur.m - 1)) + dir;
      cursor = `${Math.floor(total / 12)}-${pad((total % 12) + 1)}-01`;
    } else cursor = addDays(cursor, dir * (view === 'week' ? 7 : 1));
  }
  function goToday() { cursor = today; selectedDate = today; }
  function setView(v: CalendarView) { view = v; if (v !== 'month') cursor = selectedDate; }

  // ── Editor ───────────────────────────────────────────────────────────
  let editing: { event: CalendarEvent; occurrenceDate: string; source: string | null } | null = null;

  function openNew(date: string, minutes?: number) {
    selectedDate = date;
    const timed = minutes !== undefined;
    editing = {
      occurrenceDate: date, source: null,
      event: {
        id: '', title: '', date, description: '', color: EVENT_COLORS[0],
        time: timed ? `${pad(Math.floor(minutes! / 60))}:${pad(minutes! % 60)}` : null,
        durationMinutes: timed ? 60 : null, recurrence: null, exdates: [], reminderMinutes: null,
      },
    };
  }
  function openOccurrence(o: Occurrence) {
    selectedDate = o.date;
    const sub = o.event.subscriptionId ? $subscriptions.find((s) => s.id === o.event.subscriptionId) : null;
    editing = { event: o.event, occurrenceDate: o.date, source: o.event.subscriptionId ? (sub?.name ?? '—') : null };
  }
  async function onSave(e: CustomEvent) { await store.upsert(e.detail); editing = null; }
  async function onDelete() { if (editing) { await store.remove(editing.event.id); editing = null; } }
  async function onDeleteOccurrence(e: CustomEvent<string>) { if (editing) { await store.removeOccurrence(editing.event, e.detail); editing = null; } }

  // ── Import / export ──────────────────────────────────────────────────
  async function doImport() {
    menuOpen = false;
    const [path] = await pickPaths({ mode: 'file', title: $t('cal.import.pick'), filters: [{ name: 'iCalendar', extensions: ['ics'] }], rememberKey: 'calendar.import' });
    if (!path) return;
    let text = '';
    try { text = await SystemBridge.readFile(path); } catch (e: any) { flash(String(e?.message ?? e)); return; }
    const res = await store.importIcs(text);
    if (res) flash($t('cal.import.done', { added: res.added, skipped: res.skipped }));
  }
  async function doExport() {
    menuOpen = false;
    const ics = await store.exportIcs();
    if (ics === null) return;
    const d = new Date();
    const path = `HOME/Downloads/blue-calendar-${isoDate(d)}-${pad(d.getHours())}${pad(d.getMinutes())}.ics`;
    try { await SystemBridge.writeFile(path, ics); flash($t('cal.export.done', { path: path.replace('HOME', '~') })); }
    catch (e: any) { flash(String(e?.message ?? e)); }
  }

  const VIEWS: CalendarView[] = ['month', 'week', 'day'];
  const isToday = (iso: string) => iso === today;
  const inMonth = (iso: string) => iso.slice(0, 7) === monthFirst.slice(0, 7);
  const dayLabel = (iso: string) => new Date(iso + 'T00:00:00').toLocaleDateString($language, { weekday: 'long', day: 'numeric', month: 'long' });
</script>

<svelte:window on:mousedown={() => (menuOpen = false)} />

<div class="flex flex-col h-full bg-slate-900 text-white text-sm">
  <!-- Header -->
  <div class="flex items-center justify-between gap-2 px-4 py-3 border-b border-white/5 shrink-0">
    <div class="flex items-center gap-2 min-w-0">
      <CalendarDays size={18} class="text-blue-400 shrink-0" />
      <h1 class="text-base font-semibold truncate first-letter:uppercase">{title}</h1>
    </div>
    <div class="flex items-center gap-1 shrink-0">
      <div class="flex rounded-lg bg-slate-800 p-0.5 mr-1">
        {#each VIEWS as v (v)}
          <button on:click={() => setView(v)} class="px-2.5 py-1 text-xs rounded-md transition-colors {view === v ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white'}">{$t(`cal.view.${v}`)}</button>
        {/each}
      </div>
      <button on:click={goToday} class="px-2.5 py-1 text-xs bg-slate-800 hover:bg-slate-700 rounded-lg transition-colors">{$t('cal.today')}</button>
      <button on:click={() => shift(-1)} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white"><ChevronLeft size={16} /></button>
      <button on:click={() => shift(1)} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white"><ChevronRight size={16} /></button>
      <button on:click={() => openNew(selectedDate)} class="ml-1 flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs transition-colors"><Plus size={13} /> {$t('cal.new_event')}</button>
      <div class="relative" on:mousedown|stopPropagation role="presentation">
        <button on:click={() => (menuOpen = !menuOpen)} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" aria-haspopup="menu" aria-expanded={menuOpen}><MoreHorizontal size={16} /></button>
        {#if menuOpen}
          <div class="absolute right-0 top-full mt-1 w-56 bg-slate-800 border border-white/10 rounded-xl shadow-2xl p-1 z-30" role="menu">
            <button on:click={() => { menuOpen = false; showSubs = true; }} class="w-full flex items-center gap-2 px-2.5 py-2 rounded-lg text-xs text-left hover:bg-white/10" role="menuitem"><Globe size={13} /> {$t('cal.menu.subscriptions')}</button>
            <button on:click={doImport} class="w-full flex items-center gap-2 px-2.5 py-2 rounded-lg text-xs text-left hover:bg-white/10" role="menuitem"><Upload size={13} /> {$t('cal.menu.import')}</button>
            <button on:click={doExport} class="w-full flex items-center gap-2 px-2.5 py-2 rounded-lg text-xs text-left hover:bg-white/10" role="menuitem"><Download size={13} /> {$t('cal.menu.export')}</button>
          </div>
        {/if}
      </div>
    </div>
  </div>

  {#if $error}<div class="px-4 py-1.5 bg-red-500/10 text-red-300 text-xs shrink-0 break-words">{$error}</div>{/if}
  {#if status}<div class="px-4 py-1.5 bg-emerald-500/10 text-emerald-300 text-xs shrink-0 break-words">{status}</div>{/if}

  {#if view === 'month'}
    <div class="flex flex-1 overflow-hidden">
      <div class="flex-1 flex flex-col p-3 overflow-hidden min-w-0">
        <div class="grid grid-cols-7 shrink-0 mb-1">
          {#each WEEKDAY_KEYS as wk (wk)}<div class="text-center text-[10px] font-semibold text-slate-500 uppercase tracking-wider py-1">{$t(wk)}</div>{/each}
        </div>
        <div class="grid grid-cols-7 gap-1 flex-1 overflow-y-auto auto-rows-fr">
          {#each days as iso (iso)}
            {@const dayOccs = byDate[iso] ?? []}
            <div on:click={() => (selectedDate = iso)} on:dblclick={() => openNew(iso)} role="button" tabindex="0"
              on:keydown={(e) => { if (e.key === 'Enter') openNew(iso); }}
              class="flex flex-col items-stretch p-1 rounded-lg text-left min-h-[72px] border cursor-pointer transition-colors overflow-hidden
                {selectedDate === iso ? 'border-blue-500 bg-blue-500/10' : 'border-transparent hover:bg-white/5'} {inMonth(iso) ? '' : 'opacity-40'}">
              <span class="self-start text-xs font-medium mb-0.5 {isToday(iso) ? 'w-5 h-5 flex items-center justify-center rounded-full bg-blue-500 text-white' : 'text-slate-300 px-1'}">{Number(iso.slice(8))}</span>
              {#each dayOccs.slice(0, MAX_CHIPS) as o (o.key)}
                <button on:click|stopPropagation={() => openOccurrence(o)} on:dblclick|stopPropagation
                  class="flex items-center gap-1 w-full truncate rounded px-1 py-px mb-px text-[10px] text-left text-white hover:brightness-125"
                  style="background:{o.event.color}55; border-left:2px solid {o.event.color}" title={o.event.title}>
                  {#if o.event.time}<span class="text-slate-300 shrink-0">{o.event.time}</span>{/if}<span class="truncate">{o.event.title}</span>
                </button>
              {/each}
              {#if dayOccs.length > MAX_CHIPS}<span class="text-[9px] text-slate-500 px-1">{$t('cal.more', { n: dayOccs.length - MAX_CHIPS })}</span>{/if}
            </div>
          {/each}
        </div>
      </div>

      <!-- Day panel -->
      <div class="w-64 shrink-0 border-l border-white/5 flex flex-col overflow-hidden">
        <div class="px-3 py-2.5 border-b border-white/5 shrink-0"><div class="text-xs text-slate-400 first-letter:uppercase">{dayLabel(selectedDate)}</div></div>
        <div class="flex-1 overflow-y-auto p-2 space-y-1.5">
          {#if $loading}
            <div class="text-center text-slate-600 text-xs py-6">{$t('cal.loading')}</div>
          {:else if selectedOccs.length === 0}
            <div class="text-center text-slate-600 text-xs py-6">{$t('cal.no_events')}</div>
          {:else}
            {#each selectedOccs as o (o.key)}
              <div on:click={() => openOccurrence(o)} role="button" tabindex="0" on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openOccurrence(o); } }}
                class="group flex items-start gap-2 p-2 rounded-lg bg-slate-800 hover:bg-slate-700 cursor-pointer transition-colors">
                <span class="w-1 self-stretch rounded-full shrink-0" style="background:{o.event.color}" />
                <div class="min-w-0 flex-1">
                  <div class="text-xs font-medium text-white truncate">{o.event.title}</div>
                  <div class="text-[10px] text-slate-500 flex items-center gap-1.5">
                    {#if o.event.time}<Clock size={9} /> {o.event.time}{:else}{$t('cal.all_day')}{/if}
                    {#if o.event.recurrence}<Repeat size={9} />{/if}
                    {#if o.event.reminderMinutes != null}<Bell size={9} />{/if}
                    {#if o.event.subscriptionId}<Lock size={9} />{/if}
                  </div>
                </div>
                {#if !o.event.recurrence && !o.event.subscriptionId}
                  <button on:click|stopPropagation={() => store.remove(o.event.id)} class="opacity-0 group-hover:opacity-100 p-0.5 hover:text-red-400 text-slate-600 shrink-0"><Trash2 size={11} /></button>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
        <button on:click={() => openNew(selectedDate)} class="m-2 flex items-center justify-center gap-1.5 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs text-slate-300 transition-colors"><Plus size={12} /> {$t('cal.add_for_day')}</button>
      </div>
    </div>
  {:else}
    <TimeGrid {days} {byDate} {today} on:slot={(e) => openNew(e.detail.date, e.detail.minutes)} on:open={(e) => openOccurrence(e.detail)} />
  {/if}
</div>

{#if editing}
  <EventEditor event={editing.event} occurrenceDate={editing.occurrenceDate} readonlySource={editing.source}
    on:save={onSave} on:delete={onDelete} on:deleteOccurrence={onDeleteOccurrence} on:close={() => (editing = null)} />
{/if}
{#if showSubs}<SubscriptionsDialog {store} on:close={() => (showSubs = false)} />{/if}
