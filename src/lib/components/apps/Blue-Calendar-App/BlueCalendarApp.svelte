<script lang="ts">
  import { onMount } from 'svelte';
  import { ChevronLeft, ChevronRight, Plus, Trash2, X, CalendarDays, Clock } from 'lucide-svelte';
  import { createCalendarStore } from './calendarStore';
  import { EVENT_COLORS, type CalendarEvent } from './types';
  import { t } from '../../../stores/language';

  export let windowId: string;

  const store = createCalendarStore();
  const { events, loading, error } = store;

  onMount(() => { store.load(); });

  const WEEKDAY_KEYS = ['cal.mon', 'cal.tue', 'cal.wed', 'cal.thu', 'cal.fri', 'cal.sat', 'cal.sun'];
  const MONTH_KEYS = [
    'cal.month.1', 'cal.month.2', 'cal.month.3', 'cal.month.4', 'cal.month.5', 'cal.month.6',
    'cal.month.7', 'cal.month.8', 'cal.month.9', 'cal.month.10', 'cal.month.11', 'cal.month.12',
  ];

  const today = new Date();
  let viewYear = today.getFullYear();
  let viewMonth = today.getMonth(); // 0-11
  let selectedDate = isoDate(today);

  function isoDate(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }

  function prevMonth() { if (viewMonth === 0) { viewMonth = 11; viewYear--; } else { viewMonth--; } }
  function nextMonth() { if (viewMonth === 11) { viewMonth = 0; viewYear++; } else { viewMonth++; } }
  function goToday() { viewYear = today.getFullYear(); viewMonth = today.getMonth(); selectedDate = isoDate(today); }

  // Grid cells: leading days from the previous month to align the 1st
  // under the right weekday (Monday-first week), plus enough of the
  // current month to fill full weeks. No trailing-month padding cells —
  // the grid is allowed to end mid-row, matching most calendar apps.
  $: gridCells = (() => {
    const firstOfMonth = new Date(viewYear, viewMonth, 1);
    // getDay(): 0=Sun..6=Sat; convert to Monday-first (0=Mon..6=Sun).
    const leading = (firstOfMonth.getDay() + 6) % 7;
    const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate();
    const daysInPrevMonth = new Date(viewYear, viewMonth, 0).getDate();
    const cells: { date: Date; inMonth: boolean }[] = [];
    for (let i = leading - 1; i >= 0; i--) {
      cells.push({ date: new Date(viewYear, viewMonth - 1, daysInPrevMonth - i), inMonth: false });
    }
    for (let d = 1; d <= daysInMonth; d++) {
      cells.push({ date: new Date(viewYear, viewMonth, d), inMonth: true });
    }
    return cells;
  })();

  $: eventsByDate = $events.reduce<Record<string, CalendarEvent[]>>((acc, e) => {
    (acc[e.date] ??= []).push(e);
    return acc;
  }, {});

  $: selectedEvents = (eventsByDate[selectedDate] ?? []).slice().sort((a, b) => (a.time ?? '').localeCompare(b.time ?? ''));

  // --- Event editor modal --------------------------------------------------
  let editing: CalendarEvent | null = null;
  let formTitle = '';
  let formTime = '';
  let formAllDay = true;
  let formDuration = 30;
  let formDescription = '';
  let formColor: string = EVENT_COLORS[0];

  function openNewEvent(dateIso: string) {
    selectedDate = dateIso;
    editing = { id: '', title: '', date: dateIso, time: null, durationMinutes: null, description: '', color: EVENT_COLORS[0] };
    formTitle = ''; formTime = '09:00'; formAllDay = true; formDuration = 30; formDescription = ''; formColor = EVENT_COLORS[0];
  }

  function openEditEvent(ev: CalendarEvent) {
    editing = ev;
    formTitle = ev.title;
    formTime = ev.time ?? '09:00';
    formAllDay = ev.time === null;
    formDuration = ev.durationMinutes ?? 30;
    formDescription = ev.description;
    formColor = ev.color;
  }

  function closeEditor() { editing = null; }

  async function saveEvent() {
    if (!editing || !formTitle.trim()) return;
    await store.upsert({
      id: editing.id || undefined,
      title: formTitle.trim(),
      date: editing.date,
      time: formAllDay ? null : formTime,
      durationMinutes: formAllDay ? null : formDuration,
      description: formDescription,
      color: formColor,
    });
    editing = null;
  }

  async function deleteEvent(id: string) {
    await store.remove(id);
    if (editing?.id === id) editing = null;
  }

  function isToday(d: Date): boolean { return isoDate(d) === isoDate(today); }
</script>

<div class="flex flex-col h-full bg-slate-900 text-white text-sm">
  <!-- Header -->
  <div class="flex items-center justify-between px-4 py-3 border-b border-white/5 shrink-0">
    <div class="flex items-center gap-2">
      <CalendarDays size={18} class="text-blue-400" />
      <h1 class="text-base font-semibold">{$t(MONTH_KEYS[viewMonth])} {viewYear}</h1>
    </div>
    <div class="flex items-center gap-1">
      <button on:click={goToday} class="px-2.5 py-1 text-xs bg-slate-800 hover:bg-slate-700 rounded-lg transition-colors">{$t('cal.today')}</button>
      <button on:click={prevMonth} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white"><ChevronLeft size={16} /></button>
      <button on:click={nextMonth} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white"><ChevronRight size={16} /></button>
      <button on:click={() => openNewEvent(selectedDate)} class="ml-2 flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs transition-colors">
        <Plus size={13} /> {$t('cal.new_event')}
      </button>
    </div>
  </div>

  {#if $error}
    <div class="px-4 py-1.5 bg-red-500/10 text-red-300 text-xs shrink-0">{$error}</div>
  {/if}

  <div class="flex flex-1 overflow-hidden">
    <!-- Month grid -->
    <div class="flex-1 flex flex-col p-3 overflow-hidden">
      <div class="grid grid-cols-7 shrink-0 mb-1">
        {#each WEEKDAY_KEYS as wk (wk)}
          <div class="text-center text-[10px] font-semibold text-slate-500 uppercase tracking-wider py-1">{$t(wk)}</div>
        {/each}
      </div>
      <div class="grid grid-cols-7 gap-1 flex-1 overflow-y-auto auto-rows-fr">
        {#each gridCells as cell (cell.date.toISOString())}
          {@const iso = isoDate(cell.date)}
          {@const dayEvents = eventsByDate[iso] ?? []}
          <button
            on:click={() => (selectedDate = iso)}
            on:dblclick={() => openNewEvent(iso)}
            class="flex flex-col items-start p-1.5 rounded-lg text-left min-h-[64px] border transition-colors
              {selectedDate === iso ? 'border-blue-500 bg-blue-500/10' : 'border-transparent hover:bg-white/5'}
              {cell.inMonth ? '' : 'opacity-40'}">
            <span class="text-xs font-medium {isToday(cell.date) ? 'w-5 h-5 flex items-center justify-center rounded-full bg-blue-500 text-white' : 'text-slate-300'}">
              {cell.date.getDate()}
            </span>
            <div class="flex flex-wrap gap-0.5 mt-1">
              {#each dayEvents.slice(0, 4) as ev (ev.id)}
                <span class="w-1.5 h-1.5 rounded-full shrink-0" style="background:{ev.color}" title={ev.title} />
              {/each}
              {#if dayEvents.length > 4}<span class="text-[9px] text-slate-500">+{dayEvents.length - 4}</span>{/if}
            </div>
          </button>
        {/each}
      </div>
    </div>

    <!-- Day panel -->
    <div class="w-64 shrink-0 border-l border-white/5 flex flex-col overflow-hidden">
      <div class="px-3 py-2.5 border-b border-white/5 shrink-0">
        <div class="text-xs text-slate-500">{selectedDate}</div>
      </div>
      <div class="flex-1 overflow-y-auto p-2 space-y-1.5">
        {#if $loading}
          <div class="text-center text-slate-600 text-xs py-6">{$t('cal.loading')}</div>
        {:else if selectedEvents.length === 0}
          <div class="text-center text-slate-600 text-xs py-6">{$t('cal.no_events')}</div>
        {:else}
          {#each selectedEvents as ev (ev.id)}
            <div on:click={() => openEditEvent(ev)}
              class="group flex items-start gap-2 p-2 rounded-lg bg-slate-800 hover:bg-slate-700 cursor-pointer transition-colors">
              <span class="w-1 self-stretch rounded-full shrink-0" style="background:{ev.color}" />
              <div class="min-w-0 flex-1">
                <div class="text-xs font-medium text-white truncate">{ev.title}</div>
                <div class="text-[10px] text-slate-500 flex items-center gap-1">
                  {#if ev.time}<Clock size={9} /> {ev.time}{:else}{$t('cal.all_day')}{/if}
                </div>
              </div>
              <button on:click|stopPropagation={() => deleteEvent(ev.id)}
                class="opacity-0 group-hover:opacity-100 p-0.5 hover:text-red-400 text-slate-600 shrink-0"><Trash2 size={11} /></button>
            </div>
          {/each}
        {/if}
      </div>
      <button on:click={() => openNewEvent(selectedDate)}
        class="m-2 flex items-center justify-center gap-1.5 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs text-slate-300 transition-colors">
        <Plus size={12} /> {$t('cal.add_for_day')}
      </button>
    </div>
  </div>
</div>

{#if editing}
  <div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm" on:mousedown={closeEditor}>
    <div class="w-96 bg-slate-800 border border-white/10 rounded-2xl shadow-2xl p-5" on:mousedown|stopPropagation>
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-sm font-semibold text-white">{editing.id ? $t('cal.edit_event') : $t('cal.new_event')}</h3>
        <button on:click={closeEditor} class="p-1 hover:bg-white/10 rounded text-slate-500"><X size={14} /></button>
      </div>

      <div class="space-y-3">
        <input bind:value={formTitle} placeholder={$t('cal.title_placeholder')}
          class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60" />

        <label class="flex items-center gap-2 text-xs text-slate-300">
          <input type="checkbox" bind:checked={formAllDay} class="rounded" /> {$t('cal.all_day')}
        </label>

        {#if !formAllDay}
          <div class="flex gap-2">
            <div class="flex-1">
              <label class="block text-[10px] text-slate-500 mb-1">{$t('cal.time')}</label>
              <input type="time" bind:value={formTime} class="w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-xs text-white" />
            </div>
            <div class="flex-1">
              <label class="block text-[10px] text-slate-500 mb-1">{$t('cal.duration_min')}</label>
              <input type="number" min="5" step="5" bind:value={formDuration} class="w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-xs text-white" />
            </div>
          </div>
        {/if}

        <textarea bind:value={formDescription} rows="3" placeholder={$t('cal.description_placeholder')}
          class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-xs text-white placeholder:text-slate-500 resize-none focus:outline-none focus:border-blue-500/60" />

        <div class="flex items-center gap-2">
          {#each EVENT_COLORS as c (c)}
            <button on:click={() => (formColor = c)}
              class="w-6 h-6 rounded-full transition-transform {formColor === c ? 'scale-110 ring-2 ring-white/60' : ''}"
              style="background:{c}" />
          {/each}
        </div>
      </div>

      <div class="flex justify-between items-center mt-5">
        {#if editing.id}
          <button on:click={() => editing && deleteEvent(editing.id)} class="text-xs text-red-400 hover:text-red-300 flex items-center gap-1">
            <Trash2 size={12} /> {$t('settings.common.delete')}
          </button>
        {:else}<span />{/if}
        <div class="flex gap-2">
          <button on:click={closeEditor} class="px-3.5 py-1.5 text-xs bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors">{$t('settings.common.cancel')}</button>
          <button on:click={saveEvent} disabled={!formTitle.trim()}
            class="px-3.5 py-1.5 text-xs bg-blue-600 hover:bg-blue-500 rounded-lg transition-colors disabled:opacity-40">{$t('settings.common.save')}</button>
        </div>
      </div>
    </div>
  </div>
{/if}
