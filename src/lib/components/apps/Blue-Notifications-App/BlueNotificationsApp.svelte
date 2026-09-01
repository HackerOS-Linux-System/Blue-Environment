<script lang="ts">
  // Blue Notifications — new app. Feed-watcher rules that surface into
  // the shell's existing Notification Center — see notificationsStore.ts
  // and src-tauri/src/BlueNotificationsApp/mod.rs's module docs for the
  // full design (real bus, not a parallel alert system; frontend-driven
  // polling, no OS daemon).
  import { onMount } from 'svelte';
  import { Rss, Plus, Trash2, RefreshCw, Bell, BellOff, X, Check, AlertTriangle } from 'lucide-svelte';
  import { createNotificationsStore } from './notificationsStore';
  import { INTERVAL_OPTIONS } from './types';
  import type { NotificationRule } from './types';
  import { t } from '../../../stores/language';

  export let windowId: string;

  const store = createNotificationsStore();
  const { rules, loading, error, checking } = store;

  let showAdd = false;
  let draftName = '';
  let draftUrl = '';
  let draftInterval = 30;

  onMount(() => {
    store.load();
    store.startPolling();
  });

  function intervalLabel(mins: number): string {
    if (mins < 60) return `${mins} ${$t('notif.minutes')}`;
    if (mins < 1440) return `${Math.round(mins / 60)} ${$t('notif.hours')}`;
    return `${Math.round(mins / 1440)} ${$t('notif.days')}`;
  }

  async function addRule() {
    const name = draftName.trim();
    const url = draftUrl.trim();
    if (!name || !url) return;
    await store.saveRule({ name, url, kind: 'rss', intervalMinutes: draftInterval, enabled: true });
    draftName = ''; draftUrl = ''; draftInterval = 30; showAdd = false;
  }

  async function toggleEnabled(rule: NotificationRule) {
    await store.saveRule({ ...rule, enabled: !rule.enabled });
  }
</script>

<div class="flex flex-col h-full bg-slate-900 text-white text-sm">
  <div class="flex items-center gap-2 px-4 h-11 border-b border-white/5 shrink-0">
    <Bell size={16} class="text-blue-400" />
    <span class="font-semibold">{$t('notif.title')}</span>
    <div class="flex-1" />
    <button on:click={() => (showAdd = !showAdd)} class="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500">
      <Plus size={13} /> {$t('notif.add_rule')}
    </button>
  </div>

  {#if showAdd}
    <div class="p-4 border-b border-white/5 bg-slate-950/40 flex flex-col gap-3 shrink-0">
      <div class="flex items-center gap-2 text-xs text-slate-400"><Rss size={13} /> {$t('notif.rss_watcher')}</div>
      <input bind:value={draftName} placeholder={$t('notif.name_placeholder')} class="bg-slate-800 rounded-lg px-3 py-2 focus:outline-none" />
      <input bind:value={draftUrl} placeholder={$t('notif.url_placeholder')} class="bg-slate-800 rounded-lg px-3 py-2 focus:outline-none font-mono text-xs" />
      <div class="flex items-center gap-2">
        <span class="text-xs text-slate-400 shrink-0">{$t('notif.check_every')}</span>
        <select bind:value={draftInterval} class="bg-slate-800 rounded-lg px-3 py-1.5 focus:outline-none flex-1">
          {#each INTERVAL_OPTIONS as m}<option value={m}>{intervalLabel(m)}</option>{/each}
        </select>
      </div>
      <div class="flex justify-end gap-2">
        <button on:click={() => (showAdd = false)} class="text-xs px-3 py-1.5 rounded-lg hover:bg-white/5">{$t('notif.cancel')}</button>
        <button on:click={addRule} disabled={!draftName.trim() || !draftUrl.trim()} class="text-xs px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-40">{$t('notif.add')}</button>
      </div>
    </div>
  {/if}

  {#if $error}
    <div class="flex items-center gap-2 px-4 py-2 bg-red-500/10 text-red-300 text-xs shrink-0">
      <AlertTriangle size={13} /> {$error}
      <button on:click={() => error.set(null)} class="ml-auto"><X size={12} /></button>
    </div>
  {/if}

  <div class="flex-1 overflow-y-auto">
    {#if $loading}
      <div class="flex items-center justify-center h-full text-slate-500 text-xs">{$t('notif.loading')}</div>
    {:else if $rules.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-slate-500 gap-2">
        <Rss size={28} class="opacity-30" />
        <span class="text-xs">{$t('notif.empty')}</span>
      </div>
    {:else}
      {#each $rules as rule (rule.id)}
        <div class="flex items-center gap-3 px-4 py-3 border-b border-white/[0.03] hover:bg-white/5">
          <div class="w-8 h-8 rounded-lg bg-orange-500/15 flex items-center justify-center shrink-0">
            <Rss size={14} class="text-orange-400" />
          </div>
          <div class="flex-1 min-w-0">
            <div class="truncate {rule.enabled ? 'text-white' : 'text-slate-500'}">{rule.name}</div>
            <div class="text-[10px] text-slate-500 truncate font-mono">{rule.url}</div>
            <div class="text-[10px] text-slate-600 mt-0.5">{$t('notif.check_every')} {intervalLabel(rule.intervalMinutes)} · {rule.lastSeenGuids.length} {$t('notif.tracked')}</div>
          </div>
          <button on:click={() => store.checkNow(rule)} title={$t('notif.check_now')}
            disabled={$checking.has(rule.id)} class="p-1.5 rounded-lg hover:bg-white/10 shrink-0">
            <RefreshCw size={14} class={$checking.has(rule.id) ? 'animate-spin text-blue-400' : 'text-slate-400'} />
          </button>
          <button on:click={() => toggleEnabled(rule)} title={rule.enabled ? $t('notif.disable') : $t('notif.enable')} class="p-1.5 rounded-lg hover:bg-white/10 shrink-0">
            {#if rule.enabled}<Bell size={14} class="text-blue-400" />{:else}<BellOff size={14} class="text-slate-500" />{/if}
          </button>
          <button on:click={() => store.deleteRule(rule.id)} class="p-1.5 rounded-lg hover:bg-white/10 hover:text-red-400 text-slate-500 shrink-0"><Trash2 size={14} /></button>
        </div>
      {/each}
    {/if}
  </div>

  <div class="px-4 py-2 border-t border-white/5 text-[10px] text-slate-600 shrink-0">
    {$t('notif.footer_note')}
  </div>
</div>
