<script lang="ts">
  import { Star, StarOff, Reply, ReplyAll, Forward, Archive, Trash2, X, FileText, Paperclip } from 'lucide-svelte';
  import type { MailState } from './mailState';

  export let state: MailState;
  const { selectedEmail, toggleStar, openReply, moveToFolder, deleteEmail } = state;
</script>

{#if $selectedEmail}
  {@const email = $selectedEmail}
  <div class="flex-1 flex flex-col overflow-hidden">
    <div class="p-4 border-b border-white/5 bg-slate-800/30">
      <div class="flex items-start justify-between gap-3">
        <div class="flex-1 min-w-0">
          <h2 class="text-lg font-semibold text-white">{email.subject}</h2>
          <div class="flex items-center gap-2 mt-1">
            <div class="w-7 h-7 rounded-full bg-blue-700 flex items-center justify-center text-xs font-bold shrink-0">
              {email.from.name.charAt(0).toUpperCase()}
            </div>
            <div>
              <div class="text-sm font-medium text-white">{email.from.name}</div>
              <div class="text-xs text-slate-400">&lt;{email.from.email}&gt;</div>
            </div>
          </div>
          <div class="flex items-center gap-3 mt-1.5 text-xs text-slate-500">
            <span>To: {email.to.map((t) => t.email).join(', ')}</span>
            <span>{email.date.toLocaleString()}</span>
          </div>
        </div>
        <div class="flex items-center gap-1 shrink-0">
          <button on:click={() => toggleStar(email.id)} class="p-2 hover:bg-white/10 rounded-lg" title="Star">
            {#if email.starred}<Star size={16} class="text-yellow-400 fill-yellow-400" />{:else}<StarOff size={16} class="text-slate-400" />{/if}
          </button>
          <button on:click={() => openReply('reply')} class="p-2 hover:bg-white/10 rounded-lg" title="Reply"><Reply size={16} class="text-slate-400" /></button>
          <button on:click={() => openReply('replyAll')} class="p-2 hover:bg-white/10 rounded-lg" title="Reply All"><ReplyAll size={16} class="text-slate-400" /></button>
          <button on:click={() => openReply('forward')} class="p-2 hover:bg-white/10 rounded-lg" title="Forward"><Forward size={16} class="text-slate-400" /></button>
          <button on:click={() => moveToFolder(email.id, 'archive')} class="p-2 hover:bg-white/10 rounded-lg" title="Archive"><Archive size={16} class="text-slate-400" /></button>
          <button on:click={() => deleteEmail(email.id)} class="p-2 hover:bg-red-500/20 rounded-lg" title="Delete"><Trash2 size={16} class="text-red-400" /></button>
          <button on:click={() => selectedEmail.set(null)} class="p-2 hover:bg-white/10 rounded-lg ml-1"><X size={16} class="text-slate-400" /></button>
        </div>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-6">
      <div class="max-w-3xl">
        <pre class="text-sm text-slate-200 whitespace-pre-wrap break-words font-sans leading-relaxed">{email.body}</pre>
        {#if email.attachments && email.attachments.length > 0}
          <div class="mt-6 pt-4 border-t border-white/5">
            <div class="text-xs font-medium text-slate-400 mb-2 flex items-center gap-1"><Paperclip size={12} /> {email.attachments.length} attachment(s)</div>
            <div class="flex gap-2 flex-wrap">
              {#each email.attachments as att, i (i)}
                <div class="flex items-center gap-2 bg-slate-800 rounded-lg px-3 py-2 text-xs">
                  <FileText size={14} class="text-blue-400" />
                  <span class="text-slate-200">{att.name}</span>
                  <span class="text-slate-500">{att.size}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
