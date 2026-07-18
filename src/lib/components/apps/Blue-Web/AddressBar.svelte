<script lang="ts">
  import { ArrowLeft, ArrowRight, RefreshCw, Home, Search, Lock, Unlock, Star, StarOff, Bookmark, History, X } from 'lucide-svelte';
  import { createEventDispatcher, afterUpdate } from 'svelte';

  export let url: string;
  export let isNew: boolean;
  export let isSecure: boolean;
  export let isBookmarked: boolean;
  export let canGoBack: boolean;
  export let canGoForward: boolean;
  export let panelOpen: 'bookmarks' | 'history' | 'none';

  const dispatch = createEventDispatcher<{
    back: void; forward: void; refresh: void; home: void; navigate: string;
    toggleBookmark: void; toggleBookmarks: void; toggleHistory: void;
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
  <button on:click={() => dispatch('toggleBookmark')} disabled={isNew} class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30">
    {#if isBookmarked}<Star size={15} class="text-yellow-400 fill-yellow-400" />{:else}<StarOff size={15} class="text-slate-400" />{/if}
  </button>
  <button on:click={() => dispatch('toggleBookmarks')} class="p-1.5 rounded-lg hover:bg-white/10 {panelOpen === 'bookmarks' ? 'text-blue-400' : ''}"><Bookmark size={15} /></button>
  <button on:click={() => dispatch('toggleHistory')} class="p-1.5 rounded-lg hover:bg-white/10 {panelOpen === 'history' ? 'text-blue-400' : ''}"><History size={15} /></button>
</div>
