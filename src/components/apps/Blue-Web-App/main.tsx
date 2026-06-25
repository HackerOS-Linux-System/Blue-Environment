import React, { useState, useCallback } from 'react';
import { AppProps } from '../../../types';
import { Plus, X, Globe, ExternalLink } from 'lucide-react';
import { useTabs } from './src/useTabs';
import { useHistory } from './src/useHistory';
import AddressBar from './src/AddressBar';
import SidePanel from './src/SidePanel';
import NewTabPage from './src/NewTabPage';

type Panel = 'bookmarks' | 'history' | 'none';

const BlueWebApp: React.FC<AppProps> = () => {
    const [panel,     setPanel]     = useState<Panel>('none');
    const [lastError, setLastError] = useState<string | null>(null);

    const hist = useHistory();

    const handleNavigate = useCallback((url: string, _tabId: string) => {
        const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();
        hist.addHistory(url, title);
        setLastError(null);
    }, [hist]);

    const tabs = useTabs(handleNavigate);
    const activeTab = tabs.activeTab;
    const isSecure = activeTab.url.startsWith('https://') || activeTab.isNew;

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white select-none">
            {/* Tab bar */}
            <div className="flex items-center h-9 bg-slate-950/70 border-b border-white/5 overflow-x-auto shrink-0">
                {tabs.tabs.map(t => (
                    <div key={t.id} onClick={() => tabs.setActiveId(t.id)}
                        className={`group flex items-center gap-1.5 px-3 h-full shrink-0 cursor-pointer border-r border-white/5 transition-colors max-w-[180px] ${
                            t.id === tabs.activeId ? 'bg-slate-800 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                        }`}>
                        <Globe size={12} className="shrink-0 opacity-60"/>
                        <span className="text-xs truncate flex-1">{t.title || 'New Tab'}</span>
                        <button onClick={e => tabs.closeTab(t.id, e)} className="opacity-0 group-hover:opacity-100 hover:text-red-400 shrink-0 ml-1">
                            <X size={10}/>
                        </button>
                    </div>
                ))}
                <button onClick={tabs.addTab} className="p-2 text-slate-500 hover:text-white shrink-0"><Plus size={14}/></button>
            </div>

            {/* Address bar */}
            <AddressBar
                url={activeTab.url} isNew={activeTab.isNew} isSecure={isSecure}
                isBookmarked={hist.isBookmarked(activeTab.url)}
                canGoBack={hist.canGoBack} canGoForward={hist.canGoForward}
                onBack={() => { const u = hist.goBackNav(); if (u) tabs.openUrl(u); }}
                onForward={() => { const u = hist.goForwardNav(); if (u) tabs.openUrl(u); }}
                onRefresh={() => !activeTab.isNew && tabs.openUrl(activeTab.url)}
                onHome={() => tabs.openUrl('https://duckduckgo.com')}
                onNavigate={url => tabs.openUrl(url)}
                onToggleBookmark={() => hist.toggleBookmark(activeTab.url, activeTab.title)}
                onToggleBookmarks={() => setPanel(p => p === 'bookmarks' ? 'none' : 'bookmarks')}
                onToggleHistory={() => setPanel(p => p === 'history' ? 'none' : 'history')}
                panelOpen={panel}
            />

            {/* Main */}
            <div className="flex-1 overflow-hidden relative">
                {activeTab.isNew ? (
                    <NewTabPage onNavigate={url => tabs.openUrl(url)} error={lastError}/>
                ) : (
                    <div className="flex-1 flex flex-col items-center justify-center gap-4 text-center px-8 h-full">
                        <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-blue-600 to-indigo-700 flex items-center justify-center shadow-lg shadow-blue-500/20 mx-auto">
                            <ExternalLink size={26} className="text-white"/>
                        </div>
                        <div>
                            <p className="text-white font-semibold mb-1">{activeTab.title}</p>
                            <p className="text-slate-400 text-xs mb-4 font-mono break-all max-w-sm">{activeTab.url}</p>
                            <p className="text-slate-500 text-sm max-w-sm mx-auto">
                                This site is open in a native webview window. Switch to it using the taskbar or Alt+Tab.
                            </p>
                        </div>
                        <div className="flex gap-2">
                            <button onClick={() => tabs.openUrl(activeTab.url)}
                                className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">
                                <ExternalLink size={14}/> Re-open
                            </button>
                            <button onClick={tabs.addTab}
                                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded-xl text-sm">
                                New Tab
                            </button>
                        </div>
                    </div>
                )}

                <SidePanel
                    panel={panel} bookmarks={hist.bookmarks} history={hist.history}
                    onClose={() => setPanel('none')}
                    onNavigate={url => tabs.openUrl(url)}
                    onClearHistory={hist.clearHistory}
                />
            </div>
        </div>
    );
};

export default BlueWebApp;
