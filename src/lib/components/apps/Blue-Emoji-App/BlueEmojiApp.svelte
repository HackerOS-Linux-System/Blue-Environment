<script lang="ts">
  // Blue Emoji — new app, not an expansion of an existing one. A
  // searchable emoji picker: click any emoji to copy it to the
  // clipboard (via SystemBridge.copyText — the same real
  // tauri-plugin-clipboard-manager path every other copy action in this
  // shell uses), with recents and favorites persisted locally (see
  // emojiStore.ts, same localStorage tier as Blue Web's history/
  // bookmarks — this isn't data that needs syncing or backup).
  import { onMount } from 'svelte';
  import { Search, Star, Clock, X, Check } from 'lucide-svelte';
  import { EMOJI_CATEGORIES } from './emojiData';
  import { createEmojiStore } from './emojiStore';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { t } from '../../../stores/language';

  export let windowId: string;

  const store = createEmojiStore();
  const { recent, favorites } = store;

  let query = '';
  let activeCategory = EMOJI_CATEGORIES[0].id;
  let justCopied: string | null = null;
  let copiedTimer: ReturnType<typeof setTimeout>;
  let searchInputEl: HTMLInputElement;

  // Simple keyword index — emoji Unicode code points don't carry a
  // machine-readable name, so search matches against a small
  // hand-authored keyword list per emoji covering common search terms
  // (not exhaustive; unmapped emojis are still browsable by category,
  // just not searchable by name until they get a keyword entry).
  const KEYWORDS: Record<string, string> = {
    '😀':'grin happy smile', '😂':'laugh cry funny lol', '🥰':'love heart eyes',
    '😍':'love heart eyes crush', '😭':'cry sad sob tears', '😡':'angry mad rage',
    '🤔':'think thinking hmm', '👍':'thumbs up yes good like', '👎':'thumbs down no bad dislike',
    '👏':'clap applause bravo', '🙏':'pray please thanks folded hands',
    '❤️':'heart love red', '💔':'broken heart heartbreak', '🔥':'fire lit hot flame',
    '💯':'hundred perfect score', '🎉':'party celebrate confetti tada',
    '✅':'check done yes correct', '❌':'x no wrong cancel', '⭐':'star favorite',
    '🐶':'dog puppy', '🐱':'cat kitten', '🍕':'pizza food', '☕':'coffee',
    '🚀':'rocket launch ship', '💡':'idea light bulb', '⚡':'lightning bolt fast',
    '🎂':'birthday cake', '🎁':'gift present', '📌':'pin', '📎':'paperclip attach',
    '💻':'laptop computer', '📱':'phone mobile', '⏰':'alarm clock time',
    '🙈':'monkey see no evil', '🤷':'shrug idk', '😴':'sleep tired zzz',
    '🥳':'party celebrate birthday', '👀':'eyes look watching',
    '💪':'muscle strong flex', '🧠':'brain smart', '🎯':'target goal bullseye',
  };

  function matchesQuery(emoji: string): boolean {
    if (!query.trim()) return true;
    const q = query.trim().toLowerCase();
    return (KEYWORDS[emoji] ?? '').includes(q);
  }

  $: filteredCategories = EMOJI_CATEGORIES.map((cat) => ({
    ...cat,
    emojis: cat.emojis.filter(matchesQuery),
  })).filter((cat) => cat.emojis.length > 0);

  $: isSearching = query.trim().length > 0;

  async function pick(emoji: string) {
    await SystemBridge.copyText(emoji);
    store.recordUsed(emoji);
    justCopied = emoji;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (justCopied = null), 900);
  }

  function scrollToCategory(id: string) {
    activeCategory = id;
    const el = document.getElementById(`emoji-cat-${id}`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  onMount(() => searchInputEl?.focus());
</script>

<div class="flex flex-col h-full bg-slate-900 text-white text-sm select-none">
  <div class="flex items-center gap-2 px-3 py-2.5 border-b border-white/5 shrink-0">
    <Search size={14} class="text-slate-500 shrink-0" />
    <input bind:this={searchInputEl} bind:value={query}
      placeholder={$t('emoji.search_placeholder')}
      class="flex-1 bg-transparent text-sm focus:outline-none placeholder:text-slate-500" />
    {#if query}
      <button on:click={() => (query = '')}><X size={13} class="text-slate-500 hover:text-white" /></button>
    {/if}
  </div>

  {#if !isSearching}
    <div class="flex items-center gap-0.5 px-2 py-1.5 border-b border-white/5 overflow-x-auto shrink-0">
      <button on:click={() => scrollToCategory('recent')} title={$t('emoji.recent')}
        class="p-1.5 rounded-lg hover:bg-white/10 shrink-0 {activeCategory === 'recent' ? 'bg-white/10' : ''}">
        <Clock size={15} class="text-slate-400" />
      </button>
      <button on:click={() => scrollToCategory('favorites')} title={$t('emoji.favorites')}
        class="p-1.5 rounded-lg hover:bg-white/10 shrink-0 {activeCategory === 'favorites' ? 'bg-white/10' : ''}">
        <Star size={15} class="text-slate-400" />
      </button>
      <div class="w-px h-5 bg-white/10 mx-0.5 shrink-0" />
      {#each EMOJI_CATEGORIES as cat (cat.id)}
        <button on:click={() => scrollToCategory(cat.id)} title={$t(cat.nameKey)}
          class="text-base p-1.5 rounded-lg hover:bg-white/10 shrink-0 {activeCategory === cat.id ? 'bg-white/10' : ''}">
          {cat.icon}
        </button>
      {/each}
    </div>
  {/if}

  <div class="flex-1 overflow-y-auto px-2 py-2">
    {#if !isSearching && $recent.length > 0}
      <div id="emoji-cat-recent" class="mb-3">
        <div class="px-1 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wide flex items-center gap-1">
          <Clock size={10} /> {$t('emoji.recent')}
        </div>
        <div class="grid grid-cols-8 gap-0.5">
          {#each $recent as emoji (emoji)}
            <button on:click={() => pick(emoji)} title={emoji}
              class="relative aspect-square flex items-center justify-center text-xl rounded-lg hover:bg-white/10 transition-colors">
              {emoji}
              {#if justCopied === emoji}<span class="absolute inset-0 flex items-center justify-center bg-blue-500/80 rounded-lg"><Check size={14} /></span>{/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#if !isSearching}
      <div id="emoji-cat-favorites" class="mb-3">
        <div class="px-1 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wide flex items-center gap-1">
          <Star size={10} /> {$t('emoji.favorites')}
        </div>
        {#if $favorites.length === 0}
          <p class="px-1 text-xs text-slate-600">{$t('emoji.no_favorites')}</p>
        {:else}
          <div class="grid grid-cols-8 gap-0.5">
            {#each $favorites as emoji (emoji)}
              <button on:click={() => pick(emoji)} title={emoji}
                class="relative aspect-square flex items-center justify-center text-xl rounded-lg hover:bg-white/10 transition-colors">
                {emoji}
                {#if justCopied === emoji}<span class="absolute inset-0 flex items-center justify-center bg-blue-500/80 rounded-lg"><Check size={14} /></span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    {#each filteredCategories as cat (cat.id)}
      <div id="emoji-cat-{cat.id}" class="mb-3">
        {#if !isSearching}
          <div class="px-1 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wide">{$t(cat.nameKey)}</div>
        {/if}
        <div class="grid grid-cols-8 gap-0.5">
          {#each cat.emojis as emoji (emoji)}
            <button on:click={() => pick(emoji)} on:contextmenu|preventDefault={() => store.toggleFavorite(emoji)}
              title={emoji + ' — ' + $t('emoji.right_click_favorite')}
              class="relative aspect-square flex items-center justify-center text-xl rounded-lg hover:bg-white/10 transition-colors group">
              {emoji}
              {#if $favorites.includes(emoji)}
                <Star size={9} class="absolute bottom-0.5 right-0.5 text-yellow-400 fill-yellow-400 opacity-70" />
              {/if}
              {#if justCopied === emoji}<span class="absolute inset-0 flex items-center justify-center bg-blue-500/80 rounded-lg"><Check size={14} /></span>{/if}
            </button>
          {/each}
        </div>
      </div>
    {/each}

    {#if isSearching && filteredCategories.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-slate-500 gap-2 py-12">
        <Search size={24} class="opacity-30" />
        <span class="text-xs">{$t('emoji.no_results')}</span>
      </div>
    {/if}
  </div>

  <div class="px-3 py-1.5 border-t border-white/5 text-[10px] text-slate-600 shrink-0">
    {$t('emoji.footer_hint')}
  </div>
</div>
