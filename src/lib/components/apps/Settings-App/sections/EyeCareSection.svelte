<script lang="ts">
  import { HeartPulse, Eye, Coffee, Droplets, PersonStanding, Timer, Moon, Gauge, Sparkles } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import {
    eyeCareConfig, eyeCareStats, activeBreak, setEyeCare, applyPreset, beginBreak, resetEyeCareStats,
    healthScore, formatDuration, LIMITS, DEFAULT_EYECARE,
    type EyeCareConfig, type EyeCarePreset, type EyeCareMode,
  } from '../../../../stores/eyeCare';

  let tick = Date.now();
  setInterval(() => (tick = Date.now()), 5000);

  $: cfg = $eyeCareConfig;
  $: rt = (void tick, $eyeCareStats);
  $: score = healthScore(rt, cfg);
  $: screen = formatDuration(rt.activeSec);
  $: limitPct = cfg.dailyLimit ? Math.min(100, Math.round((rt.activeSec / (cfg.dailyLimitMin * 60)) * 100)) : 0;
  $: nextLook = Math.max(0, Math.ceil((cfg.lookAwayEveryMin * 60 - rt.sinceLookAway) / 60));
  $: nextLong = Math.max(0, Math.ceil((cfg.longBreakEveryMin * 60 - rt.sinceLongBreak) / 60));

  const presets: { id: EyeCarePreset; key: string }[] = [
    { id: 'light', key: 'eyecare.preset.light' },
    { id: 'recommended', key: 'eyecare.preset.recommended' },
    { id: 'intense', key: 'eyecare.preset.intense' },
  ];
  const modes: { id: EyeCareMode; key: string; hint: string }[] = [
    { id: 'gentle', key: 'eyecare.mode.gentle', hint: 'eyecare.mode.gentle_hint' },
    { id: 'firm', key: 'eyecare.mode.firm', hint: 'eyecare.mode.firm_hint' },
    { id: 'strict', key: 'eyecare.mode.strict', hint: 'eyecare.mode.strict_hint' },
  ];

  type BoolKey = 'rule2020' | 'longBreak' | 'blink' | 'posture' | 'stretch' | 'water' | 'dailyLimit' | 'lateHours' | 'pauseInDnd';
  type NumKey = keyof typeof LIMITS;

  const reminders: { on: BoolKey; every: NumKey; label: string; hint: string; icon: any }[] = [
    { on: 'blink', every: 'blinkEveryMin', label: 'eyecare.blink', hint: 'eyecare.blink_hint', icon: Eye },
    { on: 'posture', every: 'postureEveryMin', label: 'eyecare.posture', hint: 'eyecare.posture_hint', icon: PersonStanding },
    { on: 'stretch', every: 'stretchEveryMin', label: 'eyecare.stretch', hint: 'eyecare.stretch_hint', icon: Sparkles },
    { on: 'water', every: 'waterEveryMin', label: 'eyecare.water', hint: 'eyecare.water_hint', icon: Droplets },
  ];

  const setBool = (k: BoolKey, v: boolean) => setEyeCare({ [k]: v } as Partial<EyeCareConfig>);
  const setNum = (k: NumKey, e: Event) => {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    if (Number.isFinite(v)) setEyeCare({ [k]: v } as Partial<EyeCareConfig>);
  };
  const numAttrs = (k: NumKey) => ({ min: LIMITS[k][0], max: LIMITS[k][1] });
  const testBreak = () => beginBreak('lookAway', Math.min(cfg.lookAwaySec, 20));
  const testLong = () => beginBreak('longBreak', 20);
  const scoreColor = (s: number) => (s >= 80 ? 'text-emerald-300' : s >= 50 ? 'text-amber-300' : 'text-rose-300');
</script>

<div class="space-y-5 max-w-2xl">
  <p class="text-sm text-slate-400">{$t('eyecare.subtitle')}</p>

  <!-- master switch -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-4">
    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl grid place-items-center {cfg.enabled ? 'bg-emerald-600/30' : 'bg-white/5'}"><HeartPulse size={18} class={cfg.enabled ? 'text-emerald-300' : 'text-slate-400'} /></div>
        <div>
          <div class="text-white font-medium">{cfg.enabled ? $t('eyecare.on') : $t('eyecare.off')}</div>
          <div class="text-xs text-slate-500">{cfg.enabled ? $t('eyecare.on_hint') : $t('eyecare.off_hint')}</div>
        </div>
      </div>
      <button on:click={() => setEyeCare({ enabled: !cfg.enabled })} class="px-4 py-2 rounded-lg text-sm {cfg.enabled ? 'bg-slate-700 hover:bg-slate-600' : 'bg-emerald-600 hover:bg-emerald-500'}">{cfg.enabled ? $t('eyecare.turn_off') : $t('eyecare.turn_on')}</button>
    </div>

    <div>
      <div class="text-xs text-slate-400 mb-2">{$t('eyecare.presets')}</div>
      <div class="flex flex-wrap gap-2">{#each presets as p}<button on:click={() => applyPreset(p.id)} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs">{$t(p.key)}</button>{/each}</div>
    </div>
  </section>

  <!-- today -->
  {#if cfg.enabled}
    <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
      <h3 class="flex items-center gap-2 text-white font-medium"><Gauge size={16} class="text-emerald-300" /> {$t('eyecare.today')}</h3>
      <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 text-center">
        <div class="bg-slate-900/60 rounded-xl p-3"><div class="text-lg font-semibold text-white">{screen.h}h {screen.m}m</div><div class="text-[11px] text-slate-500">{$t('eyecare.stat.screen')}</div></div>
        <div class="bg-slate-900/60 rounded-xl p-3"><div class="text-lg font-semibold text-white">{rt.lookAways + rt.longBreaks}</div><div class="text-[11px] text-slate-500">{$t('eyecare.stat.breaks')}</div></div>
        <div class="bg-slate-900/60 rounded-xl p-3"><div class="text-lg font-semibold text-white">{rt.skipped}</div><div class="text-[11px] text-slate-500">{$t('eyecare.stat.skipped')}</div></div>
        <div class="bg-slate-900/60 rounded-xl p-3"><div class="text-lg font-semibold {scoreColor(score)}">{score}</div><div class="text-[11px] text-slate-500">{$t('eyecare.stat.score')}</div></div>
      </div>
      {#if cfg.dailyLimit}
        <div>
          <div class="flex justify-between text-[11px] text-slate-500 mb-1"><span>{$t('eyecare.stat.limit')}</span><span>{limitPct}%</span></div>
          <div class="h-1.5 rounded bg-slate-700 overflow-hidden"><div class="h-full {limitPct >= 100 ? 'bg-rose-400' : limitPct >= 75 ? 'bg-amber-400' : 'bg-emerald-400'}" style="width:{limitPct}%"></div></div>
        </div>
      {/if}
      <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-400">
        {#if cfg.rule2020}<span>{$t('eyecare.next_look', { n: nextLook })}</span>{/if}
        {#if cfg.longBreak}<span>{$t('eyecare.next_long', { n: nextLong })}</span>{/if}
        <span>{$t('eyecare.stat.streak', { n: rt.streak, best: rt.bestStreak })}</span>
      </div>
      <div class="flex flex-wrap gap-2 pt-1">
        <button disabled={!!$activeBreak} on:click={testBreak} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs disabled:opacity-40">{$t('eyecare.break_now_look')}</button>
        <button disabled={!!$activeBreak} on:click={testLong} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs disabled:opacity-40">{$t('eyecare.break_now_long')}</button>
        <button on:click={resetEyeCareStats} class="px-3 py-1.5 rounded-lg bg-slate-700/60 hover:bg-slate-600 text-xs text-slate-300">{$t('eyecare.reset_stats')}</button>
      </div>
    </section>
  {/if}

  <!-- strictness -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="text-white font-medium">{$t('eyecare.mode')}</h3>
    <div class="space-y-2">
      {#each modes as m}
        <label class="flex items-start gap-3 cursor-pointer">
          <input type="radio" name="eyecare-mode" class="mt-1" checked={cfg.mode === m.id} on:change={() => setEyeCare({ mode: m.id })} />
          <span class="text-sm text-slate-200">{$t(m.key)}<span class="block text-xs text-slate-500">{$t(m.hint)}</span></span>
        </label>
      {/each}
    </div>
  </section>

  <!-- 20-20-20 -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="flex items-center gap-2 text-white font-medium"><Eye size={16} class="text-sky-300" /> {$t('eyecare.rule')}</h3>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('eyecare.rule_enable')}<span class="block text-xs text-slate-500">{$t('eyecare.rule_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={cfg.rule2020} on:change={(e) => setBool('rule2020', e.currentTarget.checked)} />
    </label>
    <div class="grid grid-cols-2 gap-3 text-xs text-slate-400 {cfg.rule2020 ? '' : 'opacity-50'}">
      <label>{$t('eyecare.every_min')}<input type="number" {...numAttrs('lookAwayEveryMin')} value={cfg.lookAwayEveryMin} on:change={(e) => setNum('lookAwayEveryMin', e)} class="mt-1 w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" /></label>
      <label>{$t('eyecare.duration_sec')}<input type="number" {...numAttrs('lookAwaySec')} value={cfg.lookAwaySec} on:change={(e) => setNum('lookAwaySec', e)} class="mt-1 w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" /></label>
    </div>
  </section>

  <!-- long break -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="flex items-center gap-2 text-white font-medium"><Coffee size={16} class="text-amber-300" /> {$t('eyecare.long')}</h3>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('eyecare.long_enable')}<span class="block text-xs text-slate-500">{$t('eyecare.long_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={cfg.longBreak} on:change={(e) => setBool('longBreak', e.currentTarget.checked)} />
    </label>
    <div class="grid grid-cols-2 gap-3 text-xs text-slate-400 {cfg.longBreak ? '' : 'opacity-50'}">
      <label>{$t('eyecare.every_min')}<input type="number" {...numAttrs('longBreakEveryMin')} value={cfg.longBreakEveryMin} on:change={(e) => setNum('longBreakEveryMin', e)} class="mt-1 w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" /></label>
      <label>{$t('eyecare.duration_min')}<input type="number" {...numAttrs('longBreakMin')} value={cfg.longBreakMin} on:change={(e) => setNum('longBreakMin', e)} class="mt-1 w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" /></label>
    </div>
  </section>

  <!-- reminders -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-4">
    <h3 class="flex items-center gap-2 text-white font-medium"><Timer size={16} class="text-violet-300" /> {$t('eyecare.reminders')}</h3>
    {#each reminders as r}
      <div class="space-y-2">
        <label class="flex items-center justify-between gap-4">
          <span class="flex items-center gap-2 text-sm text-slate-200"><svelte:component this={r.icon} size={14} class="text-slate-400" /><span>{$t(r.label)}<span class="block text-xs text-slate-500">{$t(r.hint)}</span></span></span>
          <input type="checkbox" class="w-5 h-5" checked={cfg[r.on]} on:change={(e) => setBool(r.on, e.currentTarget.checked)} />
        </label>
        {#if cfg[r.on]}
          <label class="flex items-center gap-2 text-xs text-slate-400 pl-6">{$t('eyecare.every_min')}
            <input type="number" {...numAttrs(r.every)} value={cfg[r.every]} on:change={(e) => setNum(r.every, e)} class="w-20 bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" />
          </label>
        {/if}
      </div>
    {/each}
  </section>

  <!-- daily limit + late hours -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-4">
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('eyecare.limit')}<span class="block text-xs text-slate-500">{$t('eyecare.limit_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={cfg.dailyLimit} on:change={(e) => setBool('dailyLimit', e.currentTarget.checked)} />
    </label>
    {#if cfg.dailyLimit}
      <label class="flex items-center gap-2 text-xs text-slate-400">{$t('eyecare.limit_min')}
        <input type="number" {...numAttrs('dailyLimitMin')} step="30" value={cfg.dailyLimitMin} on:change={(e) => setNum('dailyLimitMin', e)} class="w-24 bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" />
      </label>
    {/if}

    <label class="flex items-center justify-between gap-4">
      <span class="flex items-center gap-2 text-sm text-slate-200"><Moon size={14} class="text-violet-300" /><span>{$t('eyecare.late')}<span class="block text-xs text-slate-500">{$t('eyecare.late_hint')}</span></span></span>
      <input type="checkbox" class="w-5 h-5" checked={cfg.lateHours} on:change={(e) => setBool('lateHours', e.currentTarget.checked)} />
    </label>
    {#if cfg.lateHours}
      <div class="flex items-center gap-3 text-sm pl-6">
        <input type="time" value={cfg.lateStart} on:change={(e) => setEyeCare({ lateStart: e.currentTarget.value || DEFAULT_EYECARE.lateStart })} class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" aria-label={$t('dnd.from')} />
        <span class="text-slate-500">→</span>
        <input type="time" value={cfg.lateEnd} on:change={(e) => setEyeCare({ lateEnd: e.currentTarget.value || DEFAULT_EYECARE.lateEnd })} class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" aria-label={$t('dnd.to')} />
      </div>
    {/if}
  </section>

  <!-- advanced -->
  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('eyecare.pause_dnd')}<span class="block text-xs text-slate-500">{$t('eyecare.pause_dnd_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={cfg.pauseInDnd} on:change={(e) => setBool('pauseInDnd', e.currentTarget.checked)} />
    </label>
    <label class="flex items-center gap-2 text-xs text-slate-400">{$t('eyecare.idle_sec')}
      <input type="number" {...numAttrs('idleSec')} step="10" value={cfg.idleSec} on:change={(e) => setNum('idleSec', e)} class="w-24 bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" />
    </label>
    <p class="text-xs text-slate-500">{$t('eyecare.idle_hint')}</p>
  </section>

  <p class="text-[11px] text-slate-600">{$t('eyecare.disclaimer')}</p>
</div>
