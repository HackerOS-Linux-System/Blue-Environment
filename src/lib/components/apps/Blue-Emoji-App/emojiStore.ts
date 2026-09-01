import { writable } from 'svelte/store';

const LS_RECENT = 'blue-emoji-recent';
const LS_FAVORITES = 'blue-emoji-favorites';
const MAX_RECENT = 36;

function loadList(key: string): string[] {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

export function createEmojiStore() {
  const recent = writable<string[]>(loadList(LS_RECENT));
  const favorites = writable<string[]>(loadList(LS_FAVORITES));

  function recordUsed(emoji: string) {
    recent.update((prev) => {
      const next = [emoji, ...prev.filter((e) => e !== emoji)].slice(0, MAX_RECENT);
      try { localStorage.setItem(LS_RECENT, JSON.stringify(next)); } catch { /* best effort */ }
      return next;
    });
  }

  function toggleFavorite(emoji: string) {
    favorites.update((prev) => {
      const next = prev.includes(emoji) ? prev.filter((e) => e !== emoji) : [...prev, emoji];
      try { localStorage.setItem(LS_FAVORITES, JSON.stringify(next)); } catch { /* best effort */ }
      return next;
    });
  }

  return { recent, favorites, recordUsed, toggleFavorite };
}
