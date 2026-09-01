<script lang="ts">
  import { Globe, X, ChevronRight, FolderOpen, Trash2, CheckCircle2, AlertCircle, Loader2 } from 'lucide-svelte';
  import type { BookmarkItem, HistoryEntry, DownloadItem } from './types';
  import { createEventDispatcher } from 'svelte';

  export let panel: 'bookmarks' | 'history' | 'downloads' | 'none';
  export let bookmarks: BookmarkItem[];
  export let history: HistoryEntry[];
  export let downloads: DownloadItem[] = [];

  const dispatch = createEventDispatcher<{
    close: void; navigate: string; clearHistory: void;
    removeDownload: string; revealDownload: string;
  }>();

  $: empty = panel === 'bookmarks' ? 'No bookmarks yet' : panel === 'history' ? 'No history' : 'No downloads yet';
  $: itemCount = panel === 'bookmarks' ? bookmarks.length : panel === 'history' ? history.length : downloads.length;
</script>

{#if panel !== 'none'}
  <div class="absolute right-0 top-0 bottom-0 w-72 bg-slate-900 border-l border-white/5 flex flex-col z-10">
    <div class="flex items-center justify-between px-4 py-3 border-b border-white/5 shrink-0">
      <span class="font-medium text-sm capitalize">{panel}</span>
      <div class="flex items-center gap-2">
        {#if panel === 'history' && history.length > 0}
          <button on:click={() => dispatch('clearHistory')} class="text-xs text-red-400 hover:text-red-300">Clear</button>
        {/if}
        <button on:click={() => dispatch('close')}><X size={14} class="text-slate-400" /></button>
      </div>
    </div>
    <div class="flex-1 overflow-y-auto">
      {#if itemCount === 0}
        <p class="text-slate-500 text-xs text-center py-8">{empty}</p>
      {/if}
      {#if panel === 'bookmarks'}
        {#each bookmarks as b, i (i)}
          <button on:click={() => { dispatch('navigate', b.url); dispatch('close'); }} class="w-full flex items-center gap-2.5 px-4 py-2.5 hover:bg-white/5 text-left transition-colors">
            {#if b.favicon}<img src={b.favicon} alt="" class="w-3.5 h-3.5 shrink-0 rounded-sm" />{:else}<Globe size={13} class="text-slate-400 shrink-0" />{/if}
            <div class="flex-1 min-w-0">
              <div class="text-sm text-white truncate">{b.title}</div>
              <div class="text-[10px] text-slate-500 truncate">{b.url}</div>
            </div>
            <ChevronRight size={12} class="text-slate-600 shrink-0" />
          </button>
        {/each}
      {:else if panel === 'history'}
        {#each history as h, i (i)}
          <button on:click={() => { dispatch('navigate', h.url); dispatch('close'); }} class="w-full flex items-center gap-2.5 px-4 py-2.5 hover:bg-white/5 text-left transition-colors">
            {#if h.favicon}<img src={h.favicon} alt="" class="w-3.5 h-3.5 shrink-0 rounded-sm" />{:else}<Globe size={13} class="text-slate-400 shrink-0" />{/if}
            <div class="flex-1 min-w-0">
              <div class="text-sm text-white truncate">{h.title}</div>
              <div class="text-[10px] text-slate-500">{new Date(h.time).toLocaleString()}</div>
            </div>
          </button>
        {/each}
      {:else if panel === 'downloads'}
        {#each downloads as d (d.id)}
          <div class="flex items-center gap-2.5 px-4 py-2.5 hover:bg-white/5 transition-colors group">
            {#if d.state === 'downloading'}<Loader2 size={14} class="text-blue-400 shrink-0 animate-spin" />
            {:else if d.state === 'done'}<CheckCircle2 size={14} class="text-green-400 shrink-0" />
            {:else}<AlertCircle size={14} class="text-red-400 shrink-0" />{/if}
            <div class="flex-1 min-w-0">
              <div class="text-sm text-white truncate">{d.filename}</div>
              <div class="text-[10px] text-slate-500 truncate capitalize">{d.state}</div>
            </div>
            {#if d.state !== 'downloading'}
              <button on:click={() => dispatch('revealDownload', d.id)} title="Show in folder" class="opacity-0 group-hover:opacity-100 p-1 hover:text-blue-400"><FolderOpen size={13} /></button>
              <button on:click={() => dispatch('removeDownload', d.id)} title="Remove from list" class="opacity-0 group-hover:opacity-100 p-1 hover:text-red-400"><Trash2 size={13} /></button>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}
