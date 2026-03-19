import React, { useState, useRef } from 'react';
import { ArrowLeft, ArrowRight, RotateCw, Home, Search, Lock, Globe, Plus, X } from 'lucide-react';
import { AppProps } from '../../types';

interface Tab {
    id: string;
    url: string;
    title: string;
    isLoading: boolean;
}

const BlueWebApp: React.FC<AppProps> = () => {
    const [tabs, setTabs] = useState<Tab[]>([
        { id: '1', url: 'https://www.google.com/webhp?igu=1', title: 'Google', isLoading: false }
    ]);
    const [activeTabId, setActiveTabId] = useState('1');
    const [inputUrl, setInputUrl] = useState('https://www.google.com/webhp?igu=1');
    const iframeRefs = useRef<Map<string, HTMLIFrameElement>>(new Map());

    const activeTab = tabs.find(t => t.id === activeTabId) || tabs[0];

    const loadUrl = (tabId: string, newUrl: string) => {
        setTabs(prev => prev.map(tab =>
        tab.id === tabId ? { ...tab, url: newUrl, title: newUrl, isLoading: true } : tab
        ));
        if (tabId === activeTabId) {
            setInputUrl(newUrl);
        }
        // Symulacja załadowania – w rzeczywistości iframe sam ładuje
    };

    const handleNavigate = (e: React.FormEvent) => {
        e.preventDefault();
        let target = inputUrl;
        if (!target.startsWith('http')) {
            if (target.includes('.') && !target.includes(' ')) {
                target = 'https://' + target;
            } else {
                target = `https://www.google.com/search?q=${encodeURIComponent(target)}&igu=1`;
            }
        }
        loadUrl(activeTabId, target);
    };

    const goBack = () => {
        // Można próbować przez iframe, ale CORS blokuje – zostawiamy placeholder
        console.log('Back');
    };

    const goForward = () => {
        console.log('Forward');
    };

    const refresh = () => {
        if (activeTab) {
            loadUrl(activeTab.id, activeTab.url);
        }
    };

    const addTab = () => {
        const newId = Date.now().toString();
        setTabs([...tabs, { id: newId, url: 'https://www.google.com/webhp?igu=1', title: 'Nowa karta', isLoading: false }]);
        setActiveTabId(newId);
        setInputUrl('https://www.google.com/webhp?igu=1');
    };

    const closeTab = (tabId: string, e: React.MouseEvent) => {
        e.stopPropagation();
        if (tabs.length === 1) return;
        const newTabs = tabs.filter(t => t.id !== tabId);
        setTabs(newTabs);
        if (activeTabId === tabId) {
            setActiveTabId(newTabs[0].id);
            setInputUrl(newTabs[0].url);
        }
    };

    const switchTab = (tabId: string) => {
        setActiveTabId(tabId);
        const tab = tabs.find(t => t.id === tabId);
        if (tab) setInputUrl(tab.url);
    };

        return (
            <div className="flex flex-col h-full bg-slate-900">
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
                    <Globe size={14} />
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
            title="Nowa karta"
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
            </div>

            <form onSubmit={handleNavigate} className="flex-1 flex items-center bg-slate-900 rounded-full px-4 py-2 border border-white/10 focus-within:border-blue-500/50 focus-within:ring-2 focus-within:ring-blue-500/20">
            {activeTab?.url.startsWith('https') ? <Lock size={12} className="text-green-400 mr-2" /> : <Globe size={12} className="text-slate-500 mr-2" />}
            <input
            className="flex-1 bg-transparent text-sm text-white focus:outline-none placeholder-slate-600"
            value={inputUrl}
            onChange={e => setInputUrl(e.target.value)}
            placeholder="Search Google or enter a URL"
            />
            </form>

            <button className="p-2 hover:bg-white/10 rounded-full text-slate-400" onClick={() => loadUrl(activeTabId, 'https://www.google.com/webhp?igu=1')}>
            <Home size={18} />
            </button>
            </div>

            {/* Zawartość iframe */}
            <div className="flex-1 relative bg-white">
            {activeTab && (
                <iframe
                key={activeTab.id}
                src={activeTab.url}
                className="w-full h-full border-none"
                title="browser"
                sandbox="allow-forms allow-scripts allow-same-origin allow-popups allow-modals allow-presentation"
                onLoad={() => {
                    setTabs(prev => prev.map(tab =>
                    tab.id === activeTab.id ? { ...tab, isLoading: false } : tab
                    ));
                }}
                />
            )}
            </div>

            {/* Status bar */}
            <div className="h-6 bg-slate-800 border-t border-white/5 flex items-center px-3 text-[10px] text-slate-500">
            <span className="mr-auto">{activeTab?.url}</span>
            <span>Blue Web Engine 1.0</span>
            </div>
            </div>
        );
};

export default BlueWebApp;
