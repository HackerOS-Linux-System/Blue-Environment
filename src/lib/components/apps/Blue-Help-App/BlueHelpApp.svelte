<script lang="ts">
  import { Search, LifeBuoy, ChevronRight, Sparkles } from 'lucide-svelte';
  import { language } from '../../../stores/language';
  import { openApp } from '../../../stores/windowManager';
  import { markWelcomeCompleted } from '../Blue-Welcome-App/welcome';
  import { AppId } from '../../../types';
  import { HELP_ARTICLES, HELP_CATEGORIES, type HelpArticle } from './helpContent';

  // English fallback for any interface language other than Polish —
  // see helpContent.ts's module doc for why this dataset only carries
  // those two languages directly instead of running through the global
  // translation dictionaries.
  function pick(text: { en: string; pl: string }): string {
    return $language === 'pl' ? text.pl : text.en;
  }

  let query = '';
  let selectedId: string = HELP_ARTICLES[0].id;

  $: filtered = query.trim()
    ? HELP_ARTICLES.filter((a) => pick(a.title).toLowerCase().includes(query.trim().toLowerCase()))
    : HELP_ARTICLES;

  $: byCategory = (Object.keys(HELP_CATEGORIES) as HelpArticle['category'][])
    .map((cat) => ({ cat, articles: filtered.filter((a) => a.category === cat) }))
    .filter((g) => g.articles.length > 0);

  $: selected = HELP_ARTICLES.find((a) => a.id === selectedId) ?? HELP_ARTICLES[0];

  function replayWelcome() {
    // Deliberately does NOT call markWelcomeCompleted() — this just
    // opens the wizard on top of the current session the same way any
    // other app opens, it doesn't touch the "have I seen this before"
    // flag at all (there's nothing to undo: markWelcomeCompleted only
    // ever gets called when the wizard's own Finish button is pressed).
    openApp(AppId.BLUE_WELCOME);
  }
</script>

<div class="h-full flex bg-slate-950 text-white">
  <!-- Sidebar -->
  <div class="w-64 shrink-0 border-r border-white/5 flex flex-col">
    <div class="p-4 border-b border-white/5">
      <div class="flex items-center gap-2 mb-3">
        <div class="w-8 h-8 rounded-xl bg-blue-500/15 flex items-center justify-center">
          <LifeBuoy size={16} class="text-blue-400" />
        </div>
        <h1 class="font-bold">Blue Help</h1>
      </div>
      <div class="relative">
        <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
        <input
          type="text"
          bind:value={query}
          placeholder={$language === 'pl' ? 'Szukaj\u2026' : 'Search\u2026'}
          class="w-full bg-slate-900 border border-white/10 rounded-xl pl-8 pr-3 py-2 text-sm text-white placeholder:text-slate-600" />
      </div>
    </div>

    <div class="flex-1 overflow-y-auto py-2">
      {#each byCategory as group (group.cat)}
        <div class="px-4 pt-3 pb-1 text-[11px] font-semibold uppercase tracking-wide text-slate-500">
          {pick(HELP_CATEGORIES[group.cat])}
        </div>
        {#each group.articles as article (article.id)}
          <button
            on:click={() => (selectedId = article.id)}
            class="w-full flex items-center gap-2 px-4 py-2 text-sm text-left transition-colors {selectedId === article.id ? 'bg-blue-600/15 text-blue-300' : 'text-slate-300 hover:bg-white/5'}">
            <span class="flex-1 truncate">{pick(article.title)}</span>
            <ChevronRight size={13} class="shrink-0 opacity-50" />
          </button>
        {/each}
      {/each}
      {#if byCategory.length === 0}
        <p class="text-sm text-slate-500 text-center py-8 px-4">
          {$language === 'pl' ? 'Brak wyników.' : 'No results.'}
        </p>
      {/if}
    </div>

    <button on:click={replayWelcome} class="flex items-center gap-2 p-4 border-t border-white/5 text-xs text-slate-400 hover:text-white hover:bg-white/5 transition-colors">
      <Sparkles size={14} class="text-blue-400" />
      {$language === 'pl' ? 'Uruchom ponownie samouczek powitalny' : 'Replay the welcome tour'}
    </button>
  </div>

  <!-- Article -->
  <div class="flex-1 overflow-y-auto p-8">
    <div class="max-w-2xl mx-auto">
      <h2 class="text-2xl font-bold mb-4">{pick(selected.title)}</h2>
      <div class="space-y-3 text-sm text-slate-300 leading-relaxed">
        {#each ($language === 'pl' ? selected.body.pl : selected.body.en) as paragraph}
          <p>{paragraph}</p>
        {/each}
      </div>
    </div>
  </div>
</div>
