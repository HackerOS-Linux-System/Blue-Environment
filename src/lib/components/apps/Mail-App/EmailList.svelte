<script lang="ts">
  import { RefreshCw, Check, Archive, Trash2, Star, Mail as MailIcon } from 'lucide-svelte';
  import type { MailState } from './mailState';

  export let state: MailState;

  const { activeFolder, selectedEmail, selectedIds, loading, displayEmails, searchQuery, bulkAction, toggleSelectId, selectEmail, formatDate } = state;
</script>

<div class="border-r border-white/5 flex flex-col {$selectedEmail ? 'w-72' : 'flex-1'}">
  <div class="h-12 bg-slate-800/50 border-b border-white/5 flex items-center px-3 gap-2">
    <span class="font-semibold text-white capitalize flex-1">{$activeFolder}</span>
    <button on:click={() => loading.update((l) => !l)} class="p-1.5 hover:bg-white/10 rounded-lg" title="Refresh">
      <RefreshCw size={15} class={$loading ? 'animate-spin' : ''} />
    </button>
    {#if $selectedIds.length > 0}
      <div class="flex items-center gap-1">
        <button on:click={() => bulkAction('read')} class="p-1.5 hover:bg-white/10 rounded text-xs text-slate-400 hover:text-white" title="Mark read"><Check size={14} /></button>
        <button on:click={() => bulkAction('archive')} class="p-1.5 hover:bg-white/10 rounded text-xs text-slate-400 hover:text-white" title="Archive"><Archive size={14} /></button>
        <button on:click={() => bulkAction('delete')} class="p-1.5 hover:bg-red-500/20 rounded text-red-400 hover:text-red-300" title="Delete"><Trash2 size={14} /></button>
        <span class="text-xs text-slate-500">{$selectedIds.length}</span>
      </div>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto">
    {#if $displayEmails.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-slate-500 gap-3">
        <MailIcon size={40} class="opacity-20" />
        <p class="text-sm">{$searchQuery ? 'No results found' : 'No emails'}</p>
      </div>
    {/if}
    {#each $displayEmails as email (email.id)}
      <div on:click={() => selectEmail(email)}
        class="p-3 border-b border-white/5 cursor-pointer transition-colors group {$selectedEmail?.id === email.id ? 'bg-blue-600/20 border-l-2 border-l-blue-500' : !email.read ? 'bg-slate-800/30 hover:bg-white/5' : 'hover:bg-white/5'}">
        <div class="flex items-start gap-2">
          <input type="checkbox" checked={$selectedIds.includes(email.id)}
            on:change={(e) => toggleSelectId(email.id, e.currentTarget.checked)}
            on:click|stopPropagation
            class="mt-1 accent-blue-500 opacity-0 group-hover:opacity-100 transition-opacity" />
          <div class="w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold shrink-0 {email.from.name === 'Blue Environment' ? 'bg-blue-600' : 'bg-slate-700'}">
            {email.from.name.charAt(0).toUpperCase()}
          </div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center justify-between gap-1">
              <span class="text-sm truncate {!email.read ? 'font-semibold text-white' : 'text-slate-300'}">{email.from.name}</span>
              <div class="flex items-center gap-1 shrink-0">
                {#if email.starred}<Star size={12} class="text-yellow-400 fill-yellow-400" />{/if}
                <span class="text-[10px] text-slate-500">{formatDate(email.date)}</span>
              </div>
            </div>
            <div class="text-xs truncate mt-0.5 {!email.read ? 'font-medium text-slate-200' : 'text-slate-400'}">{email.subject}</div>
            <div class="text-[11px] text-slate-500 truncate mt-0.5">{email.body.split('\n')[0]}</div>
          </div>
        </div>
      </div>
    {/each}
  </div>
</div>
