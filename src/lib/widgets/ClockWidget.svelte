<script lang="ts">
  import { onMount } from 'svelte';
  import { clockAngles } from './widgetModel';
  import type { WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  let now = $state(new Date());
  let analog = $derived(!!widget.config.analog);
  let hour12 = $derived(!!widget.config.hour12);
  let showSeconds = $derived(!!widget.config.seconds);

  onMount(() => {
    const id = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(id);
  });

  let time = $derived(now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: showSeconds ? '2-digit' : undefined, hour12 }));
  let date = $derived(now.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }));
  let angles = $derived(clockAngles(now));
  const toggle = (key: string) => patchWidget(widget.id, { config: { [key]: !widget.config[key] } });
</script>

<div class="h-full flex flex-col items-center justify-center gap-1 px-3">
  {#if analog}
    <svg viewBox="-50 -50 100 100" class="h-[70%] max-h-full aspect-square" aria-label={time}>
      <circle r="48" fill="none" stroke="currentColor" stroke-opacity="0.25" stroke-width="2" />
      {#each Array(12) as _, i}
        <line x1="0" y1="-42" x2="0" y2="-46" stroke="currentColor" stroke-opacity="0.5" stroke-width="2" transform="rotate({i * 30})" />
      {/each}
      <line x1="0" y1="0" x2="0" y2="-24" stroke="currentColor" stroke-width="4" stroke-linecap="round" transform="rotate({angles.hour})" />
      <line x1="0" y1="0" x2="0" y2="-36" stroke="currentColor" stroke-width="3" stroke-linecap="round" transform="rotate({angles.minute})" />
      <line x1="0" y1="4" x2="0" y2="-40" stroke="#60a5fa" stroke-width="1.5" stroke-linecap="round" transform="rotate({angles.second})" />
      <circle r="2.5" fill="#60a5fa" />
    </svg>
  {:else}
    <div class="text-4xl font-semibold tabular-nums tracking-tight leading-none">{time}</div>
    <div class="text-xs text-slate-300 capitalize">{date}</div>
  {/if}
  {#if editing}
    <div class="flex gap-1 mt-1 text-[10px]">
      <button onclick={() => toggle('analog')} class="px-2 py-0.5 rounded bg-white/10 hover:bg-white/20">{analog ? 'Digital' : 'Analog'}</button>
      <button onclick={() => toggle('hour12')} class="px-2 py-0.5 rounded bg-white/10 hover:bg-white/20">{hour12 ? '12h' : '24h'}</button>
      <button onclick={() => toggle('seconds')} class="px-2 py-0.5 rounded bg-white/10 hover:bg-white/20">{showSeconds ? 'sec ✓' : 'sec'}</button>
    </div>
  {/if}
</div>
