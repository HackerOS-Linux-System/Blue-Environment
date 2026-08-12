<script lang="ts">
  import { onMount } from 'svelte';
  import type { UserConfig } from '../../../../types';
  import { applyNightLight, getGeoLocation, computeSunTimes, type GeoCoords } from '../display_helpers';
  import { t } from '../../../../stores/language';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  let geo: GeoCoords | null = null;
  let geoDenied = false;
  let sunTimes: { sunrise: string; sunset: string } | null = null;

  $: schedule = config.nightLightSchedule ?? 'manual';

  async function resolveGeo() {
    geo = await getGeoLocation();
    geoDenied = !geo;
    sunTimes = geo ? computeSunTimes(geo.lat, geo.lon) : null;
  }

  onMount(() => {
    if (schedule === 'sunset') resolveGeo();
  });

  async function toggle(e: Event) {
    const v = (e.currentTarget as HTMLInputElement).checked;
    await onSave({ nightLightEnabled: v });
    await applyNightLight(v, config.nightLightTemperature ?? 4000, schedule, geo);
  }

  async function changeTemp(e: Event) {
    const v = parseInt((e.currentTarget as HTMLInputElement).value);
    await onSave({ nightLightTemperature: v });
    await applyNightLight(true, v, schedule, geo);
  }

  async function changeSchedule(mode: 'manual' | 'sunset') {
    await onSave({ nightLightSchedule: mode });
    if (mode === 'sunset' && !geo) await resolveGeo();
    if (config.nightLightEnabled) await applyNightLight(true, config.nightLightTemperature ?? 4000, mode, geo);
  }

  async function changeHour(field: 'nightLightStartHour' | 'nightLightEndHour', e: Event) {
    const v = parseInt((e.currentTarget as HTMLSelectElement).value);
    await onSave({ [field]: v } as Partial<UserConfig>);
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.nightlight.title')}</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-5">
    <div class="flex items-center justify-between">
      <label class="text-sm font-medium text-slate-400">{$t('settings.nightlight.title')}</label>
      <input type="checkbox" checked={config.nightLightEnabled ?? false} on:change={toggle} class="w-4 h-4 accent-blue-500" />
    </div>

    {#if config.nightLightEnabled}
      <div>
        <label class="block text-sm font-medium text-slate-400 mb-1">{$t('settings.nightlight.temperature')}</label>
        <input type="range" min="1000" max="10000" step="100" value={config.nightLightTemperature ?? 4000} on:change={changeTemp}
          class="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-orange-400" />
        <div class="flex justify-between text-xs text-slate-500 mt-1">
          <span>1000K</span>
          <span>{config.nightLightTemperature ?? 4000}K</span>
          <span>10000K</span>
        </div>
      </div>

      <div>
        <label class="block text-sm font-medium text-slate-400 mb-2">{$t('settings.nightlight.schedule')}</label>
        <div class="flex gap-2">
          <button on:click={() => changeSchedule('manual')}
            class="flex-1 py-2 rounded-lg text-sm border {schedule === 'manual' ? 'bg-blue-600/20 border-blue-500 text-blue-400' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
            {$t('settings.nightlight.manual')}
          </button>
          <button on:click={() => changeSchedule('sunset')}
            class="flex-1 py-2 rounded-lg text-sm border {schedule === 'sunset' ? 'bg-blue-600/20 border-blue-500 text-blue-400' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
            {$t('settings.nightlight.sunset_to_sunrise')}
          </button>
        </div>
      </div>

      {#if schedule === 'sunset'}
        <div class="rounded-xl bg-slate-900/60 border border-white/5 p-3 text-xs text-slate-400 space-y-1.5">
          {#if geo && sunTimes}
            <div class="flex justify-between"><span>{$t('settings.nightlight.sunset_today')}</span><span class="text-slate-200">{sunTimes.sunset}</span></div>
            <div class="flex justify-between"><span>{$t('settings.nightlight.sunrise_tomorrow')}</span><span class="text-slate-200">{sunTimes.sunrise}</span></div>
            <div class="text-[11px] text-slate-500 pt-1">{$t('settings.nightlight.location_based').replace('{lat}', geo.lat.toFixed(2)).replace('{lon}', geo.lon.toFixed(2))}</div>
          {:else if geoDenied}
            <div class="text-amber-400">{$t('settings.nightlight.location_unavailable')}</div>
            <button on:click={resolveGeo} class="text-blue-400 hover:underline">{$t('settings.nightlight.try_location_again')}</button>
          {:else}
            <div>{$t('settings.nightlight.resolving')}</div>
          {/if}
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-xs text-slate-500 mb-1">{$t('settings.nightlight.start_hour')}</label>
            <select value={config.nightLightStartHour ?? 20} on:change={(e) => changeHour('nightLightStartHour', e)}
              class="w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm">
              {#each Array(24) as _, h}<option value={h}>{String(h).padStart(2, '0')}:00</option>{/each}
            </select>
          </div>
          <div>
            <label class="block text-xs text-slate-500 mb-1">{$t('settings.nightlight.end_hour')}</label>
            <select value={config.nightLightEndHour ?? 6} on:change={(e) => changeHour('nightLightEndHour', e)}
              class="w-full bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm">
              {#each Array(24) as _, h}<option value={h}>{String(h).padStart(2, '0')}:00</option>{/each}
            </select>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>
