<script lang="ts">
  import { Languages, ArrowLeftRight, Copy, Volume2, X, Clock, Trash2, Check, Loader2, AlertCircle } from 'lucide-svelte';
  import { createTranslateState } from './translate';
  import { LANGUAGES } from './types';

  const t = createTranslateState();
  const { sourceText, translatedText, fromLang, toLang, loading, error, backend, history } = t;

  let showHistory = false;
  let copied = false;
  let debounceTimer: ReturnType<typeof setTimeout>;

  $: {
    // Debounced auto-translate as the user types, Android-Translate style.
    $sourceText;
    clearTimeout(debounceTimer);
    if ($sourceText.trim()) debounceTimer = setTimeout(() => t.translate(), 500);
    else translatedText.set('');
  }

  function copyResult() {
    navigator.clipboard.writeText($translatedText);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function speak(text: string, lang: string) {
    if (!('speechSynthesis' in window) || !text) return;
    const u = new SpeechSynthesisUtterance(text);
    if (lang !== 'auto') u.lang = lang;
    speechSynthesis.speak(u);
  }

  $: fromName = LANGUAGES.find((l) => l.code === $fromLang)?.name ?? $fromLang;
  $: toName = LANGUAGES.find((l) => l.code === $toLang)?.name ?? $toLang;
</script>

<div class="flex flex-col h-full bg-slate-900 text-white overflow-hidden">
  <div class="shrink-0 flex items-center gap-2 px-4 py-3 border-b border-white/5 bg-slate-800/50">
    <Languages size={18} class="text-blue-400" />
    <span class="font-semibold text-sm">Translate</span>
    <div class="flex-1" />
    <button on:click={() => (showHistory = !showHistory)} class="p-1.5 rounded-lg transition-colors {showHistory ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-400'}">
      <Clock size={16} />
    </button>
  </div>

  <div class="shrink-0 flex items-center gap-2 px-4 py-2 border-b border-white/5">
    <select bind:value={$fromLang} class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
      {#each LANGUAGES as l (l.code)}<option value={l.code}>{l.name}</option>{/each}
    </select>
    <button on:click={t.swap} disabled={$fromLang === 'auto'} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 disabled:opacity-30 transition-colors">
      <ArrowLeftRight size={15} />
    </button>
    <select bind:value={$toLang} class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
      {#each LANGUAGES.filter((l) => l.code !== 'auto') as l (l.code)}<option value={l.code}>{l.name}</option>{/each}
    </select>
  </div>

  <div class="flex-1 flex overflow-hidden">
    <div class="flex-1 flex flex-col overflow-hidden">
      <div class="flex-1 relative">
        <textarea bind:value={$sourceText} placeholder="Type to translate…"
          class="w-full h-full bg-transparent text-white p-4 resize-none focus:outline-none text-base leading-relaxed" />
        {#if $sourceText}
          <button on:click={() => sourceText.set('')} class="absolute top-3 right-3 p-1 hover:bg-white/10 rounded-full text-slate-500"><X size={14} /></button>
        {/if}
      </div>
      <div class="shrink-0 flex items-center justify-between px-4 py-2 border-t border-white/5">
        <span class="text-xs text-slate-600">{fromName}</span>
        <button on:click={() => speak($sourceText, $fromLang)} disabled={!$sourceText} class="p-1.5 hover:bg-white/10 rounded-full text-slate-500 disabled:opacity-30"><Volume2 size={14} /></button>
      </div>
    </div>

    <div class="w-px bg-white/5" />

    <div class="flex-1 flex flex-col overflow-hidden bg-slate-800/20">
      <div class="flex-1 p-4 overflow-y-auto">
        {#if $loading}
          <div class="flex items-center gap-2 text-slate-500 text-sm"><Loader2 size={15} class="animate-spin" /> Translating…</div>
        {:else if $error}
          <div class="flex items-start gap-2 text-amber-400 text-sm"><AlertCircle size={15} class="shrink-0 mt-0.5" /><span>{$error}</span></div>
        {:else}
          <p class="text-base leading-relaxed whitespace-pre-wrap">{$translatedText}</p>
        {/if}
      </div>
      <div class="shrink-0 flex items-center justify-between px-4 py-2 border-t border-white/5">
        <span class="text-xs text-slate-600">{toName}{#if $backend === 'ai'}<span class="ml-1.5 text-slate-700">· via AI</span>{/if}</span>
        <div class="flex gap-1">
          <button on:click={() => speak($translatedText, $toLang)} disabled={!$translatedText} class="p-1.5 hover:bg-white/10 rounded-full text-slate-500 disabled:opacity-30"><Volume2 size={14} /></button>
          <button on:click={copyResult} disabled={!$translatedText} class="p-1.5 hover:bg-white/10 rounded-full text-slate-500 disabled:opacity-30">
            {#if copied}<Check size={14} class="text-green-400" />{:else}<Copy size={14} />{/if}
          </button>
        </div>
      </div>
    </div>

    {#if showHistory}
      <div class="w-64 border-l border-white/5 flex flex-col shrink-0 bg-slate-900">
        <div class="flex items-center justify-between px-3 py-2 border-b border-white/5">
          <span class="text-xs font-medium text-slate-400">History</span>
          {#if $history.length > 0}
            <button on:click={t.clearHistory} class="text-slate-600 hover:text-red-400"><Trash2 size={12} /></button>
          {/if}
        </div>
        <div class="flex-1 overflow-y-auto">
          {#if $history.length === 0}
            <p class="text-xs text-slate-600 text-center py-8">No translations yet</p>
          {/if}
          {#each $history as h (h.id)}
            <button on:click={() => t.useHistoryEntry(h)} class="w-full text-left px-3 py-2 border-b border-white/5 hover:bg-white/5 transition-colors">
              <div class="text-xs text-slate-300 truncate">{h.sourceText}</div>
              <div class="text-xs text-blue-400 truncate mt-0.5">{h.translatedText}</div>
              <div class="text-[10px] text-slate-600 mt-1">{h.from} → {h.to}</div>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
