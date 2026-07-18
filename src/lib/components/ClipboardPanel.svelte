<script lang="ts">
  import { Clipboard, Copy, Trash2, X, Clock } from 'lucide-svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';

  interface ClipboardItem { id: string; content: string; timestamp: number; }

  const dispatch = createEventDispatcher<{ close: void }>();

  let history: ClipboardItem[] = [];
  let currentContent = '';
  let interval: ReturnType<typeof setInterval>;

  async function loadHistory() {
    history = await SystemBridge.getClipboardHistory();
  }

  async function checkClipboard() {
    const text = await SystemBridge.readText();
    if (text && text !== currentContent) {
      currentContent = text;
      await SystemBridge.addToClipboardHistory(text);
      loadHistory();
    }
  }

  onMount(() => {
    loadHistory();
    interval = setInterval(checkClipboard, 1000);
  });
  onDestroy(() => clearInterval(interval));

  async function handleCopy(content: string) { await SystemBridge.copyText(content); }
  async function handleClear() { await SystemBridge.clearClipboardHistory(); loadHistory(); }
  function formatTime(timestamp: number) { return new Date(timestamp).toLocaleTimeString(); }
</script>

<div class="absolute top-14 right-4 w-96 bg-slate-900/98 border border-white/10 rounded-2xl shadow-2xl p-4 z-50 backdrop-blur-xl">
  <div class="flex items-center justify-between mb-3">
    <h3 class="font-semibold text-white flex items-center gap-2"><Clipboard size={16} /> Clipboard History</h3>
    <button on:click={() => dispatch('close')} class="text-slate-400 hover:text-white p-1 rounded-full hover:bg-white/10 transition-colors">
      <X size={16} />
    </button>
  </div>

  <div class="max-h-96 overflow-y-auto space-y-2">
    {#each history as item (item.id)}
      <div class="bg-slate-800 rounded-xl p-3 group">
        <div class="flex items-center justify-between text-xs text-slate-500 mb-1">
          <span class="flex items-center gap-1"><Clock size={10} /> {formatTime(item.timestamp)}</span>
          <button on:click={() => handleCopy(item.content)} class="opacity-0 group-hover:opacity-100 p-1 hover:bg-white/10 rounded transition-all" title="Copy">
            <Copy size={12} />
          </button>
        </div>
        <pre class="text-xs text-slate-200 whitespace-pre-wrap break-words font-sans line-clamp-3">{item.content}</pre>
      </div>
    {/each}
    {#if history.length === 0}
      <div class="text-center text-slate-500 py-8">Clipboard is empty</div>
    {/if}
  </div>

  {#if history.length > 0}
    <button on:click={handleClear} class="mt-3 w-full flex items-center justify-center gap-2 bg-red-600/20 hover:bg-red-500/40 text-red-300 px-3 py-2 rounded-lg text-sm font-medium transition-colors">
      <Trash2 size={14} /> Clear history
    </button>
  {/if}
</div>
