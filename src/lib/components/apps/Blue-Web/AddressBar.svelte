<script lang="ts">
  import { ArrowLeft, ArrowRight, RefreshCw, Home, Search, Lock, Unlock, Star, StarOff, Bookmark, History, Download, X, ListPlus, Settings } from 'lucide-svelte';
  import { createEventDispatcher, afterUpdate } from 'svelte';

  export let url: string;
  export let isNew: boolean;
  export let isSecure: boolean;
  export let isBookmarked: boolean;
  export let canGoBack: boolean;
  export let canGoForward: boolean;
  export let panel: 'bookmarks' | 'history' | 'downloads' | 'none';
  export let downloadCount = 0;

  const dispatch = createEventDispatcher<{
    back: void; forward: void; refresh: void; home: void; navigate: string;
    toggleBookmark: void; toggleBookmarks: void; toggleHistory: void; toggleDownloads: void; find: void;
    saveToTasks: void; openSettings: void;
  }>();

  let inputEl: HTMLInputElement;
  let input = isNew ? '' : url;
  let prevUrl = url;

  $: if (url !== prevUrl) { prevUrl = url; input = isNew ? '' : url; }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter') dispatch('navigate', input);
    if (e.key === 'Escape') { input = isNew ? '' : url; inputEl?.blur(); }
  }
</script>

<div class="flex items-center gap-1.5 px-2 py-1.5 bg-slate-900 border-b border-white/5 shrink-0">
  <button on:click={() => dispatch('back')} disabled={!canGoBack} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><ArrowLeft size={15} /></button>
  <button on:click={() => dispatch('forward')} disabled={!canGoForward} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><ArrowRight size={15} /></button>
  <button on:click={() => dispatch('refresh')} disabled={isNew} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><RefreshCw size={14} /></button>
  <button on:click={() => dispatch('home')} class="p-1.5 rounded-lg hover:bg-white/10"><Home size={14} /></button>
  <div class="flex-1 flex items-center gap-1.5 bg-slate-800 border border-white/10 rounded-xl px-3 py-1.5 focus-within:border-blue-500/40">
    {#if isNew}<Search size={13} class="text-slate-500 shrink-0" />
    {:else if isSecure}<Lock size={13} class="text-green-400 shrink-0" />
    {:else}<Unlock size={13} class="text-amber-400 shrink-0" />{/if}
    <input bind:this={inputEl} bind:value={input} on:focus={(e) => e.currentTarget.select()} on:keydown={handleKeyDown}
      placeholder="Search or enter URL…" class="flex-1 bg-transparent text-sm text-white placeholder:text-slate-500 focus:outline-none min-w-0" />
    {#if input}<button on:click={() => { input = ''; inputEl?.focus(); }}><X size={12} class="text-slate-500 hover:text-white" /></button>{/if}
  </div>
  {#if !isNew}
    <button on:click={() => dispatch('find')} title="Find in page (Ctrl+F)" class="p-1.5 rounded-lg hover:bg-white/10">
      <Search size={14} />
    </button>
  {/if}
  <button on:click={() => dispatch('toggleBookmark')} disabled={isNew} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30">
    {#if isBookmarked}<Star size={15} class="text-yellow-400 fill-yellow-400" />{:else}<StarOff size={15} class="text-slate-400" />{/if}
  </button>
  <button on:click={() => dispatch('toggleBookmarks')} class="p-1.5 rounded-lg hover:bg-white/10 {panel === 'bookmarks' ? 'text-blue-400' : ''}"><Bookmark size={15} /></button>
  <button on:click={() => dispatch('toggleHistory')} class="p-1.5 rounded-lg hover:bg-white/10 {panel === 'history' ? 'text-blue-400' : ''}"><History size={15} /></button>
  <button on:click={() => dispatch('toggleDownloads')} class="relative p-1.5 rounded-lg hover:bg-white/10 {panel === 'downloads' ? 'text-blue-400' : ''}">
    <Download size={15} />
    {#if downloadCount > 0}
      <span class="absolute -top-0.5 -right-0.5 w-3.5 h-3.5 rounded-full bg-blue-500 text-[8px] flex items-center justify-center font-bold">{downloadCount}</span>
    {/if}
  </button>
  {#if !isNew}
    <button on:click={() => dispatch('saveToTasks')} title="Save to Blue Tasks" class="p-1.5 rounded-lg hover:bg-white/10"><ListPlus size={15} /></button>
  {/if}
  <button on:click={() => dispatch('openSettings')} title="Blue Web Settings" class="p-1.5 rounded-lg hover:bg-white/10"><Settings size={15} /></button>
</div>
