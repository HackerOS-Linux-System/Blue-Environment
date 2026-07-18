<script lang="ts">
  import type { UserConfig } from '../../../../types';
  import { applyNightLight } from '../display_helpers';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  async function toggle(e: Event) {
    const v = (e.currentTarget as HTMLInputElement).checked;
    await onSave({ nightLightEnabled: v });
    await applyNightLight(v, config.nightLightTemperature ?? 4000);
  }
  async function changeTemp(e: Event) {
    const v = parseInt((e.currentTarget as HTMLInputElement).value);
    await onSave({ nightLightTemperature: v });
    await applyNightLight(true, v);
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">Night Light</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <div class="flex items-center justify-between mb-4">
      <label class="text-sm font-medium text-slate-400">Night Light</label>
      <input type="checkbox" checked={config.nightLightEnabled ?? false} on:change={toggle} class="w-4 h-4 accent-blue-500" />
    </div>
    {#if config.nightLightEnabled}
      <div>
        <label class="block text-sm font-medium text-slate-400 mb-1">Color Temperature (K)</label>
        <input type="range" min="1000" max="10000" step="100" value={config.nightLightTemperature ?? 4000} on:change={changeTemp}
          class="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-orange-400" />
        <div class="flex justify-between text-xs text-slate-500 mt-1">
          <span>1000K</span>
          <span>{config.nightLightTemperature ?? 4000}K</span>
          <span>10000K</span>
        </div>
      </div>
    {/if}
  </div>
</div>
