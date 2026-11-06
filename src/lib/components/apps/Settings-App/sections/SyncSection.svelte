<script lang="ts">
  import { RefreshCw, Smartphone, Upload } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { syncNow, sendMySettings } from '../../../../stores/settingsSync';

  interface Peer { deviceId: string; name: string; enabled: boolean; lastSync: number | null; lastError: string | null }
  let peers: Peer[] = [];
  let fields: string[] = [];
  let busy = '';
  let message = '';
  let showFields = false;

  async function refresh() {
    try {
      const st = await SystemBridge.invoke<{ peers: Peer[]; fields: string[] }>('sync_status');
      peers = st.peers; fields = st.fields;
    } catch { peers = []; }
  }
  refresh();

  async function toggle(p: Peer, enabled: boolean) {
    try { await SystemBridge.invoke('sync_set_device_enabled', { deviceId: p.deviceId, enabled }); } catch (e) { message = String(e); }
    await refresh();
  }
  async function run(p: Peer, push: boolean) {
    busy = p.deviceId; message = '';
    try {
      const n = push ? await sendMySettings(p.deviceId) : await syncNow(p.deviceId);
      message = $t('sync.done', { n });
    } catch (e) { message = String(e); }
    busy = ''; await refresh();
  }
  const when = (ts: number | null) => (ts ? new Date(ts).toLocaleString() : $t('sync.never'));
</script>

<div class="space-y-5 max-w-2xl">
  <p class="text-sm text-slate-400">{$t('sync.subtitle')}</p>

  {#if peers.length === 0}
    <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 text-sm text-slate-300">{$t('sync.no_devices')}</section>
  {/if}

  {#each peers as p (p.deviceId)}
    <section class="bg-slate-800 rounded-2xl p-5 border border-white/5 space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl grid place-items-center bg-white/5"><Smartphone size={18} class="text-slate-300" /></div>
          <div>
            <div class="text-white font-medium">{p.name}</div>
            <div class="text-xs text-slate-500">{$t('sync.last')}: {when(p.lastSync)}</div>
          </div>
        </div>
        <label class="flex items-center gap-2 text-sm text-slate-200">{$t('sync.enable')}
          <input type="checkbox" class="w-5 h-5" checked={p.enabled} on:change={(e) => toggle(p, e.currentTarget.checked)} />
        </label>
      </div>
      {#if p.enabled}
        <div class="flex flex-wrap gap-2">
          <button disabled={busy === p.deviceId} on:click={() => run(p, false)} class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-xs flex items-center gap-1.5 disabled:opacity-50"><RefreshCw size={12} class={busy === p.deviceId ? 'animate-spin' : ''} /> {$t('sync.now')}</button>
          <button disabled={busy === p.deviceId} on:click={() => run(p, true)} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 text-xs flex items-center gap-1.5 disabled:opacity-50"><Upload size={12} /> {$t('sync.send_mine')}</button>
        </div>
        <p class="text-[11px] text-slate-500">{$t('sync.both_hint')}</p>
      {/if}
      {#if p.lastError}<p class="text-xs text-red-300">{p.lastError}</p>{/if}
    </section>
  {/each}

  {#if message}<p class="text-xs text-slate-300" role="status">{message}</p>{/if}

  <section class="bg-slate-800 rounded-2xl p-5 border border-white/5">
    <button class="text-sm text-slate-200 underline-offset-2 hover:underline" on:click={() => (showFields = !showFields)}>{$t('sync.what')}</button>
    {#if showFields}
      <p class="mt-2 text-xs text-slate-400">{$t('sync.what_hint')}</p>
      <p class="mt-2 text-xs font-mono text-slate-500 break-words">{fields.join(' · ')}</p>
    {/if}
  </section>
</div>
