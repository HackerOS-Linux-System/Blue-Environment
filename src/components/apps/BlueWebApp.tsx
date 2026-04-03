import React, { useState, useEffect, useRef, useCallback } from 'react';
import { AppProps } from '../../types';
import { useLanguage } from '../../contexts/LanguageContext';
import { SystemBridge } from '../../utils/systemBridge';
import {
    ArrowLeft, ArrowRight, RotateCw, Home, Search, Lock, Globe,
    Plus, X, Settings, ChevronDown, Check
} from 'lucide-react';

interface Tab {
    id: string;
    url: string;
    title: string;
    favicon?: string;
    isLoading: boolean;
}

const DEFAULT_HOMEPAGE = 'https://hackeros-linux-system.github.io/HackerOS-Search-Engine/';

// Lista dostępnych silników wyszukiwania
const SEARCH_ENGINES = {
    google: {
        name: 'Google',
        url: 'https://www.google.com/search?q=',
        icon: 'https://www.google.com/favicon.ico',
    },
    duckduckgo: {
        name: 'DuckDuckGo',
        url: 'https://duckduckgo.com/?q=',
        icon: 'https://duckduckgo.com/favicon.ico',
    },
    bing: {
        name: 'Bing',
        url: 'https://www.bing.com/search?q=',
        icon: 'https://www.bing.com/favicon.ico',
    },
    yandex: {
        name: 'Yandex',
        url: 'https://yandex.com/search/?text=',
        icon: 'https://yandex.com/favicon.ico',
    },
    hackeros: {
        name: 'HackerOS',
        url: 'https://hackeros-linux-system.github.io/HackerOS-Search-Engine/search.html?q=',
        icon: 'https://hackeros-linux-system.github.io/HackerOS-Search-Engine/favicon.ico',
    },
};

type SearchEngineId = keyof typeof SEARCH_ENGINES;

const BlueWebApp: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [tabs, setTabs] = useState<Tab[]>([
        {
            id: '1',
            url: DEFAULT_HOMEPAGE,
            title: 'HackerOS Search',
            isLoading: false,
        },
    ]);
    const [activeTabId, setActiveTabId] = useState('1');
    const [inputUrl, setInputUrl] = useState(DEFAULT_HOMEPAGE);
    const [showSettings, setShowSettings] = useState(false);
    const [searchEngine, setSearchEngine] = useState<SearchEngineId>('hackeros');

    // Wczytaj zapisany silnik z localStorage
    useEffect(() => {
        const saved = localStorage.getItem('blueweb_search_engine');
        if (saved && SEARCH_ENGINES[saved as SearchEngineId]) {
            setSearchEngine(saved as SearchEngineId);
        }
    }, []);

    // Zapisz silnik
    const saveSearchEngine = (engine: SearchEngineId) => {
        setSearchEngine(engine);
        localStorage.setItem('blueweb_search_engine', engine);
        setShowSettings(false);
    };

    const activeTab = tabs.find(t => t.id === activeTabId) || tabs[0];

    // Nawigacja
    const navigateTo = (tabId: string, newUrl: string) => {
        setTabs(prev => prev.map(tab =>
        tab.id === tabId ? { ...tab, url: newUrl, isLoading: true, title: newUrl } : tab
        ));
        if (tabId === activeTabId) setInputUrl(newUrl);
    };

        const handleNavigate = (e?: React.FormEvent) => {
            e?.preventDefault();
            let target = inputUrl.trim();
            if (!target) return;

            // Sprawdź, czy to wyszukiwanie (nie wygląda jak URL)
            const isUrl = target.includes('.') && !target.includes(' ') && !target.startsWith('search:');
            if (!isUrl) {
                const engine = SEARCH_ENGINES[searchEngine];
                target = engine.url + encodeURIComponent(target);
            } else {
                if (!target.startsWith('http')) target = 'https://' + target;
            }
            navigateTo(activeTabId, target);
        };

        const goBack = () => {
            const iframe = document.getElementById(`iframe-${activeTabId}`) as HTMLIFrameElement | null;
            if (iframe && iframe.contentWindow) {
                try {
                    iframe.contentWindow.history.back();
                } catch (e) {
                    console.warn('Cannot go back', e);
                }
            }
        };

        const goForward = () => {
            const iframe = document.getElementById(`iframe-${activeTabId}`) as HTMLIFrameElement | null;
            if (iframe && iframe.contentWindow) {
                try {
                    iframe.contentWindow.history.forward();
                } catch (e) {
                    console.warn('Cannot go forward', e);
                }
            }
        };

        const refresh = () => {
            const iframe = document.getElementById(`iframe-${activeTabId}`) as HTMLIFrameElement | null;
            if (iframe && iframe.contentWindow) {
                try {
                    iframe.contentWindow.location.reload();
                } catch (e) {
                    navigateTo(activeTabId, activeTab.url);
                }
            } else {
                navigateTo(activeTabId, activeTab.url);
            }
        };

        const goHome = () => {
            navigateTo(activeTabId, DEFAULT_HOMEPAGE);
        };

        // Karty
        const addTab = () => {
            const newId = Date.now().toString();
            setTabs(prev => [
                ...prev,
                {
                    id: newId,
                    url: DEFAULT_HOMEPAGE,
                    title: 'New Tab',
                    isLoading: false,
                },
            ]);
            setActiveTabId(newId);
            setInputUrl(DEFAULT_HOMEPAGE);
        };

        const closeTab = (tabId: string, e?: React.MouseEvent) => {
            e?.stopPropagation();
            if (tabs.length === 1) return;
            setTabs(prev => prev.filter(t => t.id !== tabId));
            if (activeTabId === tabId) {
                const newActive = tabs.find(t => t.id !== tabId) || tabs[0];
                setActiveTabId(newActive.id);
                setInputUrl(newActive.url);
            }
        };

        const switchTab = (tabId: string) => {
            setActiveTabId(tabId);
            const tab = tabs.find(t => t.id === tabId);
            if (tab) setInputUrl(tab.url);
        };

            // Obsługa zdarzeń iframe
            const handleIframeLoad = (tabId: string) => {
                setTabs(prev => prev.map(tab =>
                tab.id === tabId ? { ...tab, isLoading: false } : tab
                ));
                // Próba odczytania tytułu
                const iframe = document.getElementById(`iframe-${tabId}`) as HTMLIFrameElement | null;
                if (iframe && iframe.contentWindow && iframe.contentDocument) {
                    try {
                        const title = iframe.contentDocument.title;
                        if (title) {
                            setTabs(prev => prev.map(tab =>
                            tab.id === tabId ? { ...tab, title } : tab
                            ));
                        }
                    } catch (e) {
                        // CORS – ignorujemy
                    }
                }
            };

            // Skróty klawiszowe
            useEffect(() => {
                const handleKeyDown = (e: KeyboardEvent) => {
                    const ctrl = e.ctrlKey || e.metaKey;
                    if (ctrl && e.key === 't') {
                        e.preventDefault();
                        addTab();
                    } else if (ctrl && e.key === 'w') {
                        e.preventDefault();
                        closeTab(activeTabId);
                    } else if (ctrl && e.key === 'Tab') {
                        e.preventDefault();
                        const currentIndex = tabs.findIndex(t => t.id === activeTabId);
                        const nextIndex = e.shiftKey
                        ? (currentIndex - 1 + tabs.length) % tabs.length
                        : (currentIndex + 1) % tabs.length;
                        switchTab(tabs[nextIndex].id);
                    }
                };
                window.addEventListener('keydown', handleKeyDown);
                return () => window.removeEventListener('keydown', handleKeyDown);
            }, [tabs, activeTabId]);

            return (
                <div className="flex flex-col h-full bg-slate-900 text-white">
                {/* Pasek kart */}
                <div className="flex items-center bg-slate-800 border-b border-white/5 px-2 overflow-x-auto">
                {tabs.map(tab => (
                    <div
                    key={tab.id}
                    onClick={() => switchTab(tab.id)}
                    className={`flex items-center gap-2 px-3 py-2 min-w-[120px] max-w-[200px] rounded-t-lg cursor-pointer transition-colors ${
                        tab.id === activeTabId
                        ? 'bg-slate-900 text-white border-t-2 border-blue-500'
                        : 'bg-slate-800/50 text-slate-400 hover:bg-slate-700'
                    }`}
                    >
                    {tab.isLoading ? (
                        <div className="w-4 h-4 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
                    ) : (
                        tab.favicon ? (
                            <img src={tab.favicon} className="w-4 h-4" alt="" />
                        ) : (
                            <Globe size={14} />
                        )
                    )}
                    <span className="truncate text-sm flex-1">{tab.title}</span>
                    <button
                    onClick={(e) => closeTab(tab.id, e)}
                    className="p-1 hover:bg-white/10 rounded-full"
                    >
                    <X size={12} />
                    </button>
                    </div>
                ))}
                <button
                onClick={addTab}
                className="p-2 hover:bg-white/10 rounded-full ml-1 shrink-0"
                title="New Tab (Ctrl+T)"
                >
                <Plus size={16} />
                </button>
                </div>

                {/* Pasek adresu */}
                <div className="flex items-center gap-2 p-2 bg-slate-800 border-b border-white/5 shadow-sm z-10">
                <div className="flex items-center gap-1">
                <button onClick={goBack} className="p-2 hover:bg-white/10 rounded-full text-slate-400">
                <ArrowLeft size={16} />
                </button>
                <button onClick={goForward} className="p-2 hover:bg-white/10 rounded-full text-slate-400">
                <ArrowRight size={16} />
                </button>
                <button onClick={refresh} className="p-2 hover:bg-white/10 rounded-full text-slate-400">
                <RotateCw size={16} />
                </button>
                <button onClick={goHome} className="p-2 hover:bg-white/10 rounded-full text-slate-400">
                <Home size={16} />
                </button>
                </div>

                <form onSubmit={handleNavigate} className="flex-1 flex items-center bg-slate-900 rounded-full px-4 py-2 border border-white/10 focus-within:border-blue-500/50 focus-within:ring-2 focus-within:ring-blue-500/20">
                {activeTab?.url.startsWith('https') ? <Lock size={12} className="text-green-400 mr-2" /> : <Globe size={12} className="text-slate-500 mr-2" />}
                <input
                className="flex-1 bg-transparent text-sm text-white focus:outline-none placeholder-slate-600"
                value={inputUrl}
                onChange={e => setInputUrl(e.target.value)}
                placeholder="Search or enter URL"
                />
                </form>

                <button
                onClick={() => setShowSettings(!showSettings)}
                className={`p-2 rounded-full hover:bg-white/10 transition-colors ${showSettings ? 'bg-blue-600/20 text-blue-400' : 'text-slate-400'}`}
                >
                <Settings size={18} />
                </button>
                </div>

                {/* Ustawienia silnika wyszukiwania */}
                {showSettings && (
                    <div className="absolute top-20 right-4 w-64 bg-slate-800 border border-white/10 rounded-xl shadow-2xl z-50 p-4 backdrop-blur-sm">
                    <h3 className="font-semibold text-white mb-3">Search Engine</h3>
                    <div className="space-y-2">
                    {Object.entries(SEARCH_ENGINES).map(([id, engine]) => (
                        <button
                        key={id}
                        onClick={() => saveSearchEngine(id as SearchEngineId)}
                        className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors ${
                            searchEngine === id
                            ? 'bg-blue-600/20 text-blue-400'
                            : 'hover:bg-white/10 text-slate-300'
                        }`}
                        >
                        <div className="flex items-center gap-2">
                        <img src={engine.icon} className="w-4 h-4" alt="" />
                        <span>{engine.name}</span>
                        </div>
                        {searchEngine === id && <Check size={14} />}
                        </button>
                    ))}
                    </div>
                    </div>
                )}

                {/* Zawartość iframe – bez sandbox, aby strony działały normalnie */}
                <div className="flex-1 relative bg-white">
                {tabs.map(tab => (
                    <iframe
                    key={tab.id}
                    id={`iframe-${tab.id}`}
                    src={tab.id === activeTabId ? tab.url : 'about:blank'}
                    className="w-full h-full border-none"
                    title="browser"
                    referrerPolicy="strict-origin-when-cross-origin"
                    onLoad={() => handleIframeLoad(tab.id)}
                    style={{ display: tab.id === activeTabId ? 'block' : 'none' }}
                    />
                ))}
                </div>

                {/* Status bar */}
                <div className="h-6 bg-slate-800 border-t border-white/5 flex items-center px-3 text-[10px] text-slate-500">
                <span className="mr-auto">{activeTab?.url}</span>
                <span className="flex items-center gap-1">
                <span>🔒</span> <span>Blue Web Engine 2.0</span>
                </span>
                </div>
                </div>
            );
};

export default BlueWebApp;
