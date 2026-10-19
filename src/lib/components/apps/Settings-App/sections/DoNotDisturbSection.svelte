<script lang="ts">
  import { BellOff, Moon, Clock } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { dnd, setDnd, enableDndFor, disableDnd, isDndActive, minutesLeft, DEFAULT_DND } from '../../../../stores/doNotDisturb';

  let tick = Date.now();
  setInterval(() => (tick = Date.now()), 15000);
  $: active = (void tick, isDndActive($dnd));
  $: left = (void tick, minutesLeft($dnd));

  const durations = [15, 30, 60, 120, 480];
  const label = (m: number) => (m < 60 ? `${m} min` : `${m / 60} h`);
</script>

<div class="space-y-5 max-w-2xl">
  <p class="text-sm text-slate-400">{$t('dnd.subtitle')}</p>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-4">
    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl grid place-items-center {active ? 'bg-violet-600/30' : 'bg-white/5'}"><BellOff size={18} class={active ? 'text-violet-300' : 'text-slate-400'} /></div>
        <div>
          <div class="text-white font-medium">{active ? $t('dnd.on') : $t('dnd.off')}</div>
          <div class="text-xs text-slate-500">{left ? $t('dnd.until_left').replace('{n}', String(left)) : $t('dnd.hint')}</div>
        </div>
      </div>
      <button on:click={() => (active ? disableDnd() : setDnd({ mode: 'on', untilTs: null }))} class="px-4 py-2 rounded-lg text-sm {active ? 'bg-slate-700 hover:bg-slate-600' : 'bg-violet-600 hover:bg-violet-500'}">{active ? $t('dnd.turn_off') : $t('dnd.turn_on')}</button>
    </div>

    <div>
      <div class="text-xs text-slate-400 mb-2">{$t('dnd.for')}</div>
      <div class="flex flex-wrap gap-2">{#each durations as m}<button on:click={() => enableDndFor(m)} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs">{label(m)}</button>{/each}</div>
    </div>
  </section>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
    <h3 class="flex items-center gap-2 text-white font-medium"><Moon size={16} class="text-violet-300" /> {$t('dnd.schedule')}</h3>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('dnd.schedule_enable')}<span class="block text-xs text-slate-500">{$t('dnd.schedule_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={$dnd.mode === 'schedule'} on:change={(e) => (e.currentTarget.checked ? setDnd({ mode: 'schedule', untilTs: null }) : disableDnd())} />
    </label>
    <div class="flex items-center gap-3 text-sm {$dnd.mode === 'schedule' ? '' : 'opacity-50'}">
      <Clock size={14} class="text-slate-400" />
      <input type="time" value={$dnd.scheduleStart} on:change={(e) => setDnd({ scheduleStart: e.currentTarget.value || DEFAULT_DND.scheduleStart })} class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" aria-label={$t('dnd.from')} />
      <span class="text-slate-500">→</span>
      <input type="time" value={$dnd.scheduleEnd} on:change={(e) => setDnd({ scheduleEnd: e.currentTarget.value || DEFAULT_DND.scheduleEnd })} class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" aria-label={$t('dnd.to')} />
    </div>
  </section>

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5">
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm text-slate-200">{$t('dnd.allow_urgent')}<span class="block text-xs text-slate-500">{$t('dnd.allow_urgent_hint')}</span></span>
      <input type="checkbox" class="w-5 h-5" checked={$dnd.allowUrgent} on:change={(e) => setDnd({ allowUrgent: e.currentTarget.checked })} />
    </label>
  </section>
</div>
