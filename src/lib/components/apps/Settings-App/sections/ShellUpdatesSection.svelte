<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { t, translate } from '../../../../stores/language';

  interface Status {
    autoUpdate: boolean; ignoredVersion?: string | null; stagedVersion?: string | null;
    availableVersion?: string | null; lastCheck?: string | null; runningVersion: string;
  }
  let st: Status | null = null;
  let busy = false;
  let msg = '';

  async function refresh() { try { st = await invoke<Status>('shell_update_state'); } catch (e) { msg = String(e); } }
  onMount(refresh);

  async function setAuto(v: boolean) { await invoke('shell_update_set_auto', { enabled: v }); await refresh(); }
  async function checkNow() {
    busy = true; msg = '';
    try {
      const r = await invoke<{ version: string } | null>('shell_update_check_now');
      msg = r ? translate('supd.msg_available', { v: r.version }) : translate('supd.msg_latest');
    } catch (e) { msg = translate('supd.msg_check_failed', { e: String(e) }); }
    busy = false; await refresh();
  }
  const installAvailable = () => { if (st?.availableVersion) installNow(st.availableVersion); };
  async function installNow(v: string) {
    busy = true; msg = '';
    try { await invoke('shell_update_install', { version: v }); msg = translate('supd.msg_staged'); }
    catch (e) { msg = translate('supd.msg_failed', { e: String(e) }); }
    busy = false; await refresh();
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('supd.title')}</h2>
  {#if st}
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4 text-sm text-slate-200">
      <div class="flex justify-between"><span>{$t('supd.running')}</span><span class="tabular-nums">{st.runningVersion}</span></div>
      {#if st.stagedVersion}<div class="flex justify-between"><span>{$t('supd.staged')}</span><span class="tabular-nums text-green-400">{st.stagedVersion}</span></div>{/if}
      {#if st.availableVersion}<div class="flex justify-between"><span>{$t('supd.available')}</span><span class="tabular-nums text-blue-400">{st.availableVersion}</span></div>{/if}
      {#if st.lastCheck}<div class="flex justify-between text-slate-500"><span>{$t('supd.last_check')}</span><span>{new Date(st.lastCheck).toLocaleString()}</span></div>{/if}
      <label class="flex items-center justify-between">
        <span>{$t('supd.auto')}<span class="block text-xs text-slate-500">{$t('supd.auto_hint')}</span></span>
        <input type="checkbox" checked={st.autoUpdate} on:change={(e) => setAuto(e.currentTarget.checked)} />
      </label>
      <div class="flex gap-2">
        <button class="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/20 text-white disabled:opacity-50" disabled={busy} on:click={checkNow}>{$t('supd.check')}</button>
        {#if st.availableVersion}
          <button class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-50" disabled={busy} on:click={installAvailable}>{$t('supd.install')}</button>
        {/if}
      </div>
      {#if msg}<div class="text-xs text-slate-400">{msg}</div>{/if}
    </div>
  {:else}
    <div class="text-slate-400 text-sm">{msg || $t('supd.loading')}</div>
  {/if}
</div>
