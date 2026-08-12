<script lang="ts">
  import { onMount } from 'svelte';
  import { Signal, RefreshCw } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';

  let hasModem = false;
  let status: { connected: boolean; signal: number; carrier: string } | null = null;
  let enabled = true;
  let loading = true;
  let busy = false;

  async function refresh() {
    loading = true;
    hasModem = await SystemBridge.hasCellularModem();
    if (hasModem) status = await SystemBridge.getCellularStatus();
    loading = false;
  }

  async function toggle() {
    busy = true;
    enabled = !enabled;
    await SystemBridge.setCellularEnabled(enabled);
    if (enabled) status = await SystemBridge.getCellularStatus();
    busy = false;
  }

  onMount(refresh);
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h2 class="text-2xl font-bold text-white">{$t('settings.cellular.title')}</h2>
    <button on:click={refresh} class="p-2 bg-slate-800 rounded-full hover:bg-white/10"><RefreshCw size={18} class={loading ? 'animate-spin' : ''} /></button>
  </div>

  {#if loading}
    <div class="text-slate-500 text-sm">{$t('settings.cellular.checking')}</div>
  {:else if !hasModem}
    <div class="bg-slate-800 border border-white/5 rounded-2xl p-8 text-center text-slate-500 text-sm">
      {$t('settings.cellular.no_modem')}
    </div>
  {:else}
    <div class="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
      <span class="text-white">{$t('settings.cellular.mobile_data')}</span>
      <button on:click={toggle} disabled={busy} class="w-12 h-6 rounded-full transition-colors relative {enabled ? 'bg-blue-600' : 'bg-slate-600'}">
        <div class="w-4 h-4 rounded-full bg-white absolute top-1 transition-transform {enabled ? 'translate-x-7' : 'translate-x-1'}" />
      </button>
    </div>
    <div class="bg-slate-800 border border-white/5 rounded-2xl p-4">
      {#if status}
        <div class="flex items-center gap-3">
          <Signal size={20} class={status.connected ? 'text-green-400' : 'text-slate-500'} />
          <div>
            <div class="font-medium text-white">{status.carrier || $t('settings.cellular.unknown_carrier')}</div>
            <div class="text-xs text-slate-400">{status.connected ? $t('settings.cellular.connected_signal').replace('{pct}', String(status.signal)) : $t('settings.cellular.not_connected')}</div>
          </div>
        </div>
      {:else}
        <div class="text-slate-500 text-sm text-center py-4">{$t('settings.cellular.no_status')}</div>
      {/if}
    </div>
  {/if}
</div>
