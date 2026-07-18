<script lang="ts">
  import { Plus, Search, X } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { FOLDERS } from './types';
  import type { MailState } from './mailState';

  export let state: MailState;
  export let t: (key: string) => string;

  const { activeFolder, searchQuery, emails, openCompose, selectFolder } = state;
  $: counts = Object.fromEntries(FOLDERS.map((f) => [f.id, $emails.filter((e) => e.folder === f.id && !e.read).length]));
</script>

<div class="w-52 bg-slate-800/50 border-r border-white/5 flex flex-col">
  <div class="p-3">
    <button on:click={openCompose} class="w-full bg-blue-600 hover:bg-blue-500 text-white py-2.5 rounded-xl flex items-center justify-center gap-2 font-medium transition-colors shadow-lg shadow-blue-500/20">
      <Plus size={18} /> {t('mail.compose')}
    </button>
  </div>

  <div class="px-3 pb-2">
    <div class="flex items-center gap-2 bg-slate-700/50 rounded-lg px-3 py-2">
      <Search size={13} class="text-slate-400 shrink-0" />
      <input type="text" placeholder="Search mail..." bind:value={$searchQuery} class="bg-transparent text-sm text-white flex-1 focus:outline-none placeholder-slate-500" />
      {#if $searchQuery}<button on:click={() => searchQuery.set('')}><X size={12} class="text-slate-400 hover:text-white" /></button>{/if}
    </div>
  </div>

  <div class="flex-1 overflow-y-auto px-2 space-y-0.5">
    {#each FOLDERS as folder (folder.id)}
      {@const count = counts[folder.id] ?? 0}
      <button on:click={() => selectFolder(folder.id)} class="w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors text-sm {$activeFolder === folder.id ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
        <svelte:component this={folder.icon} size={16} />
        <span class="flex-1 text-left">{folder.label}</span>
        {#if count > 0}
          <span class="text-xs px-1.5 py-0.5 rounded-full font-medium {$activeFolder === folder.id ? 'bg-white/20 text-white' : 'bg-blue-600/30 text-blue-400'}">{count}</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="p-3 border-t border-white/5">
    <div class="flex items-center gap-2 px-2">
      <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-xs font-bold">
        {SystemBridge.getUsername().charAt(0).toUpperCase()}
      </div>
      <div class="flex-1 min-w-0">
        <div class="text-xs font-medium text-white truncate">{SystemBridge.getUsername()}</div>
        <div class="text-[10px] text-slate-500 truncate">@blue.env</div>
      </div>
    </div>
  </div>
</div>
