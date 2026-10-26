<script lang="ts">
  import { onMount } from 'svelte';
  import { parseZones, zoneLabel } from './widgetModel';
  import type { WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  let now = $state(new Date());
  let zones = $derived(parseZones(String(widget.config.zones ?? '')));
  let hour12 = $derived(!!widget.config.hour12);

  onMount(() => {
    const id = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(id);
  });

  const time = (z: string) => now.toLocaleTimeString([], { timeZone: z, hour: '2-digit', minute: '2-digit', hour12 });
  const day = (z: string) => now.toLocaleDateString([], { timeZone: z, weekday: 'short' });
  const onZones = (e: Event) => patchWidget(widget.id, { config: { zones: (e.currentTarget as HTMLTextAreaElement).value } });
</script>

<div class="h-full flex flex-col px-3 pt-2 pb-2 gap-1 overflow-hidden">
  {#if editing}
    <textarea
      value={String(widget.config.zones ?? '')}
      onchange={onZones}
      onmousedown={(e) => e.stopPropagation()}
      rows="3" spellcheck="false" placeholder="Europe/Warsaw, Asia/Tokyo, …"
      class="mt-6 w-full flex-1 resize-none rounded bg-black/30 px-2 py-1 text-xs text-slate-200 outline-none"
    ></textarea>
    <label class="flex items-center gap-1.5 text-[11px] text-slate-300">
      <input type="checkbox" checked={hour12} onchange={() => patchWidget(widget.id, { config: { hour12: !hour12 } })} /> 12-hour
    </label>
  {:else if zones.length === 0}
    <p class="m-auto text-xs text-slate-400 text-center">No valid time zones.<br />Use “Edit widgets” to add some.</p>
  {:else}
    {#each zones as z (z)}
      <div class="flex items-baseline justify-between gap-2 border-b border-white/5 last:border-0 py-0.5">
        <div class="min-w-0">
          <div class="text-sm text-slate-100 truncate">{zoneLabel(z)}</div>
          <div class="text-[10px] text-slate-400">{day(z)}</div>
        </div>
        <div class="text-lg font-semibold tabular-nums text-white">{time(z)}</div>
      </div>
    {/each}
  {/if}
</div>
