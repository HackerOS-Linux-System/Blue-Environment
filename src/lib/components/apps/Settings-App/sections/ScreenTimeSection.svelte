<script lang="ts">
  import { onMount } from 'svelte';
  import { Hourglass, Trash2, AlertTriangle } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';
  import { APPS } from '../../../../constants';
  import { AppId } from '../../../../types';

  // Mirrors the Rust struct in src-tauri/src/screen_time.rs exactly —
  // see that file's doc comment for why this is a separate, permanent
  // history rather than reusing Parental Controls' own (deliberately
  // midnight-reset) usage counters.
  interface ScreenTimeDayPoint { date: string; total_minutes: number; }
  interface ScreenTimeRangeSummary {
    total_minutes: number;
    by_app: Record<string, number>;
    daily_series: ScreenTimeDayPoint[];
  }

  type Range = 'today' | 'week' | 'month' | 'all';
  const RANGES: Range[] = ['today', 'week', 'month', 'all'];

  let range: Range = 'week';
  let summary: ScreenTimeRangeSummary = { total_minutes: 0, by_app: {}, daily_series: [] };
  let loading = true;
  let confirmingClear = false;

  const RANGE_LABELS: Record<Range, string> = {
    today: 'settings.screen_time.range_today',
    week: 'settings.screen_time.range_week',
    month: 'settings.screen_time.range_month',
    all: 'settings.screen_time.range_all',
  };

  async function load() {
    loading = true;
    summary = await SystemBridge.invokeCommand<ScreenTimeRangeSummary>('screen_time_get_summary', { range })
      .catch(() => ({ total_minutes: 0, by_app: {}, daily_series: [] }));
    loading = false;
  }

  onMount(load);
  $: range, load();

  function fmtDuration(minutes: number): string {
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    if (h === 0) return `${m} min`;
    if (m === 0) return `${h} h`;
    return `${h} h ${m} min`;
  }

  // Weekday-only label for short ranges, month/day for longer ones —
  // avoids 30 identical "pon" labels crowding the "month" chart.
  function fmtDayLabel(dateStr: string): string {
    const d = new Date(dateStr + 'T00:00:00');
    if (isNaN(d.getTime())) return dateStr;
    return range === 'month' || range === 'all'
      ? d.toLocaleDateString(undefined, { day: 'numeric', month: 'short' })
      : d.toLocaleDateString(undefined, { weekday: 'short' });
  }

  $: maxDayMinutes = Math.max(1, ...summary.daily_series.map((d) => d.total_minutes));

  // Sorted app breakdown with a resolved title/icon from the same APPS
  // registry AppsSection.svelte/StartMenu.svelte use, so an app id like
  // "notepad" shows up as "Notepad" with its real icon instead of a raw
  // id string — falls back gracefully for ids with no registry entry
  // (external/removed apps still in old history).
  $: appBreakdown = Object.entries(summary.by_app)
    .map(([id, minutes]) => ({ id, minutes, def: APPS[id as AppId] }))
    .sort((a, b) => b.minutes - a.minutes);

  async function clearHistory() {
    if (!confirmingClear) { confirmingClear = true; return; }
    confirmingClear = false;
    await SystemBridge.invokeCommand('screen_time_clear_history').catch(() => {});
    await load();
  }
</script>

<div class="space-y-6">
  <div>
    <h2 class="text-2xl font-bold text-white">{$t('settings.screen_time.title')}</h2>
    <p class="text-sm text-slate-400 mt-1">{$t('settings.screen_time.subtitle')}</p>
  </div>

  <!-- Range picker -->
  <div class="flex gap-2">
    {#each RANGES as r}
      <button
        on:click={() => (range = r)}
        class="flex-1 py-2 rounded-xl text-sm font-medium border transition-colors {range === r ? 'bg-blue-600 border-blue-500 text-white' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
        {$t(RANGE_LABELS[r])}
      </button>
    {/each}
  </div>

  <!-- Total -->
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 flex items-center gap-4">
    <div class="w-12 h-12 rounded-2xl bg-blue-500/15 flex items-center justify-center shrink-0">
      <Hourglass size={22} class="text-blue-400" />
    </div>
    <div>
      <div class="text-3xl font-bold text-white leading-tight">{fmtDuration(summary.total_minutes)}</div>
      <div class="text-xs text-slate-500">{$t(RANGE_LABELS[range])}</div>
    </div>
  </div>

  <!-- Daily bar chart — plain CSS bars, same lightweight approach as
       System Monitor's Bar.svelte, rather than pulling in a charting
       dependency for a handful of vertical bars. -->
  {#if summary.daily_series.length > 1}
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
      <div class="text-sm font-medium text-white mb-4">{$t('settings.screen_time.daily_breakdown')}</div>
      <div class="flex items-end gap-1.5 h-32">
        {#each summary.daily_series as day (day.date)}
          <div class="flex-1 flex flex-col items-center justify-end h-full gap-1.5" title="{day.date}: {fmtDuration(day.total_minutes)}">
            <div
              class="w-full rounded-t-md bg-blue-500 transition-all"
              style="height: {Math.max(2, (day.total_minutes / maxDayMinutes) * 100)}%; opacity: {day.total_minutes === 0 ? 0.15 : 1};"
            />
            {#if summary.daily_series.length <= 31}
              <span class="text-[10px] text-slate-500 whitespace-nowrap">{fmtDayLabel(day.date)}</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Per-app breakdown -->
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <div class="text-sm font-medium text-white mb-3">{$t('settings.screen_time.by_app')}</div>
    {#if loading}
      <p class="text-sm text-slate-500 py-4 text-center">{$t('settings.screen_time.loading')}</p>
    {:else if appBreakdown.length === 0}
      <p class="text-sm text-slate-500 py-4 text-center">{$t('settings.screen_time.no_data')}</p>
    {:else}
      <div class="space-y-1">
        {#each appBreakdown as app (app.id)}
          <div class="flex items-center gap-3 py-2 border-b border-white/5 last:border-0">
            {#if app.def}
              <svelte:component this={app.def.icon} size={16} class="text-slate-400 shrink-0" />
              <span class="text-white flex-1 truncate">{app.def.title}</span>
            {:else}
              <div class="w-4 h-4 shrink-0" />
              <span class="text-slate-400 flex-1 truncate">{app.id}</span>
            {/if}
            <div class="w-28 h-1.5 rounded-full bg-white/5 overflow-hidden shrink-0">
              <div class="h-full bg-blue-500 rounded-full" style="width: {Math.min(100, (app.minutes / summary.total_minutes) * 100)}%" />
            </div>
            <span class="text-sm text-slate-400 w-20 text-right shrink-0">{fmtDuration(app.minutes)}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Measurement-method disclosure + reset. Being upfront that this is
       focus-time sampling, not real attention tracking, matters more
       here than in most Settings copy — screen-time numbers get treated
       as more authoritative than they are otherwise. -->
  <div class="bg-slate-800/50 p-4 rounded-xl border border-white/5 flex items-start gap-3">
    <AlertTriangle size={16} class="text-amber-400 shrink-0 mt-0.5" />
    <p class="text-xs text-slate-400 leading-relaxed">{$t('settings.screen_time.method_disclosure')}</p>
  </div>

  <button
    on:click={clearHistory}
    on:blur={() => (confirmingClear = false)}
    class="w-full flex items-center justify-center gap-2 py-2.5 rounded-xl text-sm font-medium border transition-colors {confirmingClear ? 'bg-red-600 border-red-500 text-white' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
    <Trash2 size={15} />
    {confirmingClear ? $t('settings.screen_time.clear_confirm') : $t('settings.screen_time.clear')}
  </button>
</div>
