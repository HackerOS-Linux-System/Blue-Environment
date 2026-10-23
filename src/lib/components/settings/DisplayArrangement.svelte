<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Monitor, Check, RotateCcw, AlignHorizontalSpaceAround, Loader2 } from 'lucide-svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import { t } from '../../stores/language';
  import { logicalSize, snapMove, resolveOverlap, normalize, arrangeRow, isConnected, anyOverlap, type Rect } from '../../utils/displayLayout';

  interface Mode { width: number; height: number; refresh: number; preferred: boolean; current: boolean }
  interface Output { name: string; description: string; enabled: boolean; x: number; y: number; scale: number; transform: string; modes: Mode[] }
  interface Cfg { name: string; enabled: boolean; x: number; y: number; scale: number; transform: string; width: number | null; height: number | null; refresh: number | null }

  const SCALES = [1, 1.25, 1.5, 1.75, 2, 3];
  const ROTATIONS = ['normal', '90', '180', '270'];
  const CONFIRM_SECONDS = 15;

  let outputs = $state<Output[]>([]);
  let cfgs = $state<Cfg[]>([]);
  let previous = $state<Cfg[]>([]);
  let unavailable = $state('');
  let loading = $state(true);
  let selected = $state('');
  let error = $state('');
  let applying = $state(false);
  let countdown = $state(0);
  let countdownTimer: ReturnType<typeof setInterval> | undefined;

  let canvas = $state<HTMLDivElement | undefined>();
  let canvasW = $state(560);
  let drag: { name: string; startX: number; startY: number; origX: number; origY: number } | null = null;

  const currentMode = (o: Output) => o.modes.find((m) => m.current) ?? o.modes.find((m) => m.preferred) ?? o.modes[0];

  function toCfg(o: Output): Cfg {
    const m = currentMode(o);
    return { name: o.name, enabled: o.enabled, x: o.x, y: o.y, scale: o.scale, transform: o.transform, width: m?.width ?? null, height: m?.height ?? null, refresh: m?.refresh ?? null };
  }

  function rectOf(c: Cfg): Rect {
    const o = outputs.find((x) => x.name === c.name)!;
    const m = o.modes.find((mm) => mm.width === c.width && mm.height === c.height) ?? currentMode(o);
    const { w, h } = logicalSize({ width: m?.width ?? 1920, height: m?.height ?? 1080, scale: c.scale, transform: c.transform });
    return { name: c.name, x: c.x, y: c.y, w, h };
  }

  let enabledRects = $derived(cfgs.filter((c) => c.enabled).map(rectOf));
  let bounds = $derived((() => {
    if (!enabledRects.length) return { w: 1920, h: 1080 };
    return { w: Math.max(...enabledRects.map((r) => r.x + r.w)), h: Math.max(...enabledRects.map((r) => r.y + r.h)) };
  })());
  let k = $derived(Math.min(canvasW / Math.max(bounds.w * 1.15, 1), 0.16));
  let connected = $derived(isConnected(enabledRects));
  let overlapping = $derived(anyOverlap(enabledRects));
  let sel = $derived(cfgs.find((c) => c.name === selected) ?? null);
  let selOutput = $derived(outputs.find((o) => o.name === selected) ?? null);
  let dirty = $derived(JSON.stringify(cfgs) !== JSON.stringify(previous));

  async function load() {
    loading = true; error = '';
    try {
      outputs = await SystemBridge.invokeCommand<Output[]>('displays_list', {});
      cfgs = outputs.map(toCfg);
      previous = JSON.parse(JSON.stringify(cfgs));
      selected = outputs[0]?.name ?? '';
      unavailable = '';
    } catch (e: any) { unavailable = String(e?.message ?? e); }
    loading = false;
  }

  onMount(() => { void load(); if (canvas) canvasW = canvas.clientWidth || 560; });
  onDestroy(() => { if (countdownTimer) clearInterval(countdownTimer); });

  function down(e: PointerEvent, c: Cfg) {
    selected = c.name;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { name: c.name, startX: e.clientX, startY: e.clientY, origX: c.x, origY: c.y };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const c = cfgs.find((x) => x.name === drag!.name); if (!c) return;
    const nx = drag.origX + (e.clientX - drag.startX) / k;
    const ny = drag.origY + (e.clientY - drag.startY) / k;
    const me = rectOf({ ...c, x: nx, y: ny });
    const others = cfgs.filter((o) => o.enabled && o.name !== c.name).map(rectOf);
    const s = snapMove(me, others, 40 / k * 0.16 / Math.max(k, 0.0001) * k);
    c.x = s.x; c.y = s.y; cfgs = cfgs;
  }
  function up() {
    if (!drag) return;
    const c = cfgs.find((x) => x.name === drag!.name);
    drag = null;
    if (!c) return;
    const others = cfgs.filter((o) => o.enabled && o.name !== c.name).map(rectOf);
    const fixed = resolveOverlap(rectOf(c), others);
    c.x = fixed.x; c.y = fixed.y;
    normalizeAll();
  }

  function normalizeAll() {
    const on = cfgs.filter((c) => c.enabled);
    const norm = normalize(on.map(rectOf));
    for (const r of norm) { const c = cfgs.find((x) => x.name === r.name)!; c.x = r.x; c.y = r.y; }
    cfgs = cfgs;
  }

  function row() {
    const on = cfgs.filter((c) => c.enabled);
    const arranged = arrangeRow(on.map(rectOf));
    for (const r of arranged) { const c = cfgs.find((x) => x.name === r.name)!; c.x = r.x; c.y = r.y; }
    cfgs = cfgs;
  }

  function setMode(c: Cfg, v: string) {
    const [w, h, r] = v.split('x').join(' ').split('@').join(' ').split(' ').map(Number);
    c.width = w; c.height = h; c.refresh = r; cfgs = cfgs; normalizeAll();
  }

  async function apply() {
    applying = true; error = '';
    const toSend = cfgs;
    try {
      await SystemBridge.invokeCommand('displays_apply', { layout: toSend });
      countdown = CONFIRM_SECONDS;
      countdownTimer = setInterval(() => { countdown -= 1; if (countdown <= 0) void revert(); }, 1000);
    } catch (e: any) { error = String(e?.message ?? e); }
    applying = false;
  }

  function keep() {
    if (countdownTimer) clearInterval(countdownTimer);
    countdown = 0; previous = JSON.parse(JSON.stringify(cfgs));
  }

  /** Puts the previous layout back — also what happens if nobody confirms in time (e.g. a blank screen). */
  async function revert() {
    if (countdownTimer) clearInterval(countdownTimer);
    countdown = 0;
    try { await SystemBridge.invokeCommand('displays_apply', { layout: previous }); } catch { /* best effort */ }
    cfgs = JSON.parse(JSON.stringify(previous));
  }
</script>

{#if loading}
  <div class="flex items-center justify-center py-6"><Loader2 size={18} class="animate-spin text-blue-400" /></div>
{:else if unavailable}
  <div class="bg-slate-800 rounded-xl p-4 border border-white/5 text-xs text-slate-400">{$t('displays.unavailable')}<div class="text-slate-600 mt-1 break-words">{unavailable}</div></div>
{:else}
  <div class="bg-slate-800 rounded-xl p-4 border border-white/5 space-y-3">
    <div class="flex items-center justify-between gap-2">
      <h3 class="text-sm font-medium text-white">{$t('displays.arrangement')}</h3>
      <div class="flex gap-1.5 text-xs">
        <button onclick={row} class="flex items-center gap-1 px-2.5 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg"><AlignHorizontalSpaceAround size={12} /> {$t('displays.row')}</button>
        <button onclick={load} class="flex items-center gap-1 px-2.5 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg"><RotateCcw size={12} /> {$t('displays.reset')}</button>
      </div>
    </div>

    <div bind:this={canvas} class="relative w-full rounded-lg bg-slate-950/60 border border-white/5 overflow-hidden touch-none" style="height:{Math.max(140, bounds.h * k + 40)}px"
         onpointermove={move} onpointerup={up} onpointercancel={up} role="application" aria-label={$t('displays.arrangement')}>
      {#each cfgs.filter((c) => c.enabled) as c (c.name)}
        {@const r = rectOf(c)}
        <div role="button" tabindex="0" aria-label={c.name}
             onpointerdown={(e) => down(e, c)} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') selected = c.name; }}
             class="absolute border-2 rounded-md flex flex-col items-center justify-center cursor-move select-none {selected === c.name ? 'border-blue-500 bg-blue-500/20' : 'border-white/30 bg-white/5'}"
             style="left:{20 + r.x * k}px;top:{20 + r.y * k}px;width:{r.w * k}px;height:{r.h * k}px">
          <Monitor size={16} class="text-blue-300" />
          <div class="text-[11px] text-white truncate max-w-full px-1">{c.name}</div>
          <div class="text-[9px] text-slate-400">{r.w}×{r.h}</div>
        </div>
      {/each}
    </div>

    {#if overlapping}<p class="text-xs text-amber-300">{$t('displays.overlap')}</p>
    {:else if !connected}<p class="text-xs text-amber-300">{$t('displays.gap')}</p>{/if}

    {#if sel && selOutput}
      <div class="grid sm:grid-cols-2 gap-3 text-xs border-t border-white/5 pt-3">
        <div class="sm:col-span-2 flex items-center gap-2">
          <span class="text-sm text-white font-medium">{sel.name}</span><span class="text-slate-500 truncate">{selOutput.description}</span>
          <label class="ml-auto flex items-center gap-1.5 text-slate-300"><input type="checkbox" bind:checked={sel.enabled} onchange={() => { cfgs = cfgs; normalizeAll(); }} /> {$t('displays.enabled')}</label>
        </div>
        {#if sel.enabled}
          <label class="grid gap-1 text-slate-400">{$t('displays.resolution')}
            <select class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-white" value={`${sel.width}x${sel.height}@${sel.refresh}`} onchange={(e) => setMode(sel, e.currentTarget.value)}>
              {#each selOutput.modes as m}<option value={`${m.width}x${m.height}@${m.refresh}`}>{m.width}×{m.height} @ {m.refresh.toFixed(0)} Hz{m.preferred ? ' ★' : ''}</option>{/each}
            </select>
          </label>
          <label class="grid gap-1 text-slate-400">{$t('displays.scale')}
            <select class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-white" bind:value={sel.scale} onchange={() => { cfgs = cfgs; normalizeAll(); }}>
              {#each SCALES as s}<option value={s}>{Math.round(s * 100)}%</option>{/each}
            </select>
          </label>
          <label class="grid gap-1 text-slate-400">{$t('displays.rotation')}
            <select class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-white" bind:value={sel.transform} onchange={() => { cfgs = cfgs; normalizeAll(); }}>
              {#each ROTATIONS as r}<option value={r}>{r === 'normal' ? $t('displays.rot_normal') : `${r}°`}</option>{/each}
            </select>
          </label>
        {/if}
      </div>
    {/if}

    {#if error}<p class="text-xs text-red-300 break-words" role="alert">{error}</p>{/if}

    {#if countdown > 0}
      <div class="flex items-center gap-3 bg-amber-500/10 border border-amber-500/30 rounded-lg px-3 py-2 text-sm" role="alertdialog" aria-label={$t('displays.keep_title')}>
        <span class="flex-1 text-amber-100">{$t('displays.keep_body').replace('{s}', String(countdown))}</span>
        <button onclick={keep} class="flex items-center gap-1 px-3 py-1 bg-green-600 hover:bg-green-500 rounded-lg text-xs"><Check size={12} /> {$t('displays.keep')}</button>
        <button onclick={revert} class="px-3 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs">{$t('displays.revert')}</button>
      </div>
    {:else}
      <button onclick={apply} disabled={!dirty || applying || overlapping} class="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-40">{#if applying}<Loader2 size={14} class="animate-spin" />{/if} {$t('displays.apply')}</button>
    {/if}
  </div>
{/if}
