<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { Trash2, X, Lock } from 'lucide-svelte';
  import { t } from '../../../stores/language';
  import { EVENT_COLORS, REMINDER_CHOICES, WEEKDAY_CODES, type CalendarEvent, type RecurrenceRule } from './types';
  import { weekdayCode } from './recurrence';

  /** The event being edited (a blank one with `id: ''` for a new event). */
  export let event: CalendarEvent;
  /** The day of the occurrence that was clicked (for "delete this occurrence"). */
  export let occurrenceDate: string = event.date;
  /** Name of the subscription this event comes from → the editor is read-only. */
  export let readonlySource: string | null = null;

  const dispatch = createEventDispatcher<{
    save: Omit<CalendarEvent, 'id'> & { id?: string };
    delete: void;
    deleteOccurrence: string;
    close: void;
  }>();

  const isNew = !event.id;
  const readonly = readonlySource !== null;
  const WEEKDAY_KEYS = ['cal.mon', 'cal.tue', 'cal.wed', 'cal.thu', 'cal.fri', 'cal.sat', 'cal.sun'];

  let title = event.title;
  let date = event.date;
  let allDay = event.time === null;
  let time = event.time ?? '09:00';
  let duration = event.durationMinutes ?? 30;
  let description = event.description;
  let color: string = event.color;

  const r = event.recurrence ?? null;
  let repeat: 'none' | RecurrenceRule['freq'] = r ? r.freq : 'none';
  let interval = r?.interval ?? 1;
  let byDay: string[] = r?.byDay?.length ? [...r.byDay] : [weekdayCode(event.date)];
  let ends: 'never' | 'on' | 'after' = r?.until ? 'on' : r?.count ? 'after' : 'never';
  let endsDate = r?.until ?? '';
  let endsCount = r?.count ?? 10;
  let reminder: string = event.reminderMinutes == null ? '' : String(event.reminderMinutes);

  const isRecurring = !!event.recurrence;
  let formError = '';

  function toggleDay(code: string) {
    byDay = byDay.includes(code) ? byDay.filter((c) => c !== code) : [...byDay, code];
  }

  function reminderLabel(m: number): string {
    if (m === 0) return $t('cal.reminder.at_start');
    if (m === 60) return $t('cal.reminder.hour');
    if (m === 1440) return $t('cal.reminder.day');
    if (m === 10080) return $t('cal.reminder.week');
    if (m % 60 === 0) return $t('cal.reminder.hours', { n: m / 60 });
    return $t('cal.reminder.min', { n: m });
  }

  function save() {
    formError = '';
    if (!title.trim()) return;
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) { formError = $t('cal.err.date'); return; }
    let recurrence: RecurrenceRule | null = null;
    if (repeat !== 'none') {
      const n = Math.max(1, Math.floor(Number(interval) || 1));
      if (ends === 'on' && (!endsDate || endsDate < date)) { formError = $t('cal.err.until'); return; }
      if (ends === 'after' && !(Number(endsCount) >= 1)) { formError = $t('cal.err.count'); return; }
      const days = WEEKDAY_CODES.filter((c) => byDay.includes(c));
      recurrence = {
        freq: repeat, interval: n,
        byDay: repeat === 'weekly' ? (days.length ? days : [weekdayCode(date)]) : null,
        until: ends === 'on' ? endsDate : null,
        count: ends === 'after' ? Math.floor(Number(endsCount)) : null,
      };
    }
    dispatch('save', {
      id: event.id || undefined,
      title: title.trim(), date,
      time: allDay ? null : time,
      durationMinutes: allDay ? null : Math.max(5, Math.floor(Number(duration) || 30)),
      description, color, recurrence,
      exdates: event.exdates ?? [],
      reminderMinutes: reminder === '' ? null : Number(reminder),
    });
  }

  const field = 'w-full bg-slate-900 border border-white/10 rounded-lg px-2.5 py-1.5 text-xs text-white disabled:opacity-60 focus:outline-none focus:border-blue-500/60';
</script>

<div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm" on:mousedown={() => dispatch('close')} role="presentation">
  <div class="w-[26rem] max-h-[90vh] overflow-y-auto bg-slate-800 border border-white/10 rounded-2xl shadow-2xl p-5" on:mousedown|stopPropagation role="dialog" aria-modal="true">
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-sm font-semibold text-white">{isNew ? $t('cal.new_event') : $t('cal.edit_event')}</h3>
      <button on:click={() => dispatch('close')} class="p-1 hover:bg-white/10 rounded text-slate-500"><X size={14} /></button>
    </div>

    {#if readonly}
      <div class="mb-3 flex items-center gap-1.5 text-[11px] text-amber-300 bg-amber-500/10 rounded-lg px-2.5 py-1.5"><Lock size={11} /> {$t('cal.readonly_from', { name: readonlySource ?? '' })}</div>
    {/if}

    <div class="space-y-3">
      <input bind:value={title} disabled={readonly} placeholder={$t('cal.title_placeholder')}
        class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white placeholder:text-slate-500 disabled:opacity-60 focus:outline-none focus:border-blue-500/60" />

      <div class="flex gap-2">
        <div class="flex-1">
          <span class="block text-[10px] text-slate-500 mb-1">{$t('cal.date')}</span>
          <input type="date" bind:value={date} disabled={readonly} class={field} />
        </div>
        <label class="flex items-end gap-2 pb-1.5 text-xs text-slate-300">
          <input type="checkbox" bind:checked={allDay} disabled={readonly} class="rounded" /> {$t('cal.all_day')}
        </label>
      </div>

      {#if !allDay}
        <div class="flex gap-2">
          <div class="flex-1">
            <span class="block text-[10px] text-slate-500 mb-1">{$t('cal.time')}</span>
            <input type="time" bind:value={time} disabled={readonly} class={field} />
          </div>
          <div class="flex-1">
            <span class="block text-[10px] text-slate-500 mb-1">{$t('cal.duration_min')}</span>
            <input type="number" min="5" step="5" bind:value={duration} disabled={readonly} class={field} />
          </div>
        </div>
      {/if}

      <!-- Repeat -->
      <div>
        <span class="block text-[10px] text-slate-500 mb-1">{$t('cal.repeat')}</span>
        <select bind:value={repeat} disabled={readonly} class={field}>
          <option value="none">{$t('cal.repeat.none')}</option>
          <option value="daily">{$t('cal.repeat.daily')}</option>
          <option value="weekly">{$t('cal.repeat.weekly')}</option>
          <option value="monthly">{$t('cal.repeat.monthly')}</option>
          <option value="yearly">{$t('cal.repeat.yearly')}</option>
        </select>
      </div>

      {#if repeat !== 'none'}
        <div class="rounded-lg bg-slate-900/60 border border-white/5 p-2.5 space-y-2.5">
          <div class="flex items-center gap-2 text-xs text-slate-300">
            <span>{$t('cal.every')}</span>
            <input type="number" min="1" bind:value={interval} disabled={readonly} class="w-16 {field}" />
            <span>{$t(`cal.unit.${repeat}`)}</span>
          </div>
          {#if repeat === 'weekly'}
            <div class="flex gap-1">
              {#each WEEKDAY_CODES as code, i (code)}
                <button type="button" disabled={readonly} on:click={() => toggleDay(code)}
                  class="flex-1 py-1 rounded-md text-[10px] font-medium transition-colors {byDay.includes(code) ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'}">{$t(WEEKDAY_KEYS[i])}</button>
              {/each}
            </div>
          {/if}
          <div class="flex items-center gap-2 text-xs text-slate-300">
            <span class="shrink-0">{$t('cal.ends')}</span>
            <select bind:value={ends} disabled={readonly} class="flex-1 {field}">
              <option value="never">{$t('cal.ends.never')}</option>
              <option value="on">{$t('cal.ends.on')}</option>
              <option value="after">{$t('cal.ends.after')}</option>
            </select>
            {#if ends === 'on'}<input type="date" bind:value={endsDate} min={date} disabled={readonly} class="w-36 {field}" />{/if}
            {#if ends === 'after'}<input type="number" min="1" bind:value={endsCount} disabled={readonly} class="w-16 {field}" /><span class="shrink-0">{$t('cal.times')}</span>{/if}
          </div>
          {#if isRecurring && !readonly}<p class="text-[10px] text-slate-500">{$t('cal.recurring_note')}</p>{/if}
        </div>
      {/if}

      <!-- Reminder -->
      <div>
        <span class="block text-[10px] text-slate-500 mb-1">{$t('cal.reminder')}</span>
        <select bind:value={reminder} disabled={readonly} class={field}>
          <option value="">{$t('cal.reminder.none')}</option>
          {#each REMINDER_CHOICES as m (m)}<option value={String(m)}>{reminderLabel(m)}</option>{/each}
        </select>
      </div>

      <textarea bind:value={description} disabled={readonly} rows="3" placeholder={$t('cal.description_placeholder')}
        class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-xs text-white placeholder:text-slate-500 resize-none disabled:opacity-60 focus:outline-none focus:border-blue-500/60" />

      {#if !readonly}
        <div class="flex items-center gap-2">
          {#each EVENT_COLORS as c (c)}
            <button type="button" on:click={() => (color = c)} aria-label={c}
              class="w-6 h-6 rounded-full transition-transform {color === c ? 'scale-110 ring-2 ring-white/60' : ''}" style="background:{c}" />
          {/each}
        </div>
      {/if}

      {#if formError}<p class="text-xs text-red-300">{formError}</p>{/if}
    </div>

    <div class="flex justify-between items-center mt-5 gap-2">
      {#if !isNew && !readonly}
        <div class="flex flex-col items-start gap-1">
          {#if isRecurring}
            <button on:click={() => dispatch('deleteOccurrence', occurrenceDate)} class="text-xs text-red-400 hover:text-red-300 flex items-center gap-1"><Trash2 size={12} /> {$t('cal.delete_this')}</button>
            <button on:click={() => dispatch('delete')} class="text-xs text-red-400 hover:text-red-300 flex items-center gap-1"><Trash2 size={12} /> {$t('cal.delete_all')}</button>
          {:else}
            <button on:click={() => dispatch('delete')} class="text-xs text-red-400 hover:text-red-300 flex items-center gap-1"><Trash2 size={12} /> {$t('settings.common.delete')}</button>
          {/if}
        </div>
      {:else}<span />{/if}
      <div class="flex gap-2">
        <button on:click={() => dispatch('close')} class="px-3.5 py-1.5 text-xs bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors">{readonly ? $t('startmenu.close') : $t('settings.common.cancel')}</button>
        {#if !readonly}
          <button on:click={save} disabled={!title.trim()} class="px-3.5 py-1.5 text-xs bg-blue-600 hover:bg-blue-500 rounded-lg transition-colors disabled:opacity-40">{$t('settings.common.save')}</button>
        {/if}
      </div>
    </div>
  </div>
</div>
