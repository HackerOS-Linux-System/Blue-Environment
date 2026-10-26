<script lang="ts">
  import { onDestroy } from 'svelte';
  import { Play, Pause, RotateCcw } from 'lucide-svelte';
  import { formatTimer, type WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  let stopwatch = $derived(widget.config.mode === 'stopwatch');
  let minutes = $derived(Math.min(240, Math.max(1, Number(widget.config.minutes) || 25)));
  let total = $derived(minutes * 60_000);

  let running = $state(false);
  let elapsed = $state(0); // ms accumulated while paused/finished
  let startedAt = 0;
  let tick = $state(Date.now());
  let interval: ReturnType<typeof setInterval> | undefined;

  let used = $derived(running ? elapsed + (tick - startedAt) : elapsed);
  let shown = $derived(stopwatch ? used : Math.max(0, total - used));
  let finished = $derived(!stopwatch && used >= total);
  let progress = $derived(stopwatch ? 0 : Math.min(1, used / total));

  function start() {
    if (running) return;
    if (finished) elapsed = 0;
    startedAt = Date.now(); tick = startedAt; running = true;
    interval = setInterval(() => {
      tick = Date.now();
      if (!stopwatch && elapsed + (tick - startedAt) >= total) pause();
    }, 200);
  }
  function pause() {
    if (!running) return;
    elapsed += Date.now() - startedAt;
    running = false;
    clearInterval(interval);
  }
  function reset() { pause(); elapsed = 0; }
  onDestroy(() => clearInterval(interval));

  const onMinutes = (e: Event) => {
    const n = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(n) && n >= 1 && n <= 240) { reset(); patchWidget(widget.id, { config: { minutes: Math.round(n) } }); }
  };
</script>

<div class="h-full flex flex-col items-center justify-center gap-2 px-3 relative">
  {#if !stopwatch}
    <svg viewBox="0 0 36 36" class="absolute inset-0 m-auto h-[78%] max-h-full aspect-square -rotate-90 opacity-70" aria-hidden="true">
      <circle cx="18" cy="18" r="16" fill="none" stroke="currentColor" stroke-opacity="0.12" stroke-width="1.6" />
      <circle cx="18" cy="18" r="16" fill="none" stroke={finished ? '#34d399' : '#f87171'} stroke-width="1.6" stroke-linecap="round" stroke-dasharray={100.5} stroke-dashoffset={100.5 * (1 - progress)} />
    </svg>
  {/if}
  <div class="relative text-3xl font-bold tabular-nums {finished ? 'text-emerald-300' : 'text-white'}">{formatTimer(shown)}</div>
  <div class="relative text-[10px] uppercase tracking-wide text-slate-400">{stopwatch ? 'Stopwatch' : finished ? 'Done' : 'Focus'}</div>
  <div class="relative flex items-center gap-1.5" onmousedown={(e) => e.stopPropagation()} role="presentation">
    <button class="p-1.5 rounded-full bg-white/10 hover:bg-blue-600" onclick={running ? pause : start} aria-label={running ? 'Pause' : 'Start'}>{#if running}<Pause size={14} />{:else}<Play size={14} />{/if}</button>
    <button class="p-1.5 rounded-full bg-white/10 hover:bg-white/20" onclick={reset} aria-label="Reset"><RotateCcw size={14} /></button>
    {#if editing && !stopwatch}
      <input type="number" min="1" max="240" value={minutes} onchange={onMinutes} class="w-14 rounded bg-black/40 px-1.5 py-0.5 text-xs text-slate-100 outline-none" title="Minutes" />
    {/if}
  </div>
</div>
