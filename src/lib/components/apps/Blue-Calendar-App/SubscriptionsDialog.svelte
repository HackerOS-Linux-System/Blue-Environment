<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { X, RefreshCw, Trash2, Plus, Loader2, Globe } from 'lucide-svelte';
  import { t } from '../../../stores/language';
  import { EVENT_COLORS } from './types';
  import type { CalendarStore } from './calendarStore';

  export let store: CalendarStore;
  const { subscriptions, syncing, error } = store;
  const dispatch = createEventDispatcher<{ close: void }>();

  let name = '';
  let url = '';
  let color: string = EVENT_COLORS[1];
  let adding = false;

  async function add() {
    if (!name.trim() || !url.trim() || adding) return;
    adding = true;
    const sub = await store.addSubscription(name.trim(), url.trim(), color);
    adding = false;
    if (sub) { name = ''; url = ''; }
  }

  function when(iso?: string | null): string {
    if (!iso) return $t('cal.subs.never');
    const d = new Date(iso);
    return isNaN(d.getTime()) ? $t('cal.subs.never') : $t('cal.subs.synced', { time: d.toLocaleString() });
  }
</script>

<div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm" on:mousedown={() => dispatch('close')} role="presentation">
  <div class="w-[30rem] max-h-[90vh] flex flex-col bg-slate-800 border border-white/10 rounded-2xl shadow-2xl" on:mousedown|stopPropagation role="dialog" aria-modal="true">
    <div class="flex items-center justify-between px-5 pt-5 pb-3">
      <h3 class="text-sm font-semibold text-white flex items-center gap-2"><Globe size={15} class="text-blue-400" /> {$t('cal.subs.title')}</h3>
      <button on:click={() => dispatch('close')} class="p-1 hover:bg-white/10 rounded text-slate-500"><X size={14} /></button>
    </div>
    <p class="px-5 text-[11px] text-slate-400 leading-relaxed">{$t('cal.subs.desc')}</p>

    <div class="flex-1 overflow-y-auto px-5 py-3 space-y-1.5 min-h-[4rem]">
      {#each $subscriptions as s (s.id)}
        <div class="flex items-center gap-2 p-2 rounded-lg bg-slate-900/60 border border-white/5">
          <input type="checkbox" checked={s.enabled} on:change={(e) => store.setSubscriptionEnabled(s.id, e.currentTarget.checked)} class="rounded" />
          <span class="w-2.5 h-2.5 rounded-full shrink-0" style="background:{s.color}" />
          <div class="min-w-0 flex-1">
            <div class="text-xs text-white truncate">{s.name}</div>
            <div class="text-[10px] text-slate-500 truncate" title={s.url}>{$syncing[s.id] ? $t('cal.subs.syncing') : when(s.lastSynced)}</div>
          </div>
          <button on:click={() => store.syncSubscription(s.id)} disabled={$syncing[s.id]} title={$t('cal.subs.sync')} class="p-1.5 rounded-md hover:bg-white/10 text-slate-400 disabled:opacity-50">
            {#if $syncing[s.id]}<Loader2 size={13} class="animate-spin" />{:else}<RefreshCw size={13} />{/if}
          </button>
          <button on:click={() => store.removeSubscription(s.id)} title={$t('settings.common.delete')} class="p-1.5 rounded-md hover:bg-white/10 text-slate-500 hover:text-red-400"><Trash2 size={13} /></button>
        </div>
      {:else}
        <p class="text-center text-xs text-slate-600 py-4">{$t('cal.subs.none')}</p>
      {/each}
    </div>

    {#if $error}<div class="mx-5 mb-2 px-2.5 py-1.5 rounded-lg bg-red-500/10 text-red-300 text-[11px] break-words">{$error}</div>{/if}

    <div class="px-5 pb-5 pt-3 border-t border-white/5 space-y-2">
      <input bind:value={name} placeholder={$t('cal.subs.name')} class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60" />
      <input bind:value={url} placeholder={$t('cal.subs.url')} on:keydown={(e) => e.key === 'Enter' && add()} class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60" />
      <div class="flex items-center gap-2">
        {#each EVENT_COLORS as c (c)}
          <button type="button" on:click={() => (color = c)} aria-label={c} class="w-5 h-5 rounded-full {color === c ? 'ring-2 ring-white/60 scale-110' : ''}" style="background:{c}" />
        {/each}
        <button on:click={add} disabled={!name.trim() || !url.trim() || adding} class="ml-auto flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs disabled:opacity-40">
          {#if adding}<Loader2 size={12} class="animate-spin" />{:else}<Plus size={12} />{/if} {$t('cal.subs.add')}
        </button>
      </div>
    </div>
  </div>
</div>
