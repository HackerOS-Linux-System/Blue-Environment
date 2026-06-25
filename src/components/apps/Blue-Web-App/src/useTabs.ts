import { useState, useRef, useCallback } from 'react';
import { Tab, normalizeUrl } from './types';
import { SystemBridge } from '../../../../utils/systemBridge';

function makeTab(url = ''): Tab {
    const id = `t${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const title = url ? (() => { try { return new URL(url).hostname; } catch { return url; } })() : 'New Tab';
    return { id, url, title, isNew: !url };
}

export function useTabs(onNavigate: (url: string, tabId: string) => void) {
    const [tabs, setTabs] = useState<Tab[]>([makeTab()]);
    const [activeId, setActiveId] = useState(tabs[0].id);

    const activeTab = tabs.find(t => t.id === activeId) ?? tabs[0];

    const openUrl = useCallback(async (rawUrl: string, tabId?: string) => {
        const url = normalizeUrl(rawUrl);
        const id = tabId ?? activeId;
        const title = (() => { try { return new URL(url).hostname; } catch { return url; } })();

        setTabs(prev => prev.map(t => t.id === id ? { ...t, url, title, isNew: false } : t));
        onNavigate(url, id);

        if (SystemBridge.isTauri()) {
            try { await SystemBridge.invoke('web_open_native', { url }); } catch {}
        } else {
            window.open(url, '_blank', 'noopener');
        }
    }, [activeId, onNavigate]);

    const addTab = useCallback(() => {
        const t = makeTab();
        setTabs(prev => [...prev, t]);
        setActiveId(t.id);
        return t.id;
    }, []);

    const closeTab = useCallback((id: string, e?: React.MouseEvent) => {
        e?.stopPropagation();
        setTabs(prev => {
            if (prev.length === 1) return [makeTab()];
            const next = prev.filter(t => t.id !== id);
            if (activeId === id) setActiveId(next[next.length - 1].id);
            return next;
        });
    }, [activeId]);

    return { tabs, activeId, setActiveId, activeTab, openUrl, addTab, closeTab };
}
