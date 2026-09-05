<script lang="ts">
  // Blue Downloader — a real download manager backed by
  // src-tauri/src/BlueDownloaderApp/mod.rs (streaming HTTP downloads,
  // live progress events, pause/resume via Range requests when the
  // server supports it, persisted history). See that module's doc for
  // exactly what's real vs. simplified (no concurrency limit, no
  // checksum verification, in-flight downloads don't auto-resume after
  // an app restart).
  import { onMount, onDestroy } from 'svelte';
  import { Download, Plus, Pause, Play, X, Trash2, FolderOpen, CheckCircle2, AlertCircle, Loader2 } from 'lucide-svelte';
  import { createDownloaderStore } from './downloaderStore.svelte';
  import { formatBytes, formatSpeed, progressFraction, type DownloadItem } from './types';
  import { SystemBridge } from '../../../utils/systemBridge';

  export let windowId: string;

  const store = createDownloaderStore();
  const { downloads, error } = store;

  let urlInput = '';
  let adding = false;

  async function submitAdd() {
    if (!urlInput.trim()) return;
    adding = true;
    try {
      await store.add(urlInput.trim());
      urlInput = '';
    } finally {
      adding = false;
    }
  }

  function statusLabel(item: DownloadItem): string {
    switch (item.status.state) {
      case 'queued': return 'Queued';
      case 'downloading': return 'Downloading';
      case 'paused': return item.status.resumable ? 'Paused' : 'Paused (can\'t resume — server doesn\'t support ranges)';
      case 'completed': return 'Completed';
      case 'failed': return `Failed: ${item.status.error}`;
      case 'cancelled': return 'Cancelled';
    }
  }

  function openContainingFolder(item: DownloadItem) {
    const dir = item.destinationPath.substring(0, item.destinationPath.lastIndexOf('/')) || '/';
    // No dedicated "reveal in file manager" bridge method exists yet —
    // `xdg-open` on the containing directory is the same fallback this
    // codebase's other apps use for "open externally" actions on Linux.
    SystemBridge.executeCommand(`xdg-open '${dir.replace(/'/g, "'\\''")}'`);
  }

  onMount(() => {
    store.load();
    store.subscribe();
  });
  onDestroy(() => store.unsubscribe());
</script>

<div class="flex flex-col h-full bg-slate-950 text-slate-100 text-sm">
  <div class="flex items-center gap-2 px-4 py-3 border-b border-white/10 shrink-0">
    <Download size={16} class="text-blue-400" />
    <span class="font-medium">Blue Downloader</span>
  </div>

  <div class="px-4 py-3 border-b border-white/5 flex items-center gap-2 shrink-0">
    <input
      bind:value={urlInput}
      on:keydown={(e) => e.key === 'Enter' && submitAdd()}
      placeholder="Paste a URL to download…"
      class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm outline-none focus:border-blue-500 placeholder:text-slate-500"
    />
    <button
      on:click={submitAdd}
      disabled={!urlInput.trim() || adding}
      class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-40 text-sm font-medium transition-colors shrink-0"
    >
      {#if adding}<Loader2 size={14} class="animate-spin" />{:else}<Plus size={14} />{/if}
      Download
    </button>
  </div>

  <div class="flex-1 overflow-y-auto p-3 space-y-2">
    {#if $downloads.length === 0}
      <div class="flex flex-col items-center justify-center gap-2 text-slate-600 pt-16">
        <Download size={28} class="opacity-40" />
        <p class="text-xs">No downloads yet — paste a URL above to start one.</p>
      </div>
    {:else}
      {#each $downloads as item (item.id)}
        {@const fraction = progressFraction(item)}
        <div class="rounded-lg border border-white/10 bg-slate-900/60 p-3">
          <div class="flex items-center gap-2 mb-1.5">
            {#if item.status.state === 'completed'}
              <CheckCircle2 size={14} class="text-emerald-400 shrink-0" />
            {:else if item.status.state === 'failed'}
              <AlertCircle size={14} class="text-red-400 shrink-0" />
            {:else if item.status.state === 'downloading'}
              <Loader2 size={14} class="text-blue-400 animate-spin shrink-0" />
            {:else}
              <Download size={14} class="text-slate-500 shrink-0" />
            {/if}
            <span class="text-sm font-medium truncate flex-1" title={item.filename}>{item.filename}</span>
          </div>

          <div class="w-full h-1.5 bg-slate-800 rounded-full overflow-hidden mb-1.5">
            <div
              class="h-full transition-all {item.status.state === 'failed' ? 'bg-red-500' : item.status.state === 'completed' ? 'bg-emerald-500' : 'bg-blue-500'}"
              style="width: {fraction !== null ? fraction * 100 : item.status.state === 'downloading' ? 100 : 0}%; {fraction === null && item.status.state === 'downloading' ? 'animation: pulse 1.5s ease-in-out infinite;' : ''}"
            />
          </div>

          <div class="flex items-center justify-between text-[11px] text-slate-500">
            <span>
              {statusLabel(item)}
              {#if item.totalBytes !== null}
                · {formatBytes(item.downloadedBytes)} / {formatBytes(item.totalBytes)}
              {:else if item.downloadedBytes > 0}
                · {formatBytes(item.downloadedBytes)}
              {/if}
              {#if item.status.state === 'downloading' && item.speedBytesPerSec}
                · {formatSpeed(item.speedBytesPerSec)}
              {/if}
            </span>
            <div class="flex items-center gap-1">
              {#if item.status.state === 'downloading' || item.status.state === 'queued'}
                <button on:click={() => store.pause(item.id)} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors" title="Pause">
                  <Pause size={12} />
                </button>
              {:else if item.status.state === 'paused' || item.status.state === 'failed'}
                <button on:click={() => store.resume(item.id)} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors" title={item.status.state === 'paused' && !item.status.resumable ? 'Restart from the beginning' : 'Resume'}>
                  <Play size={12} />
                </button>
              {/if}
              {#if item.status.state === 'completed'}
                <button on:click={() => openContainingFolder(item)} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors" title="Show in folder">
                  <FolderOpen size={12} />
                </button>
              {/if}
              {#if item.status.state === 'downloading' || item.status.state === 'queued' || item.status.state === 'paused'}
                <button on:click={() => store.cancel(item.id)} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-red-400 transition-colors" title="Cancel">
                  <X size={12} />
                </button>
              {:else}
                <button on:click={() => store.remove(item.id)} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-red-400 transition-colors" title="Remove from list">
                  <Trash2 size={12} />
                </button>
              {/if}
            </div>
          </div>
        </div>
      {/each}
    {/if}
  </div>

  {#if $error}
    <div class="absolute bottom-3 right-3 bg-red-500/90 text-white text-xs px-3 py-2 rounded shadow-lg">{$error}</div>
  {/if}
</div>
