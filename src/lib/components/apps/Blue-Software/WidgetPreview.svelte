<script lang="ts">
  /**
   * Small live preview of a Widget Store entry. It does not mount the real
   * widget (those read and write the desktop's widget store); it draws a
   * lightweight stand-in from the entry's own settings.
   */
  import { onMount } from 'svelte';
  import type { CatalogEntry } from '../../../widgets/widgetCatalog';
  import { clockAngles, countdownParts, dailyIndex, parseLinks, parseQuotes, parseTargetDate, parseZones, formatTimer, zoneLabel, DEFAULT_QUOTES } from '../../../widgets/widgetModel';

  interface Props { entry: CatalogEntry }
  let { entry }: Props = $props();

  let now = $state(new Date());
  onMount(() => {
    const id = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(id);
  });

  const cfg = $derived(entry.config ?? {});
  const zones = $derived(parseZones(String(cfg.zones ?? 'Europe/London,America/New_York,Asia/Tokyo'), 4));
  const quotes = $derived(parseQuotes(String(cfg.quotes ?? DEFAULT_QUOTES.join('\n'))));
  const quote = $derived(quotes[dailyIndex(quotes.length, now)]);
  const links = $derived(parseLinks(String(cfg.links ?? ''), 3));
  const target = $derived(parseTargetDate(String(cfg.date ?? '')));
  const cd = $derived(countdownParts(target, now.getTime()));
  const angles = $derived(clockAngles(now));
  const analog = $derived(!!cfg.analog);
  const minutes = $derived(Number(cfg.minutes) || 25);
</script>

<div class="h-full w-full flex flex-col items-center justify-center overflow-hidden text-white px-3 py-2 text-center">
  {#if entry.type === 'clock'}
    {#if analog}
      <svg viewBox="-50 -50 100 100" class="h-full max-h-24 aspect-square" aria-hidden="true">
        <circle r="46" fill="none" stroke="currentColor" stroke-opacity="0.3" stroke-width="2" />
        {#each Array(12) as _, i}<line x1="0" y1="-40" x2="0" y2="-45" stroke="currentColor" stroke-opacity="0.6" transform="rotate({i * 30})" />{/each}
        <line x1="0" y1="0" x2="0" y2="-24" stroke="white" stroke-width="3" stroke-linecap="round" transform="rotate({angles.hour})" />
        <line x1="0" y1="0" x2="0" y2="-36" stroke="white" stroke-width="2" stroke-linecap="round" transform="rotate({angles.minute})" />
        <line x1="0" y1="4" x2="0" y2="-40" stroke="#60a5fa" stroke-width="1" transform="rotate({angles.second})" />
      </svg>
    {:else}
      <div class="text-3xl font-bold tabular-nums">{now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</div>
      <div class="text-xs text-slate-300">{now.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })}</div>
    {/if}
  {:else if entry.type === 'weather'}
    <div class="text-4xl">⛅</div><div class="text-xl font-semibold">18°C</div><div class="text-xs text-slate-300">Partly cloudy</div>
  {:else if entry.type === 'notes'}
    <div class="w-full text-left text-xs text-amber-100/90 leading-relaxed">Buy milk<br />Call Anna<br /><span class="opacity-50">▍</span></div>
  {:else if entry.type === 'system'}
    <svg viewBox="0 0 100 40" class="w-full max-h-20" aria-hidden="true"><polyline points="0,32 10,28 20,30 30,18 40,22 50,10 60,16 70,8 80,20 90,12 100,14" fill="none" stroke="#34d399" stroke-width="2" /><polyline points="0,36 15,34 30,35 45,30 60,31 75,26 100,28" fill="none" stroke="#60a5fa" stroke-width="2" /></svg>
    <div class="text-[10px] text-slate-300">CPU · RAM</div>
  {:else if entry.type === 'calendar'}
    <div class="grid grid-cols-7 gap-0.5 text-[8px] text-slate-300">{#each Array(21) as _, i}<span class="w-3 h-3 flex items-center justify-center rounded-full {i === 9 ? 'bg-blue-500 text-white' : ''}">{i + 1}</span>{/each}</div>
  {:else if entry.type === 'worldclock'}
    <div class="w-full space-y-0.5">
      {#each zones as z (z)}
        <div class="flex items-baseline justify-between text-xs"><span class="truncate text-slate-200">{zoneLabel(z)}</span><span class="tabular-nums font-semibold">{now.toLocaleTimeString([], { timeZone: z, hour: '2-digit', minute: '2-digit', hour12: !!cfg.hour12 })}</span></div>
      {/each}
    </div>
  {:else if entry.type === 'countdown'}
    <div class="text-[10px] uppercase tracking-wide text-slate-300 truncate max-w-full">{String(cfg.label ?? 'Countdown')}</div>
    {#if Number.isNaN(target)}<div class="text-xs text-slate-400">—</div>
    {:else if cd.done}<div class="text-lg font-semibold text-emerald-300">🎉</div>
    {:else}<div class="flex items-end gap-2"><span class="text-2xl font-bold tabular-nums">{cd.days}<small class="text-[10px] text-slate-400">d</small></span><span class="text-lg tabular-nums">{cd.hours}<small class="text-[10px] text-slate-400">h</small></span><span class="text-lg tabular-nums">{cd.minutes}<small class="text-[10px] text-slate-400">m</small></span></div>{/if}
  {:else if entry.type === 'quote'}
    {#if quote}<p class="text-xs italic leading-snug text-slate-100 line-clamp-3">“{quote.text}”</p>{#if quote.author}<p class="text-[10px] text-pink-300 mt-1">— {quote.author}</p>{/if}{/if}
  {:else if entry.type === 'links'}
    <div class="w-full space-y-1">{#each links as l}<div class="rounded bg-white/10 px-2 py-0.5 text-xs text-left truncate">{l.label}</div>{:else}<span class="text-xs text-slate-400">No links</span>{/each}</div>
  {:else if entry.type === 'todo'}
    <div class="w-full text-left text-xs space-y-1"><div class="flex items-center gap-1.5"><span class="text-emerald-400">☑</span><span class="line-through text-slate-500">Plan the week</span></div><div class="flex items-center gap-1.5"><span>☐</span>Write report</div><div class="flex items-center gap-1.5"><span>☐</span>Backup files</div></div>
  {:else if entry.type === 'timer'}
    <div class="text-3xl font-bold tabular-nums">{formatTimer(cfg.mode === 'stopwatch' ? 0 : minutes * 60_000)}</div>
    <div class="text-[10px] uppercase tracking-wide text-slate-300">{cfg.mode === 'stopwatch' ? 'Stopwatch' : 'Focus'}</div>
  {/if}
</div>
