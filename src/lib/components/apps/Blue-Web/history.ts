import { writable, get } from 'svelte/store';
import type { HistoryEntry, BookmarkItem } from './types';

const LS_HISTORY = 'bw_history';
const LS_BOOKMARKS = 'bw_bookmarks';
const MAX_HISTORY = 200;

function load<T>(key: string, fallback: T): T {
  try { return JSON.parse(localStorage.getItem(key) ?? 'null') ?? fallback; } catch { return fallback; }
}

export function createHistory() {
  const history = writable<HistoryEntry[]>(load(LS_HISTORY, []));
  const bookmarks = writable<BookmarkItem[]>(load(LS_BOOKMARKS, []));
  const navStack = writable<string[]>([]);
  const navIdx = writable(-1);

  function addHistory(url: string, title: string) {
    navStack.update((prev) => {
      const next = [...prev.slice(0, get(navIdx) + 1), url];
      navIdx.set(next.length - 1);
      return next;
    });
    history.update((prev) => {
      const next = [{ url, title, time: Date.now() }, ...prev.filter((h) => h.url !== url)].slice(0, MAX_HISTORY);
      localStorage.setItem(LS_HISTORY, JSON.stringify(next));
      return next;
    });
  }

  function clearHistory() { history.set([]); localStorage.removeItem(LS_HISTORY); }

  function toggleBookmark(url: string, title: string) {
    bookmarks.update((prev) => {
      const next = prev.some((b) => b.url === url) ? prev.filter((b) => b.url !== url) : [{ url, title }, ...prev];
      localStorage.setItem(LS_BOOKMARKS, JSON.stringify(next));
      return next;
    });
  }

  function isBookmarked(url: string) { return get(bookmarks).some((b) => b.url === url); }

  function goBackNav(): string | null {
    if (get(navIdx) <= 0) return null;
    const idx = get(navIdx) - 1;
    navIdx.set(idx);
    return get(navStack)[idx];
  }
  function goForwardNav(): string | null {
    if (get(navIdx) >= get(navStack).length - 1) return null;
    const idx = get(navIdx) + 1;
    navIdx.set(idx);
    return get(navStack)[idx];
  }

  return { history, clearHistory, bookmarks, toggleBookmark, addHistory, isBookmarked, navStack, navIdx, goBackNav, goForwardNav };
}
