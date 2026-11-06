<script lang="ts">
  import { onMount } from 'svelte';
  import { X, ShieldCheck, Timer, PlugZap } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { t } from '../../../stores/language';
  import type { VaultEntry } from './types';

  /** Password health, auto-lock and the Secret Service bridge (see BlueAccounts/{audit,idle,secret_service}.rs). */
  interface Props { entries: VaultEntry[]; onclose: () => void }
  let { entries, onclose }: Props = $props();

  interface Audit { total: number; audited: number; weak: string[]; reused: string[][]; empty: string[]; score: number }
  interface AccessRecord { time: string; client: string; action: string; item: string }

  let audit = $state<Audit | null>(null);
  let autoLock = $state(15);
  let service = $state<{ running: boolean; busName: string } | null>(null);
  let log = $state<AccessRecord[]>([]);
  let error = $state('');

  const titleOf = (id: string) => entries.find((e) => e.id === id)?.title || '(untitled)';
  const choices = [0, 5, 15, 30, 60, 240];

  async function refresh() {
    try {
      audit = await SystemBridge.invoke<Audit>('accounts_audit');
      autoLock = (await SystemBridge.invoke<{ autoLockMinutes: number }>('accounts_get_settings')).autoLockMinutes;
      service = await SystemBridge.invoke('secrets_service_status');
      log = await SystemBridge.invoke<AccessRecord[]>('secrets_access_log');
    } catch (e) { error = String(e); }
  }
  async function setAutoLock(m: number) {
    autoLock = m;
    try { await SystemBridge.invoke('accounts_set_settings', { settings: { autoLockMinutes: m } }); } catch (e) { error = String(e); }
  }
  onMount(refresh);

  const scoreColor = (s: number) => (s >= 80 ? 'text-emerald-400' : s >= 50 ? 'text-amber-400' : 'text-red-400');
  const time = (iso: string) => { try { return new Date(iso).toLocaleTimeString(); } catch { return iso; } };
</script>

<div class="absolute inset-0 z-20 flex flex-col overflow-y-auto bg-slate-950 p-5" role="dialog" aria-label={$t('accounts.security.title')}>
  <div class="mb-4 flex items-center justify-between">
    <h2 class="flex items-center gap-2 text-base font-semibold"><ShieldCheck class="h-5 w-5 text-blue-400" /> {$t('accounts.security.title')}</h2>
    <button onclick={onclose} class="flex h-7 w-7 items-center justify-center rounded hover:bg-white/10" aria-label={$t('accounts.security.close')}><X class="h-4 w-4" /></button>
  </div>
  {#if error}<p class="mb-3 text-xs text-red-300">{error}</p>{/if}

  <section class="mb-4 rounded-xl border border-white/10 bg-slate-900 p-4">
    <h3 class="mb-2 text-sm font-medium">{$t('accounts.security.health')}</h3>
    {#if audit}
      <div class="mb-3 flex items-baseline gap-2"><span class="text-3xl font-semibold {scoreColor(audit.score)}">{audit.score}</span><span class="text-xs text-slate-400">/ 100 · {$t('accounts.security.audited', { n: audit.audited })}</span></div>
      {#if audit.weak.length + audit.reused.length + audit.empty.length === 0}
        <p class="text-xs text-emerald-300">{$t('accounts.security.all_good')}</p>
      {:else}
        {#if audit.weak.length}<p class="mt-1 text-xs text-amber-300">{$t('accounts.security.weak', { n: audit.weak.length })}: {audit.weak.map(titleOf).join(', ')}</p>{/if}
        {#each audit.reused as group}<p class="mt-1 text-xs text-amber-300">{$t('accounts.security.reused')}: {group.map(titleOf).join(', ')}</p>{/each}
        {#if audit.empty.length}<p class="mt-1 text-xs text-slate-300">{$t('accounts.security.empty', { n: audit.empty.length })}: {audit.empty.map(titleOf).join(', ')}</p>{/if}
      {/if}
    {/if}
  </section>

  <section class="mb-4 rounded-xl border border-white/10 bg-slate-900 p-4">
    <h3 class="mb-2 flex items-center gap-2 text-sm font-medium"><Timer class="h-4 w-4 text-slate-400" /> {$t('accounts.security.auto_lock')}</h3>
    <select class="rounded border border-white/10 bg-slate-800 px-2 py-1.5 text-xs" value={autoLock} onchange={(e) => setAutoLock(Number(e.currentTarget.value))}>
      {#each choices as m}<option value={m}>{m === 0 ? $t('accounts.security.never') : m < 60 ? `${m} min` : `${m / 60} h`}</option>{/each}
    </select>
    <p class="mt-1.5 text-xs text-slate-500">{$t('accounts.security.auto_lock_hint')}</p>
  </section>

  <section class="rounded-xl border border-white/10 bg-slate-900 p-4">
    <h3 class="mb-2 flex items-center gap-2 text-sm font-medium"><PlugZap class="h-4 w-4 text-slate-400" /> {$t('accounts.security.secret_service')}</h3>
    {#if service?.running}
      <p class="text-xs text-emerald-300">{$t('accounts.security.ss_running', { name: service.busName })}</p>
    {:else}
      <p class="text-xs text-amber-300">{$t('accounts.security.ss_off')}</p>
    {/if}
    <p class="mt-1 text-xs text-slate-500">{$t('accounts.security.ss_hint')}</p>
    {#if log.length}
      <table class="mt-3 w-full text-left text-xs">
        <thead class="text-slate-500"><tr><th class="py-1 pr-2 font-normal">{$t('accounts.security.log_time')}</th><th class="pr-2 font-normal">{$t('accounts.security.log_app')}</th><th class="pr-2 font-normal">{$t('accounts.security.log_action')}</th><th class="font-normal">{$t('accounts.security.log_item')}</th></tr></thead>
        <tbody>{#each log.slice(0, 30) as r}<tr class="border-t border-white/5"><td class="py-1 pr-2 text-slate-400">{time(r.time)}</td><td class="pr-2">{r.client}</td><td class="pr-2 text-slate-300">{r.action}</td><td class="truncate text-slate-400">{r.item}</td></tr>{/each}</tbody>
      </table>
    {:else}
      <p class="mt-3 text-xs text-slate-500">{$t('accounts.security.log_empty')}</p>
    {/if}
  </section>
</div>
