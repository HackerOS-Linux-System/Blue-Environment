<script lang="ts">
  import { RefreshCw, Lock, Unlock, Loader2, Wifi } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { dialogAlert } from '../../../../stores/dialog';

  interface WifiNetwork { ssid: string; signal: number; secure: boolean; in_use: boolean; }

  let networks: WifiNetwork[] = [];
  let scanning = false;
  let enabled = true;
  let connecting: string | null = null;

  async function scan() { scanning = true; networks = await SystemBridge.getWifiNetworks(); scanning = false; }
  async function connect(ssid: string) {
    connecting = ssid;
    try { await SystemBridge.connectWifi(ssid, ''); await scan(); }
    catch { await dialogAlert({ title: 'Connection failed', message: `Could not connect to "${ssid}". Check the password and try again.` }); }
    finally { connecting = null; }
  }
  async function toggle() { enabled = !enabled; await SystemBridge.toggleWifi(enabled); if (enabled) scan(); }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-white">Wi-Fi</h2>
    <button on:click={scan} disabled={scanning} class="p-2 bg-slate-800 rounded-full hover:bg-white/10"><RefreshCw size={18} class={scanning ? 'animate-spin' : ''} /></button>
  </div>
  <div class="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
    <span class="text-white">Wi-Fi</span>
    <button on:click={toggle} class="w-12 h-6 rounded-full transition-colors relative {enabled ? 'bg-blue-600' : 'bg-slate-600'}">
      <div class="w-4 h-4 rounded-full bg-white absolute top-1 transition-transform {enabled ? 'translate-x-7' : 'translate-x-1'}" />
    </button>
  </div>
  <div class="bg-slate-800 border border-white/5 rounded-2xl overflow-hidden">
    {#if networks.length === 0 && !scanning}
      <div class="p-8 text-center text-slate-500 cursor-pointer hover:text-white" on:click={scan}>No networks found — click to scan</div>
    {/if}
    {#if scanning}<div class="p-4 text-center flex items-center justify-center gap-2 text-slate-500"><Loader2 size={16} class="animate-spin" /> Scanning...</div>{/if}
    {#each networks as net, i (i)}
      <div class="flex items-center justify-between p-4 border-b border-white/5 last:border-0 {net.in_use ? 'bg-blue-600/10' : 'hover:bg-white/5'}">
        <div class="flex items-center gap-3">
          <Wifi size={20} class={net.signal > 60 ? 'text-green-400' : 'text-yellow-400'} />
          <div>
            <div class="font-medium text-white flex items-center gap-2">
              {net.ssid}
              {#if net.in_use}<span class="text-xs bg-green-500/20 text-green-400 px-2 rounded-full">Connected</span>{/if}
            </div>
            <div class="text-xs text-slate-400 flex items-center gap-1">
              {#if net.secure}<Lock size={10} />{:else}<Unlock size={10} />{/if}
              {net.secure ? 'Secured' : 'Open'} · {net.signal}%
            </div>
          </div>
        </div>
        {#if net.in_use}
          <button on:click={() => SystemBridge.disconnectWifi().then(scan)} class="px-4 py-2 bg-red-500/20 text-red-400 rounded-lg text-sm hover:bg-red-500/40 transition-colors">Disconnect</button>
        {:else}
          <button on:click={() => connect(net.ssid)} disabled={connecting !== null} class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm disabled:opacity-50 transition-colors">
            {connecting === net.ssid ? 'Connecting...' : 'Connect'}
          </button>
        {/if}
      </div>
    {/each}
  </div>
</div>
