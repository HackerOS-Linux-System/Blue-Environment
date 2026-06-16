import React, { useState, useRef, useEffect, useCallback } from 'react';
import { AppProps } from '../../../types';
import {
    Globe, ArrowLeft, ArrowRight, RefreshCw, X, Lock, Unlock,
    Home, Search, Plus, Star, StarOff, Download, BookOpen,
    Shield, AlertTriangle, ExternalLink, Bookmark, History,
} from 'lucide-react';

const PROXY_PREFIXES = [
    'https://corsproxy.io/?',
'https://api.allorigins.win/raw?url=',
];

// Popular sites that block iframes — open externally
const IFRAME_BLOCKED = [
    'google.com', 'youtube.com', 'facebook.com', 'twitter.com', 'x.com',
'instagram.com', 'reddit.com', 'netflix.com', 'amazon.com', 'linkedin.com',
'github.com', 'stackoverflow.com', 'wikipedia.org',
];

function isIframeBlocked(url: string): boolean {
    try {
        const host = new URL(url).hostname.replace('www.', '');
        return IFRAME_BLOCKED.some(d => host === d || host.endsWith('.' + d));
    } catch { return false; }
}

function normalizeUrl(input: string): string {
    const t = input.trim();
    if (t.startsWith('http://') || t.startsWith('https://')) return t;
        if (t.includes('.') && !t.includes(' ')) return 'https://' + t;
            return `https://duckduckgo.com/?q=${encodeURIComponent(t)}`;
}

interface Tab {
    id: string;
    url: string;
    title: string;
    loading: boolean;
    blocked: boolean;
}

interface HistoryEntry { url: string; title: string; time: number; }
interface Bookmark2 { url: string; title: string; }

const BlueWebApp: React.FC<AppProps> = () => {
    const [tabs, setTabs] = useState<Tab[]>([{
        id: 'tab-1', url: 'about:blank', title: 'New Tab', loading: false, blocked: false
    }]);
    const [activeId, setActiveId] = useState('tab-1');
    const [inputUrl, setInputUrl] = useState('');
    const [showBookmarks, setShowBookmarks] = useState(false);
    const [showHistory, setShowHistory] = useState(false);
    const [bookmarks, setBookmarks] = useState<Bookmark2[]>(() => {
        try { return JSON.parse(localStorage.getItem('bw_bookmarks') ?? '[]'); } catch { return []; }
    });
    const [historyLog, setHistoryLog] = useState<HistoryEntry[]>(() => {
        try { return JSON.parse(localStorage.getItem('bw_history') ?? '[]'); } catch { return []; }
    });

    const iframeRef = useRef<HTMLIFrameElement>(null);
    const inputRef  = useRef<HTMLInputElement>(null);

    const activeTab = tabs.find(t => t.id === activeId) ?? tabs[0];
    const isSecure  = activeTab.url.startsWith('https://') || activeTab.url === 'about:blank';
    const isBookmarked = bookmarks.some(b => b.url === activeTab.url);

    const updateTab = useCallback((id: string, patch: Partial<Tab>) => {
        setTabs(prev => prev.map(t => t.id === id ? { ...t, ...patch } : t));
    }, []);

    const navigate = useCallback((rawUrl: string, tabId?: string) => {
        const url  = normalizeUrl(rawUrl);
        const tid  = tabId ?? activeId;
        const blocked = isIframeBlocked(url);

        updateTab(tid, { url, loading: !blocked, blocked, title: new URL(url).hostname });
        setInputUrl(url);

        // Log to history
        const entry = { url, title: new URL(url).hostname, time: Date.now() };
        setHistoryLog(prev => {
            const next = [entry, ...prev.filter(h => h.url !== url)].slice(0, 100);
            localStorage.setItem('bw_history', JSON.stringify(next));
            return next;
        });
    }, [activeId, updateTab]);

    // Sync input when switching tabs
    useEffect(() => {
        setInputUrl(activeTab.url === 'about:blank' ? '' : activeTab.url);
    }, [activeId, activeTab.url]);

    const addTab = (url = 'about:blank') => {
        const id = `tab-${Date.now()}`;
        setTabs(prev => [...prev, { id, url, title: 'New Tab', loading: false, blocked: false }]);
        setActiveId(id);
        setInputUrl(url === 'about:blank' ? '' : url);
        if (url !== 'about:blank') navigate(url, id);
        setTimeout(() => inputRef.current?.focus(), 100);
    };

    const closeTab = (id: string) => {
        if (tabs.length === 1) { addTab(); return; }
        const next = tabs.filter(t => t.id !== id);
        setTabs(next);
        if (activeId === id) setActiveId(next[next.length - 1].id);
    };

        const toggleBookmark = () => {
            if (isBookmarked) {
                const next = bookmarks.filter(b => b.url !== activeTab.url);
                setBookmarks(next);
                localStorage.setItem('bw_bookmarks', JSON.stringify(next));
            } else {
                const next = [{ url: activeTab.url, title: activeTab.title }, ...bookmarks];
                setBookmarks(next);
                localStorage.setItem('bw_bookmarks', JSON.stringify(next));
            }
        };

        const handleKeyDown = (e: React.KeyboardEvent) => {
            if (e.key === 'Enter') { navigate(inputUrl); inputRef.current?.blur(); }
            if (e.key === 'Escape') { setInputUrl(activeTab.url); inputRef.current?.blur(); }
        };

        const refresh = () => {
            if (iframeRef.current) iframeRef.current.src = activeTab.url;
            updateTab(activeId, { loading: true });
        };

        const goBack = () => iframeRef.current?.contentWindow?.history.back();
        const goFwd  = () => iframeRef.current?.contentWindow?.history.forward();

        // New tab page content
        const NewTabPage = () => (
            <div className="flex flex-col items-center justify-center h-full bg-slate-900 text-white gap-8">
            <div className="flex items-center gap-3 mb-2">
            <Globe size={36} className="text-blue-400"/>
            <span className="text-2xl font-light text-slate-300">Blue Web</span>
            </div>
            <div className="relative w-96">
            <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500"/>
            <input
            type="text"
            placeholder="Search or enter URL…"
            className="w-full bg-slate-800 border border-white/10 rounded-2xl py-3 pl-10 pr-4
            text-white focus:outline-none focus:border-blue-500/60 text-sm"
            onKeyDown={e => { if (e.key === 'Enter') navigate((e.target as HTMLInputElement).value); }}
            autoFocus
            />
            </div>
            {/* Quick links */}
            <div className="grid grid-cols-4 gap-4 w-96">
            {[
                { label: 'DuckDuckGo', url: 'https://duckduckgo.com', icon: '🦆' },
                { label: 'Wikipedia',  url: 'https://en.m.wikipedia.org', icon: '📖' },
                { label: 'GitHub',     url: 'https://github.com', icon: '🐙' },
                { label: 'Weather',    url: 'https://wttr.in', icon: '🌤' },
            ].map(q => (
                <button key={q.url} onClick={() => navigate(q.url)}
                className="flex flex-col items-center gap-2 p-4 bg-slate-800/50 hover:bg-slate-700/50
                rounded-2xl border border-white/5 transition-colors">
                <span className="text-2xl">{q.icon}</span>
                <span className="text-xs text-slate-400">{q.label}</span>
                </button>
            ))}
            </div>
            {bookmarks.length > 0 && (
                <div className="w-96">
                <div className="text-xs text-slate-500 mb-2 uppercase tracking-wider">Bookmarks</div>
                <div className="space-y-1">
                {bookmarks.slice(0, 5).map(b => (
                    <button key={b.url} onClick={() => navigate(b.url)}
                    className="w-full flex items-center gap-2 px-3 py-2 bg-slate-800/30 hover:bg-slate-700/50
                    rounded-xl text-sm text-left text-slate-300 transition-colors">
                    <Globe size={13} className="text-slate-500 shrink-0"/>
                    <span className="truncate">{b.title}</span>
                    <span className="text-xs text-slate-600 truncate ml-auto">{b.url.slice(0, 30)}</span>
                    </button>
                ))}
                </div>
                </div>
            )}
            </div>
        );

        // Blocked site page
        const BlockedPage = () => (
            <div className="flex flex-col items-center justify-center h-full bg-slate-900 text-white gap-6">
            <div className="w-16 h-16 bg-yellow-500/10 rounded-2xl flex items-center justify-center">
            <AlertTriangle size={32} className="text-yellow-400"/>
            </div>
            <div className="text-center max-w-xs">
            <div className="text-lg font-medium mb-2">Site blocks embedding</div>
            <div className="text-slate-400 text-sm mb-1">
            <strong>{activeTab.title}</strong> uses X-Frame-Options to prevent display in iframes.
            </div>
            <div className="text-slate-500 text-xs">{activeTab.url}</div>
            </div>
            <div className="flex gap-3">
            <button onClick={() => window.open(activeTab.url, '_blank')}
            className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">
            <ExternalLink size={14}/> Open in system browser
            </button>
            <button onClick={() => navigate('about:blank')}
            className="flex items-center gap-2 px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded-xl text-sm">
            New tab
            </button>
            </div>
            </div>
        );

        return (
            <div className="flex flex-col h-full bg-slate-900 text-white">
            {/* Tab bar */}
            <div className="flex items-center bg-slate-800 border-b border-white/5 overflow-x-auto shrink-0">
            {tabs.map(tab => (
                <div key={tab.id}
                onClick={() => setActiveId(tab.id)}
                className={`flex items-center gap-2 px-3 py-2 cursor-pointer border-r border-white/5
                    shrink-0 min-w-0 group max-w-[180px]
                    ${tab.id === activeId ? 'bg-slate-900 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}`}>
                    {tab.loading
                        ? <div className="w-3 h-3 border border-blue-400 border-t-transparent rounded-full animate-spin shrink-0"/>
                        : tab.blocked
                        ? <Shield size={13} className="text-yellow-400 shrink-0"/>
                        : <Globe size={13} className="shrink-0"/>
                    }
                    <span className="text-xs truncate flex-1">{tab.title}</span>
                    <button onClick={e => { e.stopPropagation(); closeTab(tab.id); }}
                    className="opacity-0 group-hover:opacity-100 hover:text-red-400 shrink-0">
                    <X size={10}/>
                    </button>
                    </div>
            ))}
            <button onClick={() => addTab()} className="p-2.5 hover:bg-slate-700/50 text-slate-500 hover:text-white shrink-0">
            <Plus size={14}/>
            </button>
            </div>

            {/* Toolbar */}
            <div className="h-11 bg-slate-800/80 border-b border-white/5 flex items-center px-2 gap-1.5 shrink-0">
            <button onClick={goBack}    className="p-1.5 hover:bg-white/10 rounded-lg"><ArrowLeft  size={16}/></button>
            <button onClick={goFwd}     className="p-1.5 hover:bg-white/10 rounded-lg"><ArrowRight size={16}/></button>
            <button onClick={refresh}   className="p-1.5 hover:bg-white/10 rounded-lg">
            <RefreshCw size={16} className={activeTab.loading ? 'animate-spin text-blue-400' : ''}/>
            </button>
            <button onClick={() => navigate('https://start.duckduckgo.com/')}
            className="p-1.5 hover:bg-white/10 rounded-lg"><Home size={16}/></button>

            {/* URL bar */}
            <div className="flex-1 flex items-center gap-2 bg-slate-900 border border-white/10 hover:border-white/20
            rounded-xl px-3 py-1.5 mx-1 transition-colors focus-within:border-blue-500/50">
            {isSecure
                ? <Lock size={12} className="text-green-400 shrink-0"/>
                : <Unlock size={12} className="text-yellow-400 shrink-0"/>
            }
            <input
            ref={inputRef}
            type="text"
            value={inputUrl}
            onChange={e => setInputUrl(e.target.value)}
            onKeyDown={handleKeyDown}
            onFocus={e => e.target.select()}
            className="flex-1 bg-transparent text-sm text-white focus:outline-none placeholder-slate-500"
            placeholder="Search or enter URL…"
            />
            {inputUrl && (
                <button onClick={() => { setInputUrl(''); inputRef.current?.focus(); }}
                className="text-slate-500 hover:text-white shrink-0">
                <X size={11}/>
                </button>
            )}
            </div>

            {/* Action buttons */}
            {activeTab.url !== 'about:blank' && (
                <button onClick={toggleBookmark} title={isBookmarked ? 'Remove bookmark' : 'Bookmark'}
                className="p-1.5 hover:bg-white/10 rounded-lg">
                {isBookmarked
                    ? <Star size={15} className="text-yellow-400 fill-yellow-400"/>
                    : <StarOff size={15} className="text-slate-400"/>
                }
                </button>
            )}
            <button onClick={() => { setShowBookmarks(b => !b); setShowHistory(false); }}
            className={`p-1.5 rounded-lg ${showBookmarks ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-400'}`}
            title="Bookmarks">
            <Bookmark size={15}/>
            </button>
            <button onClick={() => { setShowHistory(h => !h); setShowBookmarks(false); }}
            className={`p-1.5 rounded-lg ${showHistory ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-400'}`}
            title="History">
            <History size={15}/>
            </button>
            </div>

            {/* Bookmark / History panel */}
            {(showBookmarks || showHistory) && (
                <div className="border-b border-white/5 bg-slate-800/50 max-h-56 overflow-y-auto">
                <div className="p-3">
                <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
                {showBookmarks ? 'Bookmarks' : 'History'}
                </span>
                {showHistory && historyLog.length > 0 && (
                    <button onClick={() => {
                        setHistoryLog([]);
                        localStorage.removeItem('bw_history');
                    }} className="text-xs text-red-400 hover:text-red-300">Clear</button>
                )}
                </div>
                <div className="space-y-0.5">
                {(showBookmarks ? bookmarks : historyLog).map((item, i) => (
                    <div key={i} className="flex items-center gap-2 group">
                    <button onClick={() => { navigate(item.url); setShowBookmarks(false); setShowHistory(false); }}
                    className="flex-1 flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/5 text-left text-sm text-slate-300 min-w-0">
                    <Globe size={12} className="text-slate-500 shrink-0"/>
                    <span className="truncate">{'title' in item ? item.title : (item as any).title}</span>
                    <span className="text-xs text-slate-600 truncate ml-auto">{item.url.slice(8, 40)}</span>
                    </button>
                    {showBookmarks && (
                        <button onClick={() => {
                            const next = bookmarks.filter(b => b.url !== item.url);
                            setBookmarks(next);
                            localStorage.setItem('bw_bookmarks', JSON.stringify(next));
                        }} className="opacity-0 group-hover:opacity-100 text-red-400 hover:text-red-300 shrink-0">
                        <X size={12}/>
                        </button>
                    )}
                    </div>
                ))}
                {((showBookmarks && bookmarks.length === 0) || (showHistory && historyLog.length === 0)) && (
                    <div className="text-xs text-slate-600 py-2 text-center">Empty</div>
                )}
                </div>
                </div>
                </div>
            )}

            {/* Content */}
            <div className="flex-1 relative overflow-hidden">
            {activeTab.url === 'about:blank' ? (
                <NewTabPage/>
            ) : activeTab.blocked ? (
                <BlockedPage/>
            ) : (
                <>
                <iframe
                ref={iframeRef}
                src={activeTab.url}
                className="w-full h-full border-none bg-white"
                sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-top-navigation"
                onLoad={() => {
                    updateTab(activeId, { loading: false });
                    try {
                        const t = iframeRef.current?.contentDocument?.title;
                        if (t) updateTab(activeId, { title: t });
                    } catch {}
                }}
                onError={() => updateTab(activeId, { loading: false, blocked: true })}
                title="Blue Web Browser"
                />
                {activeTab.loading && (
                    <div className="absolute top-0 left-0 right-0 h-0.5 bg-slate-800">
                    <div className="h-full bg-blue-500 rounded-full animate-pulse" style={{ width: '60%' }}/>
                    </div>
                )}
                </>
            )}
            </div>
            </div>
        );
};

export default BlueWebApp;
