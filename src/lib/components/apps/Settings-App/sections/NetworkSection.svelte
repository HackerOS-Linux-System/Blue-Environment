<script lang="ts">
  import { Wifi, Bluetooth, Signal, Plane } from 'lucide-svelte';
  import WifiSection from './WifiSection.svelte';
  import BluetoothSection from './BluetoothSection.svelte';
  import CellularSection from './CellularSection.svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';

  type SubTab = 'wifi' | 'bluetooth' | 'cellular';
  let subTab: SubTab = 'wifi';
  let airplaneMode = false;
  let togglingAirplane = false;

  const SUB_TABS: { id: SubTab; labelKey: string; icon: any }[] = [
    { id: 'wifi', labelKey: 'settings.network.tab_wifi', icon: Wifi },
    { id: 'bluetooth', labelKey: 'settings.network.tab_bluetooth', icon: Bluetooth },
    { id: 'cellular', labelKey: 'settings.network.tab_cellular', icon: Signal },
  ];

  // Airplane mode gives a single switch that mirrors what most OSes do: cut every radio at once.
  // Individual sections below still expose their own per-radio toggle for finer control.
  async function toggleAirplaneMode() {
    togglingAirplane = true;
    airplaneMode = !airplaneMode;
    try {
      await SystemBridge.toggleWifi(!airplaneMode);
      await SystemBridge.setCellularEnabled(!airplaneMode).catch(() => {});
    } finally {
      togglingAirplane = false;
    }
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-white">{$t('settings.network.title')}</h2>
    <button on:click={toggleAirplaneMode} disabled={togglingAirplane}
      class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm border {airplaneMode ? 'bg-amber-500/20 border-amber-500/50 text-amber-400' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
      <Plane size={14} /> {$t('settings.network.airplane_mode')} {airplaneMode ? $t('settings.network.on') : $t('settings.network.off')}
    </button>
  </div>

  <div class="flex gap-1.5 bg-slate-800/60 border border-white/5 rounded-xl p-1 w-fit">
    {#each SUB_TABS as tab (tab.id)}
      <button on:click={() => (subTab = tab.id)}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-colors {subTab === tab.id ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white'}">
        <svelte:component this={tab.icon} size={14} />{$t(tab.labelKey)}
      </button>
    {/each}
  </div>

  {#if airplaneMode}
    <div class="bg-slate-800 border border-white/5 rounded-2xl p-8 text-center text-slate-500 text-sm">
      {$t('settings.network.airplane_active')}
    </div>
  {:else if subTab === 'wifi'}
    <WifiSection />
  {:else if subTab === 'bluetooth'}
    <BluetoothSection />
  {:else if subTab === 'cellular'}
    <CellularSection />
  {/if}
</div>
