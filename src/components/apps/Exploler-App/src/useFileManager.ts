import { useState, useCallback, useEffect } from 'react';
import { SystemBridge } from '../../../../utils/systemBridge';
import { useDialog } from '../../../../contexts/DialogContext';
import { FileEntry, Tab, Notif, SortKey } from './types';

function makeTab(path: string): Tab {
    return { id: Date.now().toString(), path, history: [path], historyIndex: 0 };
}

export function useFileManager() {
    const dialog = useDialog();
    const [tabs,       setTabs]       = useState<Tab[]>([makeTab('HOME')]);
    const [activeTab,  setActiveTab]  = useState(0);
    const [files,      setFiles]      = useState<FileEntry[]>([]);
    const [loading,    setLoading]    = useState(false);
    const [selected,   setSelected]   = useState<Set<string>>(new Set());
    const [clipboard,  setClipboard]  = useState<{ files: FileEntry[]; op: 'copy'|'cut' } | null>(null);
    const [sortKey,    setSortKey]    = useState<SortKey>('name');
    const [sortAsc,    setSortAsc]    = useState(true);
    const [search,     setSearch]     = useState('');
    const [notifs,     setNotifs]     = useState<Notif[]>([]);

    const tab = tabs[activeTab];

    const notify = useCallback((type: Notif['type'], message: string) => {
        const id = Date.now().toString();
        setNotifs(n => [...n, { id, type, message }]);
        setTimeout(() => setNotifs(n => n.filter(x => x.id !== id)), 3000);
    }, []);

    const loadFiles = useCallback(async (path: string) => {
        setLoading(true);
        setSelected(new Set());
        try {
            const entries = await SystemBridge.getFiles(path);
            setFiles(entries);
        } catch (e: any) {
            notify('error', e?.message ?? String(e));
        } finally {
            setLoading(false);
        }
    }, [notify]);

    useEffect(() => { if (tab?.path) loadFiles(tab.path); }, [tab?.path]);

    const navigate = useCallback((path: string) => {
        setTabs(prev => prev.map((t, i) => {
            if (i !== activeTab) return t;
            const newHist = [...t.history.slice(0, t.historyIndex + 1), path];
            return { ...t, path, history: newHist, historyIndex: newHist.length - 1 };
        }));
    }, [activeTab]);

    const goBack    = useCallback(() => {
        setTabs(prev => prev.map((t, i) => {
            if (i !== activeTab || t.historyIndex <= 0) return t;
            const ni = t.historyIndex - 1;
            return { ...t, path: t.history[ni], historyIndex: ni };
        }));
    }, [activeTab]);

    const goForward = useCallback(() => {
        setTabs(prev => prev.map((t, i) => {
            if (i !== activeTab || t.historyIndex >= t.history.length - 1) return t;
            const ni = t.historyIndex + 1;
            return { ...t, path: t.history[ni], historyIndex: ni };
        }));
    }, [activeTab]);

    const goUp = useCallback(() => {
        const parent = tab?.path.split('/').slice(0, -1).join('/') || 'HOME';
        navigate(parent);
    }, [tab?.path, navigate]);

    const openFile = useCallback((file: FileEntry) => {
        if (file.is_dir) { navigate(file.path); return; }
        SystemBridge.executeCommand(`xdg-open "${file.path}"`).catch(() => {});
    }, [navigate]);

    const deleteSelected = useCallback(async () => {
        const items = files.filter(f => selected.has(f.path));
        if (!items.length) return;
        const ok = await dialog.confirm({
            title: 'Delete files', message: `Delete ${items.length} item(s)? This cannot be undone.`,
            confirmLabel: 'Delete', danger: true,
        });
        if (!ok) return;
        for (const f of items) await SystemBridge.deleteFile(f.path).catch(() => {});
        notify('success', `Deleted ${items.length} item(s)`);
        loadFiles(tab.path);
    }, [files, selected, dialog, notify, tab?.path, loadFiles]);

    const paste = useCallback(async () => {
        if (!clipboard) return;
        for (const f of clipboard.files) {
            const dest = `${tab.path}/${f.name}`;
            if (clipboard.op === 'copy') await SystemBridge.copyFile(f.path, dest).catch(() => {});
            else await SystemBridge.moveFile(f.path, dest).catch(() => {});
        }
        if (clipboard.op === 'cut') setClipboard(null);
        notify('success', `Pasted ${clipboard.files.length} item(s)`);
        loadFiles(tab.path);
    }, [clipboard, tab?.path, notify, loadFiles]);

    const createFolder = useCallback(async () => {
        const name = await dialog.prompt({ title: 'New Folder', placeholder: 'New Folder', defaultValue: 'New Folder' });
        if (!name) return;
        await SystemBridge.createFolder(tab.path, name);
        notify('success', `Folder "${name}" created`);
        loadFiles(tab.path);
    }, [dialog, tab?.path, notify, loadFiles]);

    const rename = useCallback(async (file: FileEntry) => {
        const name = await dialog.prompt({ title: 'Rename', defaultValue: file.name });
        if (!name || name === file.name) return;
        const dest = `${file.path.slice(0, file.path.length - file.name.length)}${name}`;
        await SystemBridge.moveFile(file.path, dest);
        notify('success', `Renamed to "${name}"`);
        loadFiles(tab.path);
    }, [dialog, notify, tab?.path, loadFiles]);

    const sortedFiles = [...files]
        .filter(f => !search || f.name.toLowerCase().includes(search.toLowerCase()))
        .sort((a, b) => {
            if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
            let v = 0;
            if (sortKey === 'name')     v = a.name.localeCompare(b.name);
            else if (sortKey === 'size')     v = a.size.localeCompare(b.size);
            else if (sortKey === 'modified') v = (a.modified ?? '').localeCompare(b.modified ?? '');
            else if (sortKey === 'type')     v = a.mime_type.localeCompare(b.mime_type);
            return sortAsc ? v : -v;
        });

    const newTab = useCallback(() => {
        setTabs(prev => [...prev, makeTab(tab?.path || 'HOME')]);
        setActiveTab(tabs.length);
    }, [tab?.path, tabs.length]);

    const closeTab = useCallback((idx: number) => {
        if (tabs.length === 1) return;
        setTabs(prev => prev.filter((_, i) => i !== idx));
        setActiveTab(prev => Math.max(0, idx < prev ? prev - 1 : Math.min(prev, tabs.length - 2)));
    }, [tabs.length]);

    return {
        tab, tabs, activeTab, setActiveTab,
        files: sortedFiles, loading, selected, setSelected, clipboard, setClipboard,
        sortKey, setSortKey, sortAsc, setSortAsc, search, setSearch, notifs,
        navigate, goBack, goForward, goUp, openFile, loadFiles,
        deleteSelected, paste, createFolder, rename, notify,
        newTab, closeTab,
    };
}
