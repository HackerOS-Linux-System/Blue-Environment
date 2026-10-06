<script lang="ts">
  /** Hour grid shared by the Week (7 columns) and Day (1 column) views. */
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import { t } from '../../../stores/language';
  import { layoutDay } from './recurrence';
  import type { Occurrence } from './types';

  export let days: string[];
  export let byDate: Record<string, Occurrence[]>;
  export let today: string;
  const dispatch = createEventDispatcher<{ slot: { date: string; minutes: number }; open: Occurrence }>();

  const HOUR_H = 48;
  const WEEKDAY_KEYS = ['cal.mon', 'cal.tue', 'cal.wed', 'cal.thu', 'cal.fri', 'cal.sat', 'cal.sun'];
  const HOURS = Array.from({ length: 24 }, (_, h) => h);
  const pad = (n: number) => String(n).padStart(2, '0');
  // Monday-first index of an ISO date (for the column header's weekday name)
  const wdIndex = (iso: string) => (new Date(iso + 'T00:00:00').getDay() + 6) % 7;

  let scroller: HTMLDivElement;
  let nowMin = minutesNow();
  let timer: ReturnType<typeof setInterval>;
  function minutesNow() { const d = new Date(); return d.getHours() * 60 + d.getMinutes(); }
  onMount(() => {
    if (scroller) scroller.scrollTop = 7 * HOUR_H;               // start the view at 07:00
    timer = setInterval(() => (nowMin = minutesNow()), 60_000);
  });
  onDestroy(() => clearInterval(timer));

  function clickSlot(e: MouseEvent, date: string) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const minutes = Math.min(23 * 60 + 30, Math.max(0, Math.floor(((e.clientY - rect.top) / HOUR_H) * 2) * 30));
    dispatch('slot', { date, minutes });
  }
  const cols = () => `grid-template-columns: 3rem repeat(${days.length}, minmax(0, 1fr));`;
</script>

<div class="flex flex-col flex-1 min-h-0">
  <!-- header: weekday + date, then the all-day strip -->
  <div class="grid shrink-0 border-b border-white/5" style={cols()}>
    <div />
    {#each days as d (d)}
      <div class="text-center py-1.5 border-l border-white/5">
        <div class="text-[10px] uppercase tracking-wider text-slate-500">{$t(WEEKDAY_KEYS[wdIndex(d)])}</div>
        <div class="text-sm font-medium {d === today ? 'inline-flex w-6 h-6 items-center justify-center rounded-full bg-blue-500 text-white' : 'text-slate-200'}">{Number(d.slice(8))}</div>
      </div>
    {/each}
  </div>
  <div class="grid shrink-0 border-b border-white/5 min-h-[1.75rem]" style={cols()}>
    <div class="text-[9px] text-slate-600 text-right pr-1.5 pt-1">{$t('cal.all_day')}</div>
    {#each days as d (d)}
      <div class="border-l border-white/5 p-0.5 space-y-0.5">
        {#each (byDate[d] ?? []).filter((o) => !o.event.time) as o (o.key)}
          <button on:click={() => dispatch('open', o)} class="w-full text-left truncate rounded px-1.5 py-0.5 text-[10px] text-white" style="background:{o.event.color}cc" title={o.event.title}>{o.event.title}</button>
        {/each}
      </div>
    {/each}
  </div>

  <!-- scrolling hour grid -->
  <div class="flex-1 overflow-y-auto relative" bind:this={scroller}>
    <div class="grid relative" style="{cols()} height:{24 * HOUR_H}px;">
      <div class="relative">
        {#each HOURS as h (h)}<div class="absolute right-1.5 text-[9px] text-slate-600 -translate-y-1.5" style="top:{h * HOUR_H}px">{h === 0 ? '' : `${pad(h)}:00`}</div>{/each}
      </div>
      {#each days as d (d)}
        <div class="relative border-l border-white/5" on:click={(e) => clickSlot(e, d)} role="presentation">
          {#each HOURS as h (h)}<div class="absolute left-0 right-0 border-t border-white/[0.04]" style="top:{h * HOUR_H}px" />{/each}
          {#each layoutDay(byDate[d] ?? []) as p (p.occ.key)}
            <button
              on:click|stopPropagation={() => dispatch('open', p.occ)}
              class="absolute rounded-md px-1.5 py-0.5 text-left overflow-hidden text-white text-[10px] leading-tight border border-black/20 hover:brightness-110"
              style="top:{(p.startMin / 60) * HOUR_H}px; height:{Math.max(18, ((p.endMin - p.startMin) / 60) * HOUR_H - 2)}px; left:calc({(p.lane / p.lanes) * 100}% + 1px); width:calc({100 / p.lanes}% - 3px); background:{p.occ.event.color}dd;"
              title="{p.occ.event.time} {p.occ.event.title}">
              <span class="font-semibold">{p.occ.event.time}</span> {p.occ.event.title}
            </button>
          {/each}
          {#if d === today}
            <div class="absolute left-0 right-0 pointer-events-none" style="top:{(nowMin / 60) * HOUR_H}px">
              <div class="h-px bg-red-500" /><div class="absolute -left-1 -top-1 w-2 h-2 rounded-full bg-red-500" />
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>
