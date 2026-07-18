<script lang="ts">
  import { Package, ArrowUpCircle, Download, Trash2, RefreshCw, Loader2, HardDrive } from 'lucide-svelte';
  import type { PackageInfo, SoftwareTab } from './types';
  import SourceBadge from './SourceBadge.svelte';
  import { createEventDispatcher } from 'svelte';

  export let pkg: PackageInfo;
  export let tab: SoftwareTab;
  export let busy: boolean;

  const dispatch = createEventDispatcher<{ action: 'install' | 'remove' | 'update' }>();

  function imgError(e: Event) { (e.currentTarget as HTMLElement).style.display = 'none'; }
</script>

<div class="bg-slate-800/60 rounded-xl p-4 border border-white/5 hover:border-white/10 transition-all flex flex-col gap-3">
  <div class="flex items-start gap-3">
    <div class="w-11 h-11 bg-slate-700 rounded-xl flex items-center justify-center shrink-0">
      {#if pkg.icon}
        <img src={pkg.icon} alt="" class="w-8 h-8 object-contain" on:error={imgError} />
      {:else}
        <Package size={22} class="text-slate-400" />
      {/if}
    </div>
    <div class="flex-1 min-w-0">
      <h3 class="font-semibold text-white text-sm truncate">{pkg.name}</h3>
      <div class="flex items-center gap-1.5 mt-0.5">
        {#if pkg.source === 'dnf'}<HardDrive size={12} class="text-blue-400" />
        {:else if pkg.source === 'flatpak'}<Package size={12} class="text-green-400" />
        {:else}<Download size={12} class="text-purple-400" />{/if}
        <SourceBadge source={pkg.source} />
        <span class="text-[10px] text-slate-500">{pkg.version}</span>
      </div>
    </div>
    {#if pkg.update_available}<ArrowUpCircle size={16} class="text-orange-400 shrink-0" />{/if}
  </div>

  <p class="text-xs text-slate-400 line-clamp-2 flex-1">{pkg.description}</p>

  <div class="flex items-center justify-between">
    {#if pkg.size}<span class="text-[10px] text-slate-600">{pkg.size}</span>{/if}
    <div class="ml-auto">
      {#if tab === 'available'}
        <button on:click={() => dispatch('action', 'install')} disabled={busy} class="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs disabled:opacity-50 transition-colors">
          {#if busy}<Loader2 size={12} class="animate-spin" />{:else}<Download size={12} />{/if} Install
        </button>
      {:else if tab === 'installed'}
        <button on:click={() => dispatch('action', 'remove')} disabled={busy} class="flex items-center gap-1 px-3 py-1.5 bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg text-xs disabled:opacity-50 transition-colors">
          {#if busy}<Loader2 size={12} class="animate-spin" />{:else}<Trash2 size={12} />{/if} Remove
        </button>
      {:else if tab === 'updates'}
        <button on:click={() => dispatch('action', 'update')} disabled={busy} class="flex items-center gap-1 px-3 py-1.5 bg-green-600 hover:bg-green-500 rounded-lg text-xs disabled:opacity-50 transition-colors">
          {#if busy}<Loader2 size={12} class="animate-spin" />{:else}<RefreshCw size={12} />{/if} Update
        </button>
      {/if}
    </div>
  </div>
</div>
