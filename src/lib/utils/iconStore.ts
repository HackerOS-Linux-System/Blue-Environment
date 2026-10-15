import { SystemBridge } from './systemBridge';
import { iconThemes, ICON_THEMES_CHANGED } from './iconThemes';

/**
 * Frontend side of Blue Software → Icons. The catalogue is the KDE Store's
 * OCS API, queried by the backend (`src-tauri/src/icon_store_ocs.rs`) — the
 * webview never touches the network itself (its CSP forbids it), and nothing
 * of the website is embedded: Blue Software draws native cards instead.
 */

export interface StoreDownload {
  index: number;
  name: string;
  url: string;
  sizeKb: number | null;
  /** Blue can unpack it as an icon theme. */
  isArchive: boolean;
}

export interface StoreIcon {
  id: string;
  name: string;
  author: string;
  summary: string;
  description: string;
  version: string;
  thumb: string;
  previews: string[];
  downloadsCount: number;
  /** 0–100 */
  score: number;
  changed: string;
  detailUrl: string;
  downloads: StoreDownload[];
}

export interface StorePage {
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
  items: StoreIcon[];
}

export type StoreSort = 'new' | 'top' | 'downloads' | 'name';

export interface InstallProgress {
  id: string;
  phase: 'download' | 'install' | 'done';
  received: number;
  total: number | null;
}

export const PROGRESS_EVENT = 'icon-store-progress';

/** Which download to install when the person presses "Install" on a card. */
export function primaryDownload(item: Pick<StoreIcon, 'downloads'>): StoreDownload | null {
  return item.downloads.find((d) => d.isArchive) ?? null;
}

/** 0–100, or `null` while the total is unknown (indeterminate bar). */
export function progressPercent(p: Pick<InstallProgress, 'phase' | 'received' | 'total'> | undefined): number | null {
  if (!p) return null;
  if (p.phase !== 'download') return 100;
  if (!p.total || p.total <= 0) return null;
  return Math.max(0, Math.min(100, Math.floor((p.received / p.total) * 100)));
}

export function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`;
  if (n >= 10_000) return `${Math.round(n / 1000)}k`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

export function formatSize(kb: number | null): string {
  if (!kb || kb <= 0) return '';
  return kb >= 1024 ? `${(kb / 1024).toFixed(kb >= 10 * 1024 ? 0 : 1)} MB` : `${kb} KB`;
}

/** `2024-05-01 10:00:00` → `2024-05-01` (the store's own format; anything else is returned as is). */
export function shortDate(s: string): string {
  const m = /^(\d{4}-\d{2}-\d{2})/.exec(s ?? '');
  return m ? m[1] : s ?? '';
}

// ── thumbnails: cached, de-duplicated, at most N fetches in flight ─────────

const MAX_PARALLEL_THUMBS = 4;
const thumbCache = new Map<string, string>();
const thumbInflight = new Map<string, Promise<string>>();
const thumbWaiting: (() => void)[] = [];
let thumbActive = 0;

function acquireThumbSlot(): Promise<void> {
  if (thumbActive < MAX_PARALLEL_THUMBS) { thumbActive++; return Promise.resolve(); }
  return new Promise((resolve) => thumbWaiting.push(() => { thumbActive++; resolve(); }));
}
function releaseThumbSlot() {
  thumbActive--;
  thumbWaiting.shift()?.();
}

/** Resolves to a `data:` URL. Repeated calls for one URL share a single request. */
export function loadThumbnail(url: string): Promise<string> {
  const cached = thumbCache.get(url);
  if (cached) return Promise.resolve(cached);
  const running = thumbInflight.get(url);
  if (running) return running;
  const job = (async () => {
    await acquireThumbSlot();
    try {
      const data = await SystemBridge.invokeCommand<string>('icon_store_thumbnail', { url });
      thumbCache.set(url, data);
      return data;
    } finally {
      releaseThumbSlot();
      thumbInflight.delete(url);
    }
  })();
  thumbInflight.set(url, job);
  return job;
}

/** Test helper. */
export function _resetThumbnailCache() {
  thumbCache.clear(); thumbInflight.clear(); thumbWaiting.length = 0; thumbActive = 0;
}

// ── commands ───────────────────────────────────────────────────────────────

export const iconStore = {
  search: (query: string, sort: StoreSort, page: number, pageSize = 24) =>
    SystemBridge.invokeCommand<StorePage>('icon_store_search', { query, sort, page, pageSize }),

  details: (id: string) => SystemBridge.invokeCommand<StoreIcon>('icon_store_details', { id }),

  installedMap: (): Promise<Record<string, string[]>> =>
    SystemBridge.invokeCommand<Record<string, string[]>>('icon_store_installed_map').catch(() => ({})),

  /**
   * Downloads the file and installs it into the icon-theme folder
   * (`~/.local/share/icons`). Resolves to the installed theme names.
   */
  async install(item: Pick<StoreIcon, 'id'>, file: Pick<StoreDownload, 'url' | 'name'>): Promise<string[]> {
    const names = await SystemBridge.invokeCommand<string[]>('icon_store_install_remote', {
      contentId: item.id, url: file.url, fileName: file.name,
    });
    // Tell the rest of the shell (installed list, Settings → Personalization) that the set changed.
    await iconThemes.list();
    if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent(ICON_THEMES_CHANGED));
    return names;
  },

  async onProgress(cb: (p: InstallProgress) => void): Promise<() => void> {
    try {
      const ev = await import('@tauri-apps/api/event');
      return await ev.listen<InstallProgress>(PROGRESS_EVENT, (e) => cb(e.payload));
    } catch {
      return () => {};
    }
  },
};
