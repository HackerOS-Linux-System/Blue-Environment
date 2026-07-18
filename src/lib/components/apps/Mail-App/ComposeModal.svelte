<script lang="ts">
  import { X, Paperclip, Send } from 'lucide-svelte';
  import type { MailState } from './mailState';

  export let state: MailState;
  export let t: (key: string) => string;

  const { composeOpen, composeData, showCc, replyMode, sendEmail, saveDraft, closeCompose } = state;
</script>

{#if $composeOpen}
  <div class="fixed inset-0 bg-black/60 flex items-end justify-end z-50 p-4 pb-16 backdrop-blur-sm">
    <div class="bg-slate-800 rounded-2xl w-[540px] shadow-2xl border border-white/10 flex flex-col max-h-[80vh]">
      <div class="flex items-center justify-between px-4 py-3 border-b border-white/5 bg-slate-700/50 rounded-t-2xl">
        <span class="font-semibold text-white">{$replyMode === 'forward' ? 'Forward' : $replyMode ? 'Reply' : t('mail.compose')}</span>
        <div class="flex gap-1">
          <button on:click={saveDraft} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white text-xs px-2">Save draft</button>
          <button on:click={closeCompose} class="p-1.5 hover:bg-white/10 rounded-lg"><X size={16} class="text-slate-400" /></button>
        </div>
      </div>

      <div class="border-b border-white/5">
        <div class="flex items-center px-4 py-2 border-b border-white/5">
          <span class="text-xs text-slate-500 w-10">To</span>
          <input type="email" bind:value={$composeData.to} placeholder="recipient@example.com" class="flex-1 bg-transparent text-sm text-white focus:outline-none" autofocus={!$replyMode} />
          <button on:click={() => showCc.update((v) => !v)} class="text-xs text-slate-500 hover:text-white">CC</button>
        </div>
        {#if $showCc}
          <div class="flex items-center px-4 py-2 border-b border-white/5">
            <span class="text-xs text-slate-500 w-10">CC</span>
            <input type="text" bind:value={$composeData.cc} placeholder="cc@example.com" class="flex-1 bg-transparent text-sm text-white focus:outline-none" />
          </div>
        {/if}
        <div class="flex items-center px-4 py-2">
          <span class="text-xs text-slate-500 w-10">{t('mail.subject')}</span>
          <input type="text" bind:value={$composeData.subject} placeholder="Subject" class="flex-1 bg-transparent text-sm text-white focus:outline-none font-medium" />
        </div>
      </div>

      <textarea bind:value={$composeData.body} placeholder={t('mail.body')} class="flex-1 bg-transparent text-sm text-slate-200 focus:outline-none p-4 resize-none min-h-[200px]" />

      <div class="flex items-center justify-between px-4 py-3 border-t border-white/5 bg-slate-700/30 rounded-b-2xl">
        <div class="flex gap-2">
          <button class="p-2 hover:bg-white/10 rounded-lg" title="Attach file"><Paperclip size={16} class="text-slate-400" /></button>
        </div>
        <button on:click={() => sendEmail()} disabled={!$composeData.to.trim() || !$composeData.subject.trim()}
          class="px-5 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-medium text-sm flex items-center gap-2 disabled:opacity-40 transition-colors">
          <Send size={15} /> {t('mail.send')}
        </button>
      </div>
    </div>
  </div>
{/if}
