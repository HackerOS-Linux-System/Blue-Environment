export interface Tab {
    id: string;
    url: string;
    title: string;
    isNew: boolean;
    /** Private/incognito tab — see tabs.ts's module doc for exactly what
     * isolation this does and doesn't provide today. */
    isPrivate?: boolean;
    /** Absolute zoom factor, 1.0 = 100%. Per-tab because that's how
     * every mainstream browser scopes zoom (not global, not per-site). */
    zoom?: number;
    favicon?: string;
}
export interface HistoryEntry { url: string; title: string; time: number; favicon?: string; }
export interface BookmarkItem { url: string; title: string; favicon?: string; }
export interface DownloadItem {
    id: string;
    tabId: string;
    url: string;
    filename: string;
    path: string;
    state: 'downloading' | 'done' | 'error' | 'cancelled';
}

export type SearchEngineId = 'duckduckgo' | 'google' | 'bing' | 'ecosia' | 'startpage';

export const SEARCH_ENGINES: { id: SearchEngineId; name: string; searchUrl: (q: string) => string }[] = [
    { id: 'duckduckgo', name: 'DuckDuckGo', searchUrl: (q) => `https://duckduckgo.com/?q=${encodeURIComponent(q)}` },
    { id: 'google',     name: 'Google',     searchUrl: (q) => `https://www.google.com/search?q=${encodeURIComponent(q)}` },
    { id: 'bing',       name: 'Bing',       searchUrl: (q) => `https://www.bing.com/search?q=${encodeURIComponent(q)}` },
    { id: 'ecosia',     name: 'Ecosia',     searchUrl: (q) => `https://www.ecosia.org/search?q=${encodeURIComponent(q)}` },
    { id: 'startpage',  name: 'Startpage',  searchUrl: (q) => `https://www.startpage.com/sp/search?query=${encodeURIComponent(q)}` },
];

export interface BlueWebSettings {
    searchEngine: SearchEngineId;
    homepage: string;
    defaultZoom: number;
    contentBlockingEnabled: boolean;
    /** User-added blocked domains, on top of the built-in list — see
     * `settings.ts`'s `BUILTIN_BLOCKLIST` for what's blocked by default. */
    customBlockedDomains: string[];
    openLinksFromOtherAppsInNewTab: boolean;
}

export const DEFAULT_WEB_SETTINGS: BlueWebSettings = {
    searchEngine: 'duckduckgo',
    homepage: 'https://duckduckgo.com',
    defaultZoom: 1,
    contentBlockingEnabled: true,
    customBlockedDomains: [],
    openLinksFromOtherAppsInNewTab: true,
};

export const ZOOM_LEVELS = [0.5, 0.67, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3] as const;

export const SPEED_DIALS = [
    { label: 'DuckDuckGo',   url: 'https://duckduckgo.com',                               icon: '🦆' },
    { label: 'Wikipedia',    url: 'https://wikipedia.org',                                icon: '📖' },
    { label: 'YouTube',      url: 'https://youtube.com',                                  icon: '▶️' },
    { label: 'GitHub',       url: 'https://github.com',                                   icon: '🐙' },
    { label: 'Reddit',       url: 'https://reddit.com',                                   icon: '👽' },
    { label: 'Hacker News',  url: 'https://news.ycombinator.com',                         icon: '📰' },
    { label: 'LegendaryOS',  url: 'https://github.com/LegendaryOS-Linux-System/Blue-Environment', icon: '🔵' },
    { label: 'OpenStreetMap',url: 'https://openstreetmap.org',                            icon: '🗺️' },
] as const;

export function normalizeUrl(input: string, searchEngine: SearchEngineId = 'duckduckgo'): string {
    const t = input.trim();
    if (t.startsWith('http://') || t.startsWith('https://')) return t;
    if (t.includes('.') && !t.includes(' ') && !t.startsWith('/')) return 'https://' + t;
    const engine = SEARCH_ENGINES.find((e) => e.id === searchEngine) ?? SEARCH_ENGINES[0];
    return engine.searchUrl(t);
}
