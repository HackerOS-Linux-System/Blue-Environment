import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { NewsSource, NewsArticle } from './types';

export function createNewsStore() {
  const sources = writable<NewsSource[]>([]);
  const articles = writable<NewsArticle[]>([]);
  const loading = writable(true);
  const error = writable<string | null>(null);

  async function loadSources() {
    sources.set(await SystemBridge.newsLoadSources());
  }

  async function refreshAll() {
    loading.set(true);
    error.set(null);
    try {
      articles.set(await SystemBridge.newsFetchAll());
    } finally {
      loading.set(false);
    }
  }

  async function addSource(name: string, url: string, category: string) {
    const res = await SystemBridge.newsAddSource(name, url, category);
    if (!res.ok) { error.set(res.error ?? 'Failed to add source'); return; }
    if (res.source) sources.update((prev) => [...prev, res.source!]);
  }

  async function removeSource(id: string) {
    const res = await SystemBridge.newsRemoveSource(id);
    if (!res.ok) { error.set(res.error ?? 'Failed to remove source'); return; }
    sources.update((prev) => prev.filter((s) => s.id !== id));
    articles.update((prev) => prev.filter((a) => a.sourceId !== id));
  }

  async function toggleSource(id: string) {
    const src = get(sources).find((s) => s.id === id);
    if (!src) return;
    const enabled = !src.enabled;
    sources.update((prev) => prev.map((s) => (s.id === id ? { ...s, enabled } : s)));
    await SystemBridge.newsSetSourceEnabled(id, enabled);
  }

  return { sources, articles, loading, error, loadSources, refreshAll, addSource, removeSource, toggleSource };
}

export type NewsStore = ReturnType<typeof createNewsStore>;
