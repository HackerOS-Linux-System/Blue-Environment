<script lang="ts">
  import { FolderOpen, Plus, Save, Terminal as TerminalIcon, Search, GitBranch, Command, RefreshCw, Zap } from 'lucide-svelte';
  import type { OpenFile, SidebarTab } from './types';
  import { createEventDispatcher } from 'svelte';

  export let sidebarCollapsed: boolean;
  // Passed by the parent for symmetry with the sidebar; the toolbar doesn't use it.
  $: sidebarCollapsed;
  export let canSave: boolean;
  export let showTerminal: boolean;
  export let sidebarTab: SidebarTab;
  export let activeFile: OpenFile | undefined;
  export let lspStatus: Record<string, boolean>;

  const dispatch = createEventDispatcher<{
    toggleSidebar: void; newFile: void; save: void; toggleTerminal: void;
    setTab: SidebarTab; commandPalette: void; refresh: void;
  }>();
</script>

<div class="h-10 bg-slate-800 border-b border-white/5 flex items-center px-3 gap-1 shrink-0">
  <button on:click={() => dispatch('toggleSidebar')} class="p-1.5 hover:bg-white/10 rounded" title="Toggle sidebar"><FolderOpen size={16} /></button>
  <button on:click={() => dispatch('newFile')} class="p-1.5 hover:bg-white/10 rounded" title="New file"><Plus size={16} /></button>
  <button on:click={() => dispatch('save')} disabled={!canSave} class="p-1.5 hover:bg-white/10 rounded disabled:opacity-40" title="Save (Ctrl+S)"><Save size={16} /></button>
  <div class="w-px h-5 bg-white/10 mx-1" />
  <button on:click={() => dispatch('toggleTerminal')} class="p-1.5 rounded {showTerminal ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}" title="Terminal"><TerminalIcon size={16} /></button>
  <button on:click={() => dispatch('setTab', 'search')} class="p-1.5 rounded {sidebarTab === 'search' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}" title="Search"><Search size={16} /></button>
  <button on:click={() => dispatch('setTab', 'git')} class="p-1.5 rounded {sidebarTab === 'git' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}" title="Git"><GitBranch size={16} /></button>
  <button on:click={() => dispatch('setTab', 'dev')} class="p-1.5 rounded {sidebarTab === 'dev' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}" title="Active Dev Mode"><Zap size={16} /></button>
  <button on:click={() => dispatch('commandPalette')} class="p-1.5 hover:bg-white/10 rounded" title="Command Palette (Ctrl+Shift+P)"><Command size={16} /></button>
  <div class="flex-1" />
  <div class="text-xs text-slate-500 truncate max-w-64">{activeFile?.path || 'No file open'}</div>
  {#each Object.entries(lspStatus) as [lang, active] (lang)}
    <div title="LSP: {lang} {active ? 'active' : 'inactive'}" class="w-2 h-2 rounded-full {active ? 'bg-green-400' : 'bg-slate-600'}" />
  {/each}
  <button on:click={() => dispatch('refresh')} class="p-1.5 hover:bg-white/10 rounded"><RefreshCw size={14} /></button>
</div>
