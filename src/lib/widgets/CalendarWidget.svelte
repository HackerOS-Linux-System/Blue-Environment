<script lang="ts">
  import { ChevronLeft, ChevronRight } from 'lucide-svelte';
  import { monthGrid, type WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';
  import { language } from '../stores/language';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  const today = new Date();
  let year = $state(today.getFullYear());
  let month = $state(today.getMonth());
  let mondayFirst = $derived(widget.config.mondayFirst !== false);
  let cells = $derived(monthGrid(year, month, mondayFirst, today));

  let title = $derived(new Date(year, month, 1).toLocaleDateString($language, { month: 'long', year: 'numeric' }));
  let weekdays = $derived.by(() => {
    const base = new Date(2024, 0, mondayFirst ? 1 : 7); // a Monday / a Sunday
    return Array.from({ length: 7 }, (_, i) => new Date(base.getFullYear(), base.getMonth(), base.getDate() + i).toLocaleDateString($language, { weekday: 'narrow' }));
  });

  function shift(delta: number) {
    const d = new Date(year, month + delta, 1);
    year = d.getFullYear(); month = d.getMonth();
  }
</script>

<div class="h-full flex flex-col px-3 py-2 text-xs">
  <div class="flex items-center justify-between mb-1">
    <button onclick={() => shift(-1)} onmousedown={(e) => e.stopPropagation()} class="p-1 rounded hover:bg-white/10" aria-label="Previous month"><ChevronLeft size={14} /></button>
    <button onclick={() => { year = today.getFullYear(); month = today.getMonth(); }} onmousedown={(e) => e.stopPropagation()} class="font-medium capitalize hover:text-blue-300">{title}</button>
    <button onclick={() => shift(1)} onmousedown={(e) => e.stopPropagation()} class="p-1 rounded hover:bg-white/10" aria-label="Next month"><ChevronRight size={14} /></button>
  </div>
  <div class="grid grid-cols-7 text-center text-slate-500 text-[10px] mb-0.5">
    {#each weekdays as d}<span>{d}</span>{/each}
  </div>
  <div class="grid grid-cols-7 grid-rows-6 flex-1 text-center items-center">
    {#each cells as c}
      <span class="rounded-full mx-auto w-6 h-6 flex items-center justify-center tabular-nums {c.isToday ? 'bg-blue-600 text-white font-semibold' : c.inMonth ? 'text-slate-100' : 'text-slate-600'}">{c.day}</span>
    {/each}
  </div>
  {#if editing}
    <button onclick={() => patchWidget(widget.id, { config: { mondayFirst: !mondayFirst } })} class="mt-1 self-center text-[10px] px-2 py-0.5 rounded bg-white/10 hover:bg-white/20">{mondayFirst ? 'Mon' : 'Sun'}</button>
  {/if}
</div>
