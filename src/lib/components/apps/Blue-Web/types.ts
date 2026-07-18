export interface Tab { id: string; url: string; title: string; isNew: boolean; }
export interface HistoryEntry { url: string; title: string; time: number; }
export interface BookmarkItem { url: string; title: string; favicon?: string; }
export interface DownloadItem { url: string; filename: string; state: 'downloading' | 'done' | 'error'; progress: number; }

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

export function normalizeUrl(input: string): string {
    const t = input.trim();
    if (t.startsWith('http://') || t.startsWith('https://')) return t;
    if (t.includes('.') && !t.includes(' ') && !t.startsWith('/')) return 'https://' + t;
    return `https://duckduckgo.com/?q=${encodeURIComponent(t)}`;
}
