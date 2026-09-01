<script lang="ts">
  import { Plus, X, Settings } from 'lucide-svelte';
  import type { TerminalSession } from './terminalSession';

  export let session: TerminalSession;
  export let showSettings: boolean;
  export let onToggleSettings: () => void;

  const { tabs, activeTab, newTab, closeTab } = session;
</script>

<div class="shrink-0 flex items-center bg-slate-900 border-b border-white/5 overflow-x-auto">
  {#each $tabs as tab (tab.id)}
    <div on:click={() => activeTab.set(tab.id)} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => activeTab.set(tab.id))(); } }}
      class="flex items-center gap-2 px-4 py-2.5 cursor-pointer shrink-0 border-r border-white/5 group transition-colors {$activeTab === tab.id ? 'bg-slate-950 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-800'}"
      style="max-width:160px;">
      <span class="text-xs truncate">{tab.title}</span>
      <button on:click={(e) => closeTab(tab.id, e)} class="ml-1 p-0.5 opacity-0 group-hover:opacity-100 hover:text-red-400 rounded transition-all shrink-0">
        <X size={10} />
      </button>
    </div>
  {/each}
  <button on:click={newTab} class="p-2.5 hover:bg-slate-800 text-slate-500 hover:text-white transition-colors shrink-0"><Plus size={14} /></button>
  <div class="ml-auto flex items-center gap-1 px-2">
    <button on:click={onToggleSettings} class="p-1.5 rounded transition-colors {showSettings ? 'text-blue-400 bg-blue-500/10' : 'text-slate-500 hover:text-white'}">
      <Settings size={13} />
    </button>
  </div>
</div>
