import { writable, get } from 'svelte/store';
import type { Tab } from './types';
import { normalizeUrl } from './types';
import { SystemBridge } from '../../../utils/systemBridge';

function makeTab(url = ''): Tab {
  const id = `t${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const title = url ? (() => { try { return new URL(url).hostname; } catch { return url; } })() : 'New Tab';
  return { id, url, title, isNew: !url };
}

// Which tab ids already have a live embedded webview (backend-side) —
// tracked here, not just derived from `!tab.isNew`, because a tab keeps
// its webview alive across tab switches (hidden, not destroyed — see
// BlueWebApp.svelte's module doc) even though `isNew` only describes
// whether it's ever been navigated at all.
const liveWebviews = new Set<string>();

export function createTabs(onNavigate: (url: string, tabId: string) => void) {
  const first = makeTab();
  const tabs = writable<Tab[]>([first]);
  const activeId = writable(first.id);

  /**
   * Navigate a tab to `rawUrl`. First navigation for a tab creates its
   * embedded webview (needs real pixel bounds from the caller, since
   * this module has no DOM access — BlueWebApp.svelte passes the
   * content area's current rect); subsequent navigations in the same
   * tab just call `web_view_navigate` on the existing one.
   */
  async function openUrl(rawUrl: string, tabId?: string, bounds?: { x: number; y: number; width: number; height: number }) {
    const url = normalizeUrl(rawUrl);
    const id = tabId ?? get(activeId);
    const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();

    tabs.update((prev) => prev.map((t) => (t.id === id ? { ...t, url, title, isNew: false } : t)));
    onNavigate(url, id);

    if (!SystemBridge.isTauri()) {
      window.open(url, '_blank', 'noopener');
      return;
    }

    if (liveWebviews.has(id)) {
      try { await SystemBridge.invokeCommand('web_view_navigate', { tabId: id, url }); } catch {}
      return;
    }

    // First navigation for this tab — need real bounds to create the
    // webview at the right place; if the caller couldn't measure yet
    // (content area not mounted this frame), fall back to a
    // placeholder rect the bounds-sync rAF loop will correct on its
    // very next frame rather than skip creation entirely.
    const b = bounds ?? { x: 0, y: 0, width: 800, height: 600 };
    try {
      await SystemBridge.invokeCommand('web_view_create', {
        windowLabel: 'main', tabId: id, url, x: b.x, y: b.y, width: b.width, height: b.height,
      });
      liveWebviews.add(id);
    } catch {}
  }

  function addTab() {
    const t = makeTab();
    tabs.update((prev) => [...prev, t]);
    activeId.set(t.id);
    return t.id;
  }

  async function closeTab(id: string, e?: MouseEvent) {
    e?.stopPropagation();
    const current = get(activeId);
    tabs.update((prev) => {
      if (prev.length === 1) { const t = makeTab(); activeId.set(t.id); return [t]; }
      const next = prev.filter((t) => t.id !== id);
      if (current === id) activeId.set(next[next.length - 1].id);
      return next;
    });
    if (liveWebviews.has(id)) {
      liveWebviews.delete(id);
      if (SystemBridge.isTauri()) {
        try { await SystemBridge.invokeCommand('web_view_close', { tabId: id }); } catch {}
      }
    }
  }

  /** Show `id`'s webview, hide every other live one. Called on tab switch. */
  async function setActiveWebview(id: string) {
    if (!SystemBridge.isTauri()) return;
    for (const otherId of liveWebviews) {
      if (otherId === id) continue;
      try { await SystemBridge.invokeCommand('web_view_set_visible', { tabId: otherId, visible: false }); } catch {}
    }
    if (liveWebviews.has(id)) {
      try { await SystemBridge.invokeCommand('web_view_set_visible', { tabId: id, visible: true }); } catch {}
    }
  }

  function hasLiveWebview(id: string): boolean {
    return liveWebviews.has(id);
  }

  async function reloadActive() {
    const id = get(activeId);
    if (liveWebviews.has(id) && SystemBridge.isTauri()) {
      try { await SystemBridge.invokeCommand('web_view_reload', { tabId: id }); } catch {}
    }
  }

  return { tabs, activeId, openUrl, addTab, closeTab, setActiveWebview, hasLiveWebview, reloadActive };
}
