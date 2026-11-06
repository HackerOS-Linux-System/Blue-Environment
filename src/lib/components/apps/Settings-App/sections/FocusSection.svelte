<script lang="ts">
  import { Target, Timer, CalendarClock, ShieldCheck } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { APPS } from '../../../../constants';
  import { focusConfig, focusStatus, nextFocus, setFocusConfig, startFocus, startPomodoro, stopFocus, evaluate, endNeedsPin } from '../../../../stores/focusMode';

  let pinIsSet = false;
  let pin = '';
  let pinError = '';
  let askPin = false;
  let tick = Date.now();
  let keywordsText = $focusConfig.keywords.join(', ');

  SystemBridge.isTauri() && SystemBridge.invoke<boolean>('parental_controls_is_pin_set').then((v) => (pinIsSet = !!v)).catch(() => {});
  setInterval(() => (tick = Date.now()), 15000);

  $: status = (void tick, $focusStatus);
  $: needsPin = endNeedsPin($focusConfig, status, pinIsSet);
  $: endsText = status.endsAt ? new Date(status.endsAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '';
  $: apps = Object.values(APPS).map((a: any) => ({ id: String(a.id), title: String(a.title) })).sort((a, b) => a.title.localeCompare(b.title));
  const durations = [25, 45, 60, 90];

  const verifyPin = (p: string) => SystemBridge.invoke<boolean>('parental_controls_verify_pin', { pin: p });

  async function stop() {
    if (needsPin && !askPin) { askPin = true; return; }
    const ok = await stopFocus({ pinIsSet, verifyPin, pin });
    if (!ok) { pinError = $t('focus.pin_wrong'); return; }
    askPin = false; pin = ''; pinError = '';
    evaluate([]);
  }
  function begin(min: number | null) { startFocus(min); evaluate([]); }
  function pomodoro() { startPomodoro(4); evaluate([]); }
  function toggleApp(id: string, on: boolean) {
    const set = new Set($focusConfig.blockedApps);
    on ? set.add(id) : set.delete(id);
    setFocusConfig({ blockedApps: [...set] });
  }
  function saveKeywords() {
    setFocusConfig({ keywords: keywordsText.split(',').map((s) => s.trim()).filter(Boolean) });
  }
</script>

<div class="space-y-5 max-w-2xl">
  <p class="text-sm text-slate-400">{$t('focus.subtitle')}</p>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-4">
    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl grid place-items-center {status.active ? 'bg-emerald-600/30' : 'bg-white/5'}"><Target size={18} class={status.active ? 'text-emerald-300' : 'text-slate-400'} /></div>
        <div>
          <div class="text-white font-medium">{status.active ? $t('focus.on') : status.phase === 'break' ? $t('focus.break') : $t('focus.off')}</div>
          <div class="text-xs text-slate-500">
            {#if status.active || status.phase === 'break'}
              {status.source === 'calendar' ? $t('focus.from_calendar', { title: status.label }) : $t('focus.session')}{endsText ? ' · ' + $t('focus.until', { time: endsText }) : ''}
            {:else if $nextFocus}
              {$t('focus.next', { title: $nextFocus.title, time: new Date($nextFocus.start).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit' }) })}
            {:else}{$t('focus.hint')}{/if}
          </div>
        </div>
      </div>
      {#if status.source === 'manual' || status.source === 'pomodoro'}
        <button on:click={stop} class="px-4 py-2 rounded-lg text-sm bg-slate-700 hover:bg-slate-600">{$t('focus.stop')}</button>
      {/if}
    </div>

    {#if askPin}
      <div class="flex items-center gap-2">
        <input type="password" inputmode="numeric" bind:value={pin} placeholder={$t('focus.pin_placeholder')} class="bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white" on:keydown={(e) => e.key === 'Enter' && stop()} />
        <button on:click={stop} class="px-3 py-1.5 rounded-lg text-sm bg-blue-600 hover:bg-blue-500">{$t('focus.confirm')}</button>
        {#if pinError}<span class="text-xs text-red-300">{pinError}</span>{/if}
      </div>
    {/if}

    {#if !(status.source === 'manual' || status.source === 'pomodoro')}
      <div>
        <div class="text-xs text-slate-400 mb-2">{$t('focus.start_for')}</div>
        <div class="flex flex-wrap gap-2">
          {#each durations as m}<button on:click={() => begin(m)} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs">{m} min</button>{/each}
          <button on:click={() => begin(null)} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs">{$t('focus.until_stopped')}</button>
          <button on:click={pomodoro} class="px-3 py-1.5 rounded-lg bg-violet-600 hover:bg-violet-500 text-xs flex items-center gap-1"><Timer size={12} /> {$t('focus.pomodoro', { work: $focusConfig.workMinutes, brk: $focusConfig.breakMinutes })}</button>
        </div>
      </div>
    {/if}
  </section>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="flex items-center gap-2 text-white font-medium"><CalendarClock size={16} class="text-violet-300" /> {$t('focus.calendar')}</h3>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('focus.calendar_auto')}<span class="block text-xs text-slate-500">{$t('focus.calendar_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={$focusConfig.calendarAuto} on:change={(e) => setFocusConfig({ calendarAuto: e.currentTarget.checked })} />
    </label>
    <label class="block text-xs text-slate-400">{$t('focus.keywords')}
      <input bind:value={keywordsText} on:change={saveKeywords} class="mt-1 w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white" />
    </label>
    <div class="flex items-center gap-3 text-xs text-slate-400">
      <label>{$t('focus.work_min')} <input type="number" min="1" max="180" value={$focusConfig.workMinutes} on:change={(e) => setFocusConfig({ workMinutes: Number(e.currentTarget.value) })} class="ml-1 w-16 bg-slate-900 border border-white/10 rounded px-2 py-1 text-white" /></label>
      <label>{$t('focus.break_min')} <input type="number" min="0" max="60" value={$focusConfig.breakMinutes} on:change={(e) => setFocusConfig({ breakMinutes: Number(e.currentTarget.value) })} class="ml-1 w-16 bg-slate-900 border border-white/10 rounded px-2 py-1 text-white" /></label>
    </div>
  </section>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="flex items-center gap-2 text-white font-medium"><ShieldCheck size={16} class="text-violet-300" /> {$t('focus.rules')}</h3>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('focus.allow_urgent')}<span class="block text-xs text-slate-500">{$t('focus.allow_urgent_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={$focusConfig.allowUrgent} on:change={(e) => setFocusConfig({ allowUrgent: e.currentTarget.checked })} />
    </label>
    <label class="flex items-center justify-between gap-4 {pinIsSet ? '' : 'opacity-60'}">
      <span class="text-sm text-slate-200">{$t('focus.strict')}<span class="block text-xs text-slate-500">{pinIsSet ? $t('focus.strict_hint') : $t('focus.strict_no_pin')}</span></span>
      <input type="checkbox" class="w-5 h-5" disabled={!pinIsSet} checked={$focusConfig.strict} on:change={(e) => setFocusConfig({ strict: e.currentTarget.checked })} />
    </label>
    <div>
      <div class="text-xs text-slate-400 mb-2">{$t('focus.blocked_apps')}</div>
      <div class="max-h-48 overflow-y-auto grid grid-cols-2 gap-x-4 gap-y-1">
        {#each apps as a (a.id)}
          <label class="flex items-center gap-2 text-sm text-slate-200 truncate"><input type="checkbox" checked={$focusConfig.blockedApps.includes(a.id)} on:change={(e) => toggleApp(a.id, e.currentTarget.checked)} /> {a.title}</label>
        {/each}
      </div>
    </div>
  </section>
</div>
