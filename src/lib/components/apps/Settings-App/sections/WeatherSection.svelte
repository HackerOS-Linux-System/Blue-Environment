<script lang="ts">
  import { CloudSun, Clipboard, Gauge } from 'lucide-svelte';
  import type { UserConfig } from '../../../../types';
  import { t } from '../../../../stores/language';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  let cityDraft = config.weatherCity ?? '';
  $: cityDraft = config.weatherCity ?? cityDraft;
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.weather.title')}</h2>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-5">
    <div class="flex items-center gap-3 text-slate-400 mb-1">
      <CloudSun size={16} class="text-blue-400" />
      <span class="text-sm">{$t('settings.weather.widget_group')}</span>
    </div>

    <div class="flex items-center justify-between">
      <div>
        <div class="text-sm text-white">{$t('settings.weather.show_in_topbar')}</div>
        <div class="text-xs text-slate-500">{$t('settings.weather.show_in_topbar_desc')}</div>
      </div>
      <input type="checkbox" checked={config.weatherEnabled ?? true}
        on:change={(e) => onSave({ weatherEnabled: e.currentTarget.checked })} class="w-4 h-4 accent-blue-500" />
    </div>

    <div>
      <label class="block text-xs text-slate-500 mb-1.5">{$t('settings.weather.city_override')}</label>
      <div class="flex gap-2">
        <input bind:value={cityDraft} placeholder={$t('settings.weather.city_placeholder')}
          class="flex-1 bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
        <button on:click={() => onSave({ weatherCity: cityDraft.trim() })}
          class="px-3 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm text-white">{$t('settings.common.save')}</button>
      </div>
    </div>

    <div>
      <label class="block text-xs text-slate-500 mb-1.5">{$t('settings.weather.temp_unit')}</label>
      <div class="flex gap-2">
        <button on:click={() => onSave({ weatherUnit: 'celsius' })}
          class="px-3 py-1.5 rounded-lg text-sm {((config.weatherUnit ?? 'celsius') === 'celsius') ? 'bg-blue-600 text-white' : 'bg-slate-900 text-slate-400 hover:text-white'}">{$t('settings.weather.celsius')}</button>
        <button on:click={() => onSave({ weatherUnit: 'fahrenheit' })}
          class="px-3 py-1.5 rounded-lg text-sm {(config.weatherUnit === 'fahrenheit') ? 'bg-blue-600 text-white' : 'bg-slate-900 text-slate-400 hover:text-white'}">{$t('settings.weather.fahrenheit')}</button>
      </div>
    </div>
    <p class="text-xs text-slate-500">{$t('settings.weather.detail_hint')}</p>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="flex items-center gap-3 text-slate-400 mb-1">
      <Clipboard size={16} class="text-blue-400" />
      <span class="text-sm">{$t('settings.weather.clipboard_group')}</span>
    </div>
    <div class="flex items-center justify-between">
      <div>
        <div class="text-sm text-white">{$t('settings.weather.clipboard_show')}</div>
        <div class="text-xs text-slate-500">{$t('settings.weather.clipboard_show_desc')}</div>
      </div>
      <input type="checkbox" checked={config.clipboardHoverPreviewEnabled ?? true}
        on:change={(e) => onSave({ clipboardHoverPreviewEnabled: e.currentTarget.checked })} class="w-4 h-4 accent-blue-500" />
    </div>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="flex items-center gap-3 text-slate-400 mb-1">
      <Gauge size={16} class="text-blue-400" />
      <span class="text-sm">{$t('settings.weather.network_group')}</span>
    </div>
    <div class="flex items-center justify-between">
      <div>
        <div class="text-sm text-white">{$t('settings.weather.network_show')}</div>
        <div class="text-xs text-slate-500">{$t('settings.weather.network_show_desc')}</div>
      </div>
      <input type="checkbox" checked={config.networkHoverInfoEnabled ?? true}
        on:change={(e) => onSave({ networkHoverInfoEnabled: e.currentTarget.checked })} class="w-4 h-4 accent-blue-500" />
    </div>
  </div>
</div>
