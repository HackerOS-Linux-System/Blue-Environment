<script lang="ts">
  // Blue News — new app. RSS/Atom reader, sharing its feed parser with
  // Blue Notifications (see src-tauri/src/feed_parser.rs's module doc)
  // and opening article links in Blue Web (see openInBlueWeb.ts) rather
  // than shelling out to the OS default browser — same shell-integration
  // pattern used for AboutApp's links.
  import { onMount } from 'svelte';
  import { Rss, Plus, Trash2, RefreshCw, X, Newspaper, ExternalLink, Bookmark, ListPlus } from 'lucide-svelte';
  import { createNewsStore } from './newsStore';
  import type { NewsArticle } from './types';
  import { openInBlueWeb } from '../../../utils/openInBlueWeb';
  import { openApp } from '../../../stores/windowManager';
  import { AppId } from '../../../types';
  import { t } from '../../../stores/language';

  export let windowId: string;

  const store = createNewsStore();
  const { sources, articles, loading, error } = store;

  let showAdd = false;
  let draftName = '';
  let draftUrl = '';
  let draftCategory = '';
  let activeCategory = 'all';
  let selectedArticle: NewsArticle | null = null;

  $: categories = ['all', ...new Set($sources.map((s) => s.category))];
  $: visibleArticles = activeCategory === 'all'
    ? $articles
    : $articles.filter((a) => $sources.find((s) => s.id === a.sourceId)?.category === activeCategory);

  onMount(async () => {
    await store.loadSources();
    await store.refreshAll();
  });

  async function addSource() {
    const name = draftName.trim();
    const url = draftUrl.trim();
    if (!name || !url) return;
    await store.addSource(name, url, draftCategory.trim() || 'General');
    draftName = ''; draftUrl = ''; draftCategory = ''; showAdd = false;
    store.refreshAll();
  }

  function fmtDate(published: string): string {
    if (!published) return '';
    const d = new Date(published);
    if (isNaN(d.getTime())) return published;
    const diffH = (Date.now() - d.getTime()) / 3_600_000;
    if (diffH < 1) return $t('news.just_now');
    if (diffH < 24) return `${Math.floor(diffH)}h`;
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  function stripHtml(html: string): string {
    return html.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
  }

  function openArticle(article: NewsArticle) {
    if (article.link) openInBlueWeb(article.link);
  }

  function saveForLater(article: NewsArticle) {
    openApp(AppId.BLUE_TASKS, false, undefined, {
      prefillTitle: article.title,
      prefillUrl: article.link,
    });
  }
</script>

<div class="flex h-full bg-slate-900 text-white text-sm">
  <!-- Sources sidebar -->
  <div class="w-52 shrink-0 border-r border-white/5 flex flex-col bg-slate-950/40">
    <div class="flex items-center gap-2 px-3 h-11 border-b border-white/5 shrink-0">
      <Newspaper size={16} class="text-blue-400" />
      <span class="font-semibold">{$t('news.title')}</span>
    </div>
    <div class="flex-1 overflow-y-auto py-1">
      {#each categories as cat}
        <button on:click={() => (activeCategory = cat)}
          class="w-full flex items-center px-3 py-2 text-left hover:bg-white/5 {activeCategory === cat ? 'bg-white/10 text-white' : 'text-slate-400'}">
          <span class="truncate">{cat === 'all' ? $t('news.all_sources') : cat}</span>
        </button>
      {/each}
      <div class="h-px bg-white/5 my-1 mx-3" />
      {#each $sources as src (src.id)}
        <div class="group flex items-center gap-2 px-3 py-1.5 hover:bg-white/5">
          <button on:click={() => store.toggleSource(src.id)} class="w-2 h-2 rounded-full shrink-0 {src.enabled ? 'bg-green-400' : 'bg-slate-600'}" title={src.enabled ? $t('news.enabled') : $t('news.disabled')} />
          <span class="flex-1 min-w-0 truncate text-xs {src.enabled ? 'text-slate-300' : 'text-slate-600'}">{src.name}</span>
          <button on:click={() => store.removeSource(src.id)} class="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400 shrink-0"><X size={11} /></button>
        </div>
      {/each}
    </div>
    <div class="p-2 border-t border-white/5 shrink-0">
      {#if showAdd}
        <div class="flex flex-col gap-1.5">
          <input bind:value={draftName} placeholder={$t('news.name_placeholder')} class="bg-slate-800 rounded-md px-2 py-1 text-xs focus:outline-none" />
          <input bind:value={draftUrl} placeholder={$t('news.url_placeholder')} class="bg-slate-800 rounded-md px-2 py-1 text-xs focus:outline-none font-mono" />
          <input bind:value={draftCategory} placeholder={$t('news.category_placeholder')} class="bg-slate-800 rounded-md px-2 py-1 text-xs focus:outline-none" />
          <div class="flex justify-end gap-1.5 mt-0.5">
            <button on:click={() => (showAdd = false)} class="text-[10px] px-2 py-1 rounded-md hover:bg-white/5 text-slate-400">{$t('news.cancel')}</button>
            <button on:click={addSource} disabled={!draftName.trim() || !draftUrl.trim()} class="text-[10px] px-2 py-1 rounded-md bg-blue-600 hover:bg-blue-500 disabled:opacity-40">{$t('news.add')}</button>
          </div>
        </div>
      {:else}
        <button on:click={() => (showAdd = true)} class="w-full flex items-center gap-1.5 px-2 py-1.5 rounded-md text-slate-400 hover:bg-white/5 hover:text-white text-xs">
          <Plus size={13} /> {$t('news.add_source')}
        </button>
      {/if}
    </div>
  </div>

  <!-- Article list -->
  <div class="flex-1 flex flex-col min-w-0 border-r border-white/5">
    <div class="flex items-center justify-between px-4 h-11 border-b border-white/5 shrink-0">
      <span class="font-medium">{activeCategory === 'all' ? $t('news.all_sources') : activeCategory}</span>
      <button on:click={() => store.refreshAll()} class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400">
        <RefreshCw size={14} class={$loading ? 'animate-spin' : ''} />
      </button>
    </div>

    {#if $error}
      <div class="px-4 py-2 bg-red-500/10 text-red-300 text-xs shrink-0">{$error}</div>
    {/if}

    <div class="flex-1 overflow-y-auto">
      {#if $loading && $articles.length === 0}
        <div class="flex items-center justify-center h-full text-slate-500 text-xs">{$t('news.loading')}</div>
      {:else if visibleArticles.length === 0}
        <div class="flex flex-col items-center justify-center h-full text-slate-500 gap-2">
          <Rss size={28} class="opacity-30" />
          <span class="text-xs">{$t('news.empty')}</span>
        </div>
      {:else}
        {#each visibleArticles as article (article.sourceId + article.guid)}
          <button on:click={() => (selectedArticle = article)}
            class="w-full flex flex-col gap-1 px-4 py-3 border-b border-white/[0.03] text-left hover:bg-white/5 {selectedArticle === article ? 'bg-blue-500/10' : ''}">
            <div class="flex items-center gap-2 text-[10px] text-slate-500">
              <span class="text-blue-400 font-medium">{article.sourceName}</span>
              {#if article.published}<span>· {fmtDate(article.published)}</span>{/if}
            </div>
            <div class="text-sm text-white font-medium line-clamp-2">{article.title}</div>
          </button>
        {/each}
      {/if}
    </div>
  </div>

  <!-- Reading pane -->
  <div class="w-96 shrink-0 flex flex-col bg-slate-950/30">
    {#if selectedArticle}
      <div class="flex items-center justify-between px-4 h-11 border-b border-white/5 shrink-0">
        <span class="text-xs text-blue-400 font-medium truncate">{selectedArticle.sourceName}</span>
        <button on:click={() => (selectedArticle = null)}><X size={14} class="text-slate-400" /></button>
      </div>
      <div class="flex-1 overflow-y-auto p-4 flex flex-col gap-3">
        <h2 class="text-base font-semibold text-white leading-snug">{selectedArticle.title}</h2>
        {#if selectedArticle.published}<div class="text-[10px] text-slate-500">{fmtDate(selectedArticle.published)}</div>{/if}
        {#if selectedArticle.description}
          <p class="text-sm text-slate-300 leading-relaxed">{stripHtml(selectedArticle.description)}</p>
        {/if}
      </div>
      <div class="flex items-center gap-2 p-3 border-t border-white/5 shrink-0">
        <button on:click={() => selectedArticle && openArticle(selectedArticle)} class="flex-1 flex items-center justify-center gap-1.5 text-xs px-3 py-2 rounded-lg bg-blue-600 hover:bg-blue-500">
          <ExternalLink size={13} /> {$t('news.read_full')}
        </button>
        <button on:click={() => selectedArticle && saveForLater(selectedArticle)} title={$t('news.save_for_later')} class="p-2 rounded-lg bg-white/5 hover:bg-white/10">
          <ListPlus size={14} />
        </button>
      </div>
    {:else}
      <div class="flex flex-col items-center justify-center h-full text-slate-600 gap-2">
        <Newspaper size={28} class="opacity-30" />
        <span class="text-xs">{$t('news.select_article')}</span>
      </div>
    {/if}
  </div>
</div>
