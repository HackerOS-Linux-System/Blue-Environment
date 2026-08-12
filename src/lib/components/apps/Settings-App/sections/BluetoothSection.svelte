<script lang="ts">
  import { RefreshCw } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';

  interface BtDevice { name: string; mac: string; connected: boolean; device_type: string; battery?: number; }

  let devices: BtDevice[] = [];
  let scanning = false;
  let enabled = true;

  async function scan() { scanning = true; devices = await SystemBridge.getBluetoothDevices(); scanning = false; }
  async function toggleDevice(mac: string) { await SystemBridge.toggleBluetoothDevice(mac); scan(); }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-white">{$t('settings.bluetooth.title')}</h2>
    <button on:click={scan} class="p-2 bg-slate-800 rounded-full hover:bg-white/10"><RefreshCw size={18} class={scanning ? 'animate-spin' : ''} /></button>
  </div>
  <div class="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
    <span class="text-white">{$t('settings.bluetooth.title')}</span>
    <button on:click={() => (enabled = !enabled)} class="w-12 h-6 rounded-full transition-colors relative {enabled ? 'bg-blue-600' : 'bg-slate-600'}">
      <div class="w-4 h-4 rounded-full bg-white absolute top-1 transition-transform {enabled ? 'translate-x-7' : 'translate-x-1'}" />
    </button>
  </div>
  <div class="bg-slate-800 border border-white/5 rounded-2xl overflow-hidden">
    {#if devices.length === 0 && !scanning}
      <div class="p-8 text-center text-slate-500 cursor-pointer hover:text-white" on:click={scan}>{$t('settings.bluetooth.no_devices')}</div>
    {/if}
    {#each devices as dev, i (i)}
      <div class="flex items-center justify-between p-4 border-b border-white/5 last:border-0 hover:bg-white/5">
        <div>
          <div class="font-medium text-white">{dev.name}</div>
          <div class="text-xs text-slate-400">{dev.device_type} · {dev.connected ? $t('settings.common.connected') : $t('settings.common.disconnected')}{dev.battery != null ? ` · ${dev.battery}%` : ''}</div>
        </div>
        <button on:click={() => toggleDevice(dev.mac)} class="px-4 py-2 rounded-lg text-sm transition-colors {dev.connected ? 'bg-red-500/20 text-red-400 hover:bg-red-500/40' : 'bg-blue-600 hover:bg-blue-500 text-white'}">
          {dev.connected ? $t('settings.common.disconnect') : $t('settings.common.connect')}
        </button>
      </div>
    {/each}
  </div>
</div>
