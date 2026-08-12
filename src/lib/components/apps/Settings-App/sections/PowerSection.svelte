<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Battery, Zap, Wind, Check } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';
  import type { PowerProfile } from '../../../../types';

  let profile = 'balanced';
  let profiles: PowerProfile[] = [];
  let battery = { percentage: 100, charging: false };
  let interval: ReturnType<typeof setInterval>;

  onMount(() => {
    SystemBridge.getPowerProfiles().then((p) => (profiles = p));
    const refresh = () => SystemBridge.getSystemStats().then((s: any) => (battery = { percentage: s.battery ?? 100, charging: s.isCharging ?? false }));
    refresh();
    interval = setInterval(refresh, 30000);
  });
  onDestroy(() => clearInterval(interval));

  function iconFor(icon: string) { return icon === 'Zap' ? Zap : icon === 'Wind' ? Wind : Battery; }

  async function selectProfile(p: PowerProfile) { profile = p.name; await SystemBridge.setPowerProfile(p.name); }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.power.title')}</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <div class="flex items-center gap-4">
      <div class="p-4 bg-blue-600/20 rounded-full">
        <Battery size={32} class={battery.percentage < 20 ? 'text-red-400' : 'text-green-400'} />
      </div>
      <div>
        <div class="text-3xl font-bold text-white">{battery.percentage}%</div>
        <div class="text-slate-400">{battery.charging ? $t('settings.power.charging') : $t('settings.power.on_battery')}</div>
      </div>
    </div>
  </div>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <h3 class="text-lg font-semibold text-white mb-4">{$t('settings.power.profiles')}</h3>
    <div class="space-y-2">
      {#each profiles as p (p.name)}
        {@const Icon = iconFor(p.icon ?? 'Battery')}
        <button on:click={() => selectProfile(p)}
          class="w-full flex items-center justify-between p-4 rounded-xl border transition-all {profile === p.name ? 'bg-blue-600/20 border-blue-500' : 'bg-slate-900 border-white/5 hover:bg-slate-700'}">
          <div class="flex items-center gap-3">
            <svelte:component this={Icon} size={20} />
            <div class="text-left">
              <div class="font-medium text-white">{p.name}</div>
              <div class="text-xs text-slate-400">{p.description}</div>
            </div>
          </div>
          {#if profile === p.name}<Check size={20} class="text-blue-400" />{/if}
        </button>
      {/each}
    </div>
  </div>
</div>
