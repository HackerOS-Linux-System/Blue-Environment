<script lang="ts">
  import { Package, Loader2 } from 'lucide-svelte';
  import type { PackageInfo, SoftwareTab } from './types';
  import SourceBadge from './SourceBadge.svelte';
  import { createEventDispatcher } from 'svelte';

  export let pkg: PackageInfo;
  export let tab: SoftwareTab;
  export let busy: boolean;

  const dispatch = createEventDispatcher<{ action: 'install' | 'remove' | 'update' }>();

  function imgError(e: Event) { (e.currentTarget as HTMLElement).style.display = 'none'; }
</script>

<div class="flex items-center gap-3 p-3 bg-slate-800/40 hover:bg-slate-800/70 rounded-xl border border-white/5 transition-colors">
  <div class="w-9 h-9 bg-slate-700 rounded-lg flex items-center justify-center shrink-0">
    {#if pkg.icon}
      <img src={pkg.icon} alt="" class="w-7 h-7 object-contain" on:error={imgError} />
    {:else}
      <Package size={18} class="text-slate-400" />
    {/if}
  </div>
  <div class="flex-1 min-w-0">
    <div class="flex items-center gap-2 flex-wrap">
      <span class="font-medium text-white text-sm">{pkg.name}</span>
      <SourceBadge source={pkg.source} />
      <span class="text-[10px] text-slate-500">{pkg.version}</span>
      {#if pkg.update_available}<span class="text-[10px] text-orange-400">● update</span>{/if}
    </div>
    <div class="text-xs text-slate-500 truncate">{pkg.description}</div>
  </div>
  <div class="shrink-0">
    {#if tab === 'available'}
      <button on:click={() => dispatch('action', 'install')} disabled={busy} class="px-3 py-1 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs disabled:opacity-50 flex items-center gap-1">
        {#if busy}<Loader2 size={11} class="animate-spin" />{/if} Install
      </button>
    {:else if tab === 'installed'}
      <button on:click={() => dispatch('action', 'remove')} disabled={busy} class="px-3 py-1 bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg text-xs disabled:opacity-50">
        Remove
      </button>
    {:else if tab === 'updates'}
      <button on:click={() => dispatch('action', 'update')} disabled={busy} class="px-3 py-1 bg-green-600 hover:bg-green-500 rounded-lg text-xs disabled:opacity-50 flex items-center gap-1">
        {#if busy}<Loader2 size={11} class="animate-spin" />{/if} Update
      </button>
    {/if}
  </div>
</div>
