<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { weatherGlyph, type WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';
  import { t } from '../stores/language';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  type W = Awaited<ReturnType<typeof SystemBridge.getWeather>>;
  let data = $state<W | null>(null);
  let failed = $state(false);
  let city = $derived(String(widget.config.city ?? ''));
  let cityDraft = $state(untrack(() => String(widget.config.city ?? '')));

  async function refresh() {
    try { data = await SystemBridge.getWeather(city); failed = false; }
    catch { failed = true; }
  }

  onMount(() => {
    const id = setInterval(refresh, 30 * 60 * 1000);
    return () => clearInterval(id);
  });
  $effect(() => { void city; untrack(refresh); });

  function saveCity() { patchWidget(widget.id, { config: { city: cityDraft.trim() } }); }
</script>

<div class="h-full flex flex-col justify-center px-4 gap-1">
  {#if data}
    <div class="flex items-center gap-3">
      <span class="text-4xl leading-none">{weatherGlyph(data.code)}</span>
      <div>
        <div class="text-3xl font-semibold leading-none">{Math.round(data.tempC)}°</div>
        <div class="text-xs text-slate-300 truncate max-w-[130px]">{data.city}</div>
      </div>
    </div>
    <div class="text-[11px] text-slate-400 flex gap-3">
      {#if data.highC != null && data.lowC != null}<span>↑{Math.round(data.highC)}° ↓{Math.round(data.lowC)}°</span>{/if}
      {#if data.humidity != null}<span>💧{Math.round(data.humidity)}%</span>{/if}
      {#if data.windKph != null}<span>💨{Math.round(data.windKph)} km/h</span>{/if}
    </div>
  {:else if failed}
    <div class="text-xs text-slate-400">{$t('widgets.weather_failed')}</div>
    <button onclick={refresh} class="self-start text-[11px] px-2 py-0.5 rounded bg-white/10 hover:bg-white/20">{$t('widgets.retry')}</button>
  {:else}
    <div class="text-xs text-slate-400">…</div>
  {/if}
  {#if editing}
    <form class="flex gap-1 mt-1" onsubmit={(e) => { e.preventDefault(); saveCity(); }}>
      <input bind:value={cityDraft} placeholder={$t('widgets.weather_city')} class="flex-1 min-w-0 bg-black/30 border border-white/10 rounded px-2 py-0.5 text-[11px]" />
      <button class="px-2 rounded bg-blue-600 text-[11px]">OK</button>
    </form>
  {/if}
</div>
