<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { ShieldCheck, ShieldAlert, Loader2, Server } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';

  /** The PIN the person just unlocked this section with (held in memory only). */
  export let pin: string;
  export let totalLimit: number | null = null;

  const dispatch = createEventDispatcher<{ changed: void }>();

  interface SystemStatus { supported: boolean; enforced: boolean; serviceActive: boolean; username: string; ruleCount: number }

  let status: SystemStatus | null = null;
  let busy = false;
  let error = '';
  let info = '';
  let limitInput = totalLimit ? String(totalLimit) : '';

  onMount(refresh);

  async function refresh() {
    status = await SystemBridge.invokeCommand<SystemStatus>('parental_system_status').catch(() => null);
  }

  async function saveLimit() {
    const n = limitInput.trim() === '' ? null : Math.max(0, parseInt(limitInput, 10) || 0);
    const ok = await SystemBridge.invokeCommand<boolean>('parental_controls_set_total_limit', { minutes: n, pin }).catch(() => false);
    if (!ok) { error = $t('settings.parental.system.limit_failed'); return; }
    error = ''; info = $t('settings.parental.system.limit_saved');
    dispatch('changed');
    // The limit only counts at system level after the policy is re-synced.
    if (status?.enforced) await enable();
  }

  async function enable() {
    busy = true; error = ''; info = '';
    try {
      await SystemBridge.invokeCommand('parental_system_enable', { pin });
      info = $t('settings.parental.system.enabled_ok');
    } catch (e: any) { error = String(e?.message ?? e); }
    finally { busy = false; await refresh(); }
  }

  async function disable() {
    busy = true; error = ''; info = '';
    try {
      await SystemBridge.invokeCommand('parental_system_disable', { pin });
      info = $t('settings.parental.system.disabled_ok');
    } catch (e: any) { error = String(e?.message ?? e); }
    finally { busy = false; await refresh(); }
  }
</script>

<div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
  <div class="flex items-start gap-3">
    {#if status?.enforced && status.serviceActive}
      <ShieldCheck class="w-6 h-6 text-green-400 shrink-0" />
    {:else}
      <ShieldAlert class="w-6 h-6 text-amber-400 shrink-0" />
    {/if}
    <div class="min-w-0">
      <h3 class="text-white font-medium">{$t('settings.parental.system.title')}</h3>
      <p class="text-white/60 text-sm mt-1">{$t('settings.parental.system.why')}</p>
    </div>
  </div>

  {#if status}
    <div class="text-sm flex flex-wrap gap-x-5 gap-y-1">
      <span class="text-white/70"><Server class="w-3.5 h-3.5 inline -mt-0.5" />
        {status.enforced && status.serviceActive ? $t('settings.parental.system.active') : status.enforced ? $t('settings.parental.system.inactive') : $t('settings.parental.system.off')}
      </span>
      {#if status.enforced}<span class="text-white/50">{$t('settings.parental.system.rules').replace('{n}', String(status.ruleCount))}</span>{/if}
    </div>

    {#if !status.supported}
      <p class="text-amber-300 text-sm">{$t('settings.parental.system.not_installed')}</p>
    {/if}
  {/if}

  <div class="space-y-2">
    <label class="block text-white/80 text-sm" for="parental-total">{$t('settings.parental.system.total_limit')}</label>
    <div class="flex items-center gap-3">
      <input id="parental-total" type="number" min="0" max="1440" placeholder="—" bind:value={limitInput}
        class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-white w-28" />
      <span class="text-white/50 text-sm">{$t('settings.parental.system.minutes_per_day')}</span>
      <button on:click={saveLimit} class="bg-slate-700 hover:bg-slate-600 text-white rounded-lg px-3 py-2 text-sm">{$t('settings.common.update')}</button>
    </div>
  </div>

  <div class="flex flex-wrap items-center gap-3">
    <button on:click={enable} disabled={busy || (status ? !status.supported : true)}
      class="bg-blue-600 hover:bg-blue-500 disabled:opacity-40 text-white rounded-lg px-4 py-2 text-sm flex items-center gap-2">
      {#if busy}<Loader2 class="w-4 h-4 animate-spin" />{/if}
      {status?.enforced ? $t('settings.parental.system.resync') : $t('settings.parental.system.enable')}
    </button>
    {#if status?.enforced}
      <button on:click={disable} disabled={busy} class="bg-slate-700 hover:bg-red-600/80 text-white rounded-lg px-4 py-2 text-sm">{$t('settings.parental.system.disable')}</button>
    {/if}
  </div>

  {#if error}<p class="text-red-400 text-sm break-words" role="alert">{error}</p>{/if}
  {#if info}<p class="text-green-400 text-sm" role="status">{info}</p>{/if}

  <ul class="text-xs text-white/50 space-y-1 list-disc pl-4">
    <li>{$t('settings.parental.system.covers')}</li>
    <li>{$t('settings.parental.system.limits_internal')}</li>
    <li>{$t('settings.parental.system.needs_standard')}</li>
  </ul>
</div>
