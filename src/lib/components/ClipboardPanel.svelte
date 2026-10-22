<script lang="ts">
  import { Clipboard, Copy, Trash2, X, Clock, Image as ImageIcon, FileText, Files, Search } from 'lucide-svelte';
  import { SystemBridge, type RichClipboardItem } from '../utils/systemBridge';
  import { t } from '../stores/language';
  import { onMount, onDestroy } from 'svelte';

  interface Props { onclose?: () => void; }
  let { onclose }: Props = $props();

  type Filter = 'all' | 'text' | 'image' | 'files';
  interface TextItem { id: string; content: string; timestamp: number; }
  type Entry =
    | { type: 'text'; id: string; timestamp: number; text: string }
    | { type: 'image'; id: string; timestamp: number; rich: RichClipboardItem }
    | { type: 'files'; id: string; timestamp: number; rich: RichClipboardItem };

  let texts = $state<TextItem[]>([]);
  let rich = $state<RichClipboardItem[]>([]);
  let thumbs = $state<Record<string, string>>({});
  let filter = $state<Filter>('all');
  let query = $state('');
  let copiedId = $state<string | null>(null);
  let currentText = '';
  let interval: ReturnType<typeof setInterval>;

  let entries = $derived.by<Entry[]>(() => {
    const all: Entry[] = [
      ...texts.map((x): Entry => ({ type: 'text', id: 't' + x.id, timestamp: x.timestamp, text: x.content })),
      ...rich.map((x): Entry => ({ type: x.kind, id: 'r' + x.id, timestamp: x.timestamp, rich: x }) as Entry),
    ].sort((a, b) => b.timestamp - a.timestamp);
    const q = query.trim().toLowerCase();
    return all.filter((e) => {
      if (filter !== 'all' && e.type !== filter) return false;
      if (!q) return true;
      if (e.type === 'text') return e.text.toLowerCase().includes(q);
      if (e.type === 'files') return e.rich.paths.some((p) => p.toLowerCase().includes(q));
      return 'image'.includes(q) || e.rich.mime.includes(q);
    });
  });

  async function loadHistory() {
    const [t, r] = await Promise.all([SystemBridge.getClipboardHistory(), SystemBridge.getRichClipboardHistory()]);
    texts = t;
    rich = r;
    for (const item of r) {
      if (item.kind === 'image' && !thumbs[item.id]) {
        SystemBridge.getRichClipboardImage(item.id).then((url) => (thumbs = { ...thumbs, [item.id]: url })).catch(() => {});
      }
    }
  }

  async function poll() {
    try {
      const res = await SystemBridge.pollClipboardRich();
      if (res.kind === 'text' && res.text && res.text !== currentText) {
        currentText = res.text;
        await SystemBridge.addToClipboardHistory(res.text);
        await loadHistory();
      } else if (res.item) {
        await loadHistory();
      }
    } catch { /* clipboard tools missing — text-only fallback below */
      const text = await SystemBridge.readText();
      if (text && text !== currentText) { currentText = text; await SystemBridge.addToClipboardHistory(text); await loadHistory(); }
    }
  }

  onMount(() => { loadHistory(); poll(); interval = setInterval(poll, 1000); });
  onDestroy(() => clearInterval(interval));

  async function copyEntry(e: Entry) {
    if (e.type === 'text') await SystemBridge.copyText(e.text);
    else await SystemBridge.copyRichClipboardItem(e.rich.id);
    copiedId = e.id;
    setTimeout(() => { if (copiedId === e.id) copiedId = null; }, 1200);
  }
  async function removeEntry(e: Entry) {
    if (e.type !== 'text') { await SystemBridge.deleteRichClipboardItem(e.rich.id); await loadHistory(); }
  }
  async function clearAll() {
    await Promise.all([SystemBridge.clearClipboardHistory(), SystemBridge.clearRichClipboard()]);
    thumbs = {};
    await loadHistory();
  }

  const formatTime = (ts: number) => new Date(ts).toLocaleTimeString();
  const baseName = (p: string) => p.split('/').filter(Boolean).pop() ?? p;
  function formatSize(n: number) {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }
  const FILTERS: { id: Filter; key: string; icon: typeof FileText }[] = [
    { id: 'all', key: 'clipboard.filter_all', icon: Clipboard },
    { id: 'text', key: 'clipboard.filter_text', icon: FileText },
    { id: 'image', key: 'clipboard.filter_images', icon: ImageIcon },
    { id: 'files', key: 'clipboard.filter_files', icon: Files },
  ];
</script>

<div data-shell-occluder="clipboard" class="absolute top-14 right-4 w-96 bg-slate-900/98 border border-white/10 rounded-2xl shadow-2xl p-4 z-50 backdrop-blur-xl">
  <div class="flex items-center justify-between mb-3">
    <h3 class="font-semibold text-white flex items-center gap-2"><Clipboard size={16} /> {$t('clipboard.title')}</h3>
    <button onclick={() => onclose?.()} aria-label={$t('clipboard.close')} class="text-slate-400 hover:text-white p-1 rounded-full hover:bg-white/10 transition-colors">
      <X size={16} />
    </button>
  </div>

  <div class="relative mb-2">
    <Search size={13} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
    <input bind:value={query} placeholder={$t('clipboard.search')} class="w-full bg-slate-800 border border-white/10 rounded-lg pl-8 pr-2 py-1.5 text-xs text-white placeholder:text-slate-500" />
  </div>

  <div class="flex gap-1 mb-3">
    {#each FILTERS as f (f.id)}
      <button onclick={() => (filter = f.id)}
        class="flex-1 flex items-center justify-center gap-1 py-1 rounded-lg text-xs transition-colors {filter === f.id ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400 hover:text-white'}">
        <f.icon size={12} /> {$t(f.key)}
      </button>
    {/each}
  </div>

  <div class="max-h-96 overflow-y-auto space-y-2">
    {#each entries as e (e.id)}
      <div class="bg-slate-800 rounded-xl p-3 group">
        <div class="flex items-center justify-between text-xs text-slate-500 mb-1">
          <span class="flex items-center gap-1"><Clock size={10} /> {formatTime(e.timestamp)}
            {#if e.type !== 'text'}<span class="text-slate-600">· {formatSize(e.rich.size)}</span>{/if}
          </span>
          <span class="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-all">
            <button onclick={() => copyEntry(e)} class="p-1 hover:bg-white/10 rounded" title={$t('clipboard.copy')} aria-label={$t('clipboard.copy')}>
              {#if copiedId === e.id}<span class="text-emerald-400 text-[10px] font-medium">OK</span>{:else}<Copy size={12} />{/if}
            </button>
            {#if e.type !== 'text'}
              <button onclick={() => removeEntry(e)} class="p-1 hover:bg-red-500/20 rounded text-slate-400 hover:text-red-300" title={$t('clipboard.delete')} aria-label={$t('clipboard.delete')}>
                <Trash2 size={12} />
              </button>
            {/if}
          </span>
        </div>
        {#if e.type === 'text'}
          <pre class="text-xs text-slate-200 whitespace-pre-wrap break-words font-sans line-clamp-3">{e.text}</pre>
        {:else if e.type === 'image'}
          {#if thumbs[e.rich.id]}
            <img src={thumbs[e.rich.id]} alt={$t('clipboard.image_alt')} class="max-h-40 w-full object-contain rounded-lg bg-black/30" />
          {:else}
            <div class="h-16 flex items-center justify-center text-slate-600"><ImageIcon size={20} /></div>
          {/if}
        {:else}
          <ul class="text-xs text-slate-200 space-y-0.5">
            {#each e.rich.paths.slice(0, 4) as p (p)}
              <li class="flex items-center gap-1.5 truncate" title={p}><FileText size={11} class="shrink-0 text-slate-500" /> {baseName(p)}</li>
            {/each}
            {#if e.rich.paths.length > 4}
              <li class="text-slate-500">{$t('clipboard.more_files', { n: e.rich.paths.length - 4 })}</li>
            {/if}
          </ul>
        {/if}
      </div>
    {/each}
    {#if entries.length === 0}
      <div class="text-center text-slate-500 py-8">{$t('clipboard.empty')}</div>
    {/if}
  </div>

  {#if texts.length + rich.length > 0}
    <button onclick={clearAll} class="mt-3 w-full flex items-center justify-center gap-2 bg-red-600/20 hover:bg-red-500/40 text-red-300 px-3 py-2 rounded-lg text-sm font-medium transition-colors">
      <Trash2 size={14} /> {$t('clipboard.clear')}
    </button>
  {/if}
</div>
