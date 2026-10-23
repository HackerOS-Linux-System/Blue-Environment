<script lang="ts">
  import { run } from 'svelte/legacy';

  import { t } from '../../../stores/language';
  import { ArrowLeft, ArrowRight, RefreshCw, Home, Search, Lock, Unlock, Star, StarOff, Bookmark, History, Download, X, ListPlus, Settings } from 'lucide-svelte';
  import { createEventDispatcher, } from 'svelte';

  interface Props {
    url: string;
    isNew: boolean;
    isSecure: boolean;
    isBookmarked: boolean;
    canGoBack: boolean;
    canGoForward: boolean;
    panel: 'bookmarks' | 'history' | 'downloads' | 'none';
    downloadCount?: number;
  }

  let {
    url,
    isNew,
    isSecure,
    isBookmarked,
    canGoBack,
    canGoForward,
    panel,
    downloadCount = 0
  }: Props = $props();

  const dispatch = createEventDispatcher<{
    back: void; forward: void; refresh: void; home: void; navigate: string;
    toggleBookmark: void; toggleBookmarks: void; toggleHistory: void; toggleDownloads: void; find: void;
    saveToTasks: void; openSettings: void;
  }>();

  let inputEl = $state() as HTMLInputElement;
  let input = $state(isNew ? '' : url);
  let prevUrl = $state(url);

  run(() => {
    if (url !== prevUrl) { prevUrl = url; input = isNew ? '' : url; }
  });

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter') dispatch('navigate', input);
    if (e.key === 'Escape') { input = isNew ? '' : url; inputEl?.blur(); }
  }
</script>

<div class="flex items-center gap-1.5 px-2 py-1.5 bg-slate-900 border-b border-white/5 shrink-0">
  <button onclick={() => dispatch('back')} disabled={!canGoBack} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><ArrowLeft size={15} /></button>
  <button onclick={() => dispatch('forward')} disabled={!canGoForward} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><ArrowRight size={15} /></button>
  <button onclick={() => dispatch('refresh')} disabled={isNew} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><RefreshCw size={14} /></button>
  <button onclick={() => dispatch('home')} class="p-1.5 rounded-lg hover:bg-white/10"><Home size={14} /></button>
  <div class="flex-1 flex items-center gap-1.5 bg-slate-800 border border-white/10 rounded-xl px-3 py-1.5 focus-within:border-blue-500/40">
    {#if isNew}<Search size={13} class="text-slate-500 shrink-0" />
    {:else if isSecure}<Lock size={13} class="text-green-400 shrink-0" />
    {:else}<Unlock size={13} class="text-amber-400 shrink-0" />{/if}
    <input bind:this={inputEl} bind:value={input} onfocus={(e) => e.currentTarget.select()} onkeydown={handleKeyDown}
      placeholder={$t('blueweb.address_placeholder')} class="flex-1 bg-transparent text-sm text-white placeholder:text-slate-500 focus:outline-none min-w-0" />
    {#if input}<button onclick={() => { input = ''; inputEl?.focus(); }}><X size={12} class="text-slate-500 hover:text-white" /></button>{/if}
  </div>
  {#if !isNew}
    <button onclick={() => dispatch('find')} title="{$t('blueweb.find_in_page')} (Ctrl+F)" class="p-1.5 rounded-lg hover:bg-white/10">
      <Search size={14} />
    </button>
  {/if}
  <button onclick={() => dispatch('toggleBookmark')} disabled={isNew} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30">
    {#if isBookmarked}<Star size={15} class="text-yellow-400 fill-yellow-400" />{:else}<StarOff size={15} class="text-slate-400" />{/if}
  </button>
  <button onclick={() => dispatch('toggleBookmarks')} class="p-1.5 rounded-lg hover:bg-white/10 {panel === 'bookmarks' ? 'text-blue-400' : ''}"><Bookmark size={15} /></button>
  <button onclick={() => dispatch('toggleHistory')} class="p-1.5 rounded-lg hover:bg-white/10 {panel === 'history' ? 'text-blue-400' : ''}"><History size={15} /></button>
  <button onclick={() => dispatch('toggleDownloads')} class="relative p-1.5 rounded-lg hover:bg-white/10 {panel === 'downloads' ? 'text-blue-400' : ''}">
    <Download size={15} />
    {#if downloadCount > 0}
      <span class="absolute -top-0.5 -right-0.5 w-3.5 h-3.5 rounded-full bg-blue-500 text-[8px] flex items-center justify-center font-bold">{downloadCount}</span>
    {/if}
  </button>
  {#if !isNew}
    <button onclick={() => dispatch('saveToTasks')} title={$t('blueweb.save_to_tasks')} class="p-1.5 rounded-lg hover:bg-white/10"><ListPlus size={15} /></button>
  {/if}
  <button onclick={() => dispatch('openSettings')} title={$t('blueweb.settings')} class="p-1.5 rounded-lg hover:bg-white/10"><Settings size={15} /></button>
</div>
