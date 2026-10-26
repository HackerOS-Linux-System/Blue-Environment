<script lang="ts">
  import { onMount } from 'svelte';
  import { countdownParts, parseTargetDate } from './widgetModel';
  import type { WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  let now = $state(Date.now());
  let label = $derived(String(widget.config.label ?? ''));
  let dateText = $derived(String(widget.config.date ?? ''));
  let target = $derived(parseTargetDate(dateText));
  let parts = $derived(countdownParts(target, now));

  onMount(() => {
    const id = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(id);
  });

  const set = (key: string, e: Event) => patchWidget(widget.id, { config: { [key]: (e.currentTarget as HTMLInputElement).value } });
</script>

<div class="h-full flex flex-col items-center justify-center gap-1.5 px-3 text-center">
  {#if editing}
    <input value={label} onchange={(e) => set('label', e)} onmousedown={(e) => e.stopPropagation()} placeholder="Label" class="mt-5 w-full rounded bg-black/30 px-2 py-1 text-xs text-slate-100 outline-none" />
    <input type="datetime-local" value={dateText} onchange={(e) => set('date', e)} onmousedown={(e) => e.stopPropagation()} class="w-full rounded bg-black/30 px-2 py-1 text-xs text-slate-100 outline-none" />
  {:else if Number.isNaN(target)}
    <p class="text-xs text-slate-400">Pick a date with “Edit widgets”.</p>
  {:else}
    <div class="text-xs uppercase tracking-wide text-slate-400 truncate max-w-full">{label || 'Countdown'}</div>
    {#if parts.done}
      <div class="text-xl font-semibold text-emerald-300">🎉 It's time!</div>
    {:else}
      <div class="flex items-end gap-3 text-white">
        {#each [[parts.days, 'd'], [parts.hours, 'h'], [parts.minutes, 'm']] as [v, u]}
          <div><span class="text-3xl font-bold tabular-nums">{v}</span><span class="text-xs text-slate-400 ml-0.5">{u}</span></div>
        {/each}
      </div>
    {/if}
  {/if}
</div>
