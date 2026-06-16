import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
    Folder, FileText, HardDrive, ArrowLeft, ArrowRight, RefreshCw,
    Plus, Trash2, Copy, Clipboard, Scissors, Home, Image,
    File, Music, Video, Archive, Code, ChevronRight,
    Grid, List, Eye, X, Download, Edit, Search,
    SortAsc, SortDesc, Columns, Info, Check, AlertCircle, Loader2
} from 'lucide-react';
import { AppProps } from '../../../types';
import { SystemBridge } from '../../../utils/systemBridge';

interface FileEntry {
    name: string; path: string; is_dir: boolean;
    size: string; mime_type: string; modified?: string;
}
interface Tab { id: string; path: string; history: string[]; historyIndex: number; }
interface Notif { id: string; type: 'success'|'error'|'info'; message: string; }
type SortKey = 'name'|'size'|'modified'|'type';

const BOOKMARKS = [
    { name: 'Home',      path: 'HOME',           icon: Home },
{ name: 'Desktop',   path: 'HOME/Desktop',   icon: Folder },
{ name: 'Documents', path: 'HOME/Documents', icon: FileText },
{ name: 'Downloads', path: 'HOME/Downloads', icon: Download },
{ name: 'Pictures',  path: 'HOME/Pictures',  icon: Image },
{ name: 'Music',     path: 'HOME/Music',     icon: Music },
{ name: 'Videos',    path: 'HOME/Videos',    icon: Video },
];

function getIcon(file: FileEntry, size = 40) {
    if (file.is_dir) return <Folder size={size} className="text-blue-400" />;
    const m = file.mime_type;
    if (m.startsWith('image/'))  return <Image   size={size} className="text-green-400" />;
    if (m.startsWith('audio/'))  return <Music   size={size} className="text-purple-400" />;
    if (m.startsWith('video/'))  return <Video   size={size} className="text-red-400" />;
    if (m.startsWith('text/') || m.includes('json') || m.includes('xml'))
        return <FileText size={size} className="text-yellow-400" />;
    if (m.includes('zip') || m.includes('tar') || m.includes('gz'))
        return <Archive  size={size} className="text-orange-400" />;
    const ext = file.name.split('.').pop()?.toLowerCase() ?? '';
    if (['rs','ts','tsx','js','jsx','py','go','c','cpp'].includes(ext))
        return <Code size={size} className="text-cyan-400" />;
    return <File size={size} className="text-slate-400" />;
}

const ExplorerApp: React.FC<AppProps> = () => {
    const [tabs, setTabs] = useState<Tab[]>([{
        id: 'tab-1', path: 'HOME', history: ['HOME'], historyIndex: 0
    }]);
    const [activeTabId, setActiveTabId] = useState('tab-1');
    const [dualPane, setDualPane] = useState(false);
    const [rightPath, setRightPath] = useState('HOME');

    const [files, setFiles] = useState<FileEntry[]>([]);
    const [loading, setLoading] = useState(false);
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [lastSel, setLastSel] = useState<string|null>(null);

    const [viewMode, setViewMode] = useState<'grid'|'list'>('grid');
    const [showHidden, setShowHidden] = useState(false);
    const [sortBy, setSortBy] = useState<SortKey>('name');
    const [sortAsc, setSortAsc] = useState(true);
    const [searchTerm, setSearchTerm] = useState('');
    const [showSearch, setShowSearch] = useState(false);

    const [clipboard, setClipboard] = useState<{action:'copy'|'cut'; files:string[]}|null>(null);
    const [previewFile, setPreviewFile] = useState<FileEntry|null>(null);
    const [previewContent, setPreviewContent] = useState('');
    const [previewLoading, setPreviewLoading] = useState(false);
    const [thumbnails, setThumbnails] = useState<Record<string,string>>({});
    const [notifs, setNotifs] = useState<Notif[]>([]);
    const [renaming, setRenaming] = useState<string|null>(null);
    const [renameVal, setRenameVal] = useState('');
    const [dragOver, setDragOver] = useState<string|null>(null);

    const gridRef   = useRef<HTMLDivElement>(null);
    const searchRef = useRef<HTMLInputElement>(null);
    const renameRef = useRef<HTMLInputElement>(null);

    const activeTab = tabs.find(t => t.id === activeTabId) ?? tabs[0];

    const notify = useCallback((type: Notif['type'], message: string) => {
        const id = Date.now().toString();
        setNotifs(p => [...p, { id, type, message }]);
        setTimeout(() => setNotifs(p => p.filter(n => n.id !== id)), 3500);
    }, []);

    const loadFiles = useCallback(async (path: string) => {
        setLoading(true); setSelected(new Set()); setPreviewFile(null);
        try {
            const entries = await SystemBridge.getFiles(path);
            setFiles(entries);
            // Load image thumbnails in background
            for (const e of entries.filter((e: FileEntry) => e.mime_type.startsWith('image/')).slice(0, 24)) {
                if (!thumbnails[e.path]) {
                    SystemBridge.readFileAsDataURL(e.path)
                    .then((d: string | null) => d && setThumbnails(p => ({ ...p, [e.path]: d })))
                    .catch(() => {});
                }
            }
        } catch { notify('error', `Cannot open: ${path}`); }
        finally { setLoading(false); }
    }, [thumbnails, notify]);

    const navigateTo = useCallback((path: string) => {
        setTabs(prev => prev.map(t => {
            if (t.id !== activeTabId) return t;
            const h = t.history.slice(0, t.historyIndex + 1);
            h.push(path);
            return { ...t, path, history: h, historyIndex: h.length - 1 };
        }));
        loadFiles(path);
    }, [activeTabId, loadFiles]);

    useEffect(() => { loadFiles(activeTab.path); }, [activeTabId]);

    const goBack = () => {
        const t = activeTab;
        if (t.historyIndex <= 0) return;
        const ni = t.historyIndex - 1;
        const np = t.history[ni];
        setTabs(p => p.map(tab => tab.id === activeTabId ? { ...tab, historyIndex: ni, path: np } : tab));
        loadFiles(np);
    };

    const goForward = () => {
        const t = activeTab;
        if (t.historyIndex >= t.history.length - 1) return;
        const ni = t.historyIndex + 1;
        const np = t.history[ni];
        setTabs(p => p.map(tab => tab.id === activeTabId ? { ...tab, historyIndex: ni, path: np } : tab));
        loadFiles(np);
    };

    const goUp = () => {
        const cur = activeTab.path;
        if (cur === 'HOME' || cur === '/') return;
        const parent = cur.includes('/') ? cur.split('/').slice(0, -1).join('/') || '/' : 'HOME';
        navigateTo(parent);
    };

    const addTab = () => {
        const id = `tab-${Date.now()}`;
        setTabs(p => [...p, { id, path: 'HOME', history: ['HOME'], historyIndex: 0 }]);
        setActiveTabId(id);
        loadFiles('HOME');
    };

    const closeTab = (id: string) => {
        if (tabs.length === 1) return;
        const next = tabs.filter(t => t.id !== id);
        setTabs(next);
        if (activeTabId === id) {
            const n = next[next.length - 1];
            setActiveTabId(n.id);
            loadFiles(n.path);
        }
    };

    const handleOpen = (file: FileEntry) => {
        if (file.is_dir) { navigateTo(file.path); return; }
        if (file.mime_type.startsWith('image/') || file.mime_type.startsWith('text/')) {
            openPreview(file);
        } else {
            SystemBridge.launchApp(`xdg-open "${file.path}"`);
        }
    };

    const createFolder = async () => {
        const name = window.prompt('New folder name:');
        if (!name?.trim()) return;
        try {
            await SystemBridge.createFolder(activeTab.path, name.trim());
            notify('success', `Created: ${name}`);
            loadFiles(activeTab.path);
        } catch { notify('error', 'Failed to create folder'); }
    };

    const createFile = async () => {
        const name = window.prompt('New file name (e.g. note.txt):');
        if (!name?.trim()) return;
        try {
            await SystemBridge.createTextFile(activeTab.path, name.trim(), '');
            notify('success', `Created: ${name}`);
            loadFiles(activeTab.path);
        } catch { notify('error', 'Failed to create file'); }
    };

    const deleteSelected = async () => {
        if (selected.size === 0) return;
        if (!window.confirm(`Delete ${selected.size} item(s)? This cannot be undone.`)) return;
        let errors = 0;
        for (const path of selected) {
            try { await SystemBridge.deleteFile(path); } catch { errors++; }
        }
        if (errors) notify('error', `${errors} item(s) failed`);
        else notify('success', `Deleted ${selected.size} item(s)`);
        setSelected(new Set());
        loadFiles(activeTab.path);
    };

    const copySelected = () => {
        if (!selected.size) return;
        setClipboard({ action: 'copy', files: [...selected] });
        notify('info', `Copied ${selected.size} item(s)`);
    };

    const cutSelected = () => {
        if (!selected.size) return;
        setClipboard({ action: 'cut', files: [...selected] });
        notify('info', `Cut ${selected.size} item(s)`);
    };

    const paste = async () => {
        if (!clipboard) return;
        let errors = 0;
        for (const src of clipboard.files) {
            const name = src.split('/').pop() ?? '';
            const dst  = `${activeTab.path}/${name}`;
            try {
                if (clipboard.action === 'copy') await SystemBridge.copyFile(src, dst);
                else                             await SystemBridge.moveFile(src, dst);
            } catch { errors++; }
        }
        if (errors) notify('error', `${errors} item(s) failed`);
        else notify('success', `Pasted ${clipboard.files.length} item(s)`);
        if (clipboard.action === 'cut') setClipboard(null);
        loadFiles(activeTab.path);
    };

    const startRename = (file: FileEntry) => {
        setRenaming(file.path); setRenameVal(file.name);
        setTimeout(() => renameRef.current?.select(), 50);
    };

    const commitRename = async () => {
        if (!renaming || !renameVal.trim()) { setRenaming(null); return; }
        const file = files.find(f => f.path === renaming);
        if (!file || renameVal === file.name) { setRenaming(null); return; }
        const newPath = renaming.slice(0, renaming.lastIndexOf('/') + 1) + renameVal.trim();
        try {
            await SystemBridge.moveFile(renaming, newPath);
            notify('success', `Renamed to: ${renameVal}`);
            loadFiles(activeTab.path);
        } catch { notify('error', 'Rename failed'); }
        finally { setRenaming(null); }
    };

    const openPreview = async (file: FileEntry) => {
        setPreviewFile(file); setPreviewLoading(true);
        try {
            if (file.mime_type.startsWith('image/')) {
                const d = await SystemBridge.readFileAsDataURL(file.path);
                setPreviewContent(d ?? '');
            } else {
                const c = await SystemBridge.readFile(file.path);
                setPreviewContent(c);
            }
        } catch { setPreviewContent(''); }
        finally { setPreviewLoading(false); }
    };

    const toggleSelect = (path: string, e?: React.MouseEvent) => {
        if (renaming) return;
        if (e?.shiftKey && lastSel) {
            const i1 = sorted.findIndex(f => f.path === lastSel);
            const i2 = sorted.findIndex(f => f.path === path);
            const [a, b] = [Math.min(i1,i2), Math.max(i1,i2)];
            setSelected(new Set(sorted.slice(a, b+1).map(f => f.path)));
        } else if (e?.ctrlKey || e?.metaKey) {
            setSelected(p => { const n = new Set(p); n.has(path) ? n.delete(path) : n.add(path); return n; });
        } else {
            setSelected(new Set([path]));
        }
        setLastSel(path);
    };

    const filtered = files
    .filter(f => showHidden || !f.name.startsWith('.'))
    .filter(f => !searchTerm || f.name.toLowerCase().includes(searchTerm.toLowerCase()));

    const sorted = [...filtered].sort((a, b) => {
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
        let cmp = 0;
        if (sortBy === 'name')     cmp = a.name.localeCompare(b.name);
        else if (sortBy === 'size') cmp = (parseFloat(a.size)||0) - (parseFloat(b.size)||0);
        else if (sortBy === 'modified') cmp = (a.modified??'').localeCompare(b.modified??'');
        else if (sortBy === 'type') cmp = a.mime_type.localeCompare(b.mime_type);
        return sortAsc ? cmp : -cmp;
    });

    const toggleSort = (k: SortKey) => {
        if (sortBy === k) setSortAsc(a => !a); else { setSortBy(k); setSortAsc(true); }
    };

    // Drag handlers
    const onDragStart = (e: React.DragEvent, file: FileEntry) => {
        const paths = selected.has(file.path) ? [...selected] : [file.path];
        e.dataTransfer.setData('text/plain', JSON.stringify(paths));
        e.dataTransfer.effectAllowed = 'copyMove';
    };

    const onDrop = async (e: React.DragEvent, targetDir: FileEntry | null) => {
        e.preventDefault(); setDragOver(null);
        const dest = targetDir?.path ?? activeTab.path;
        try {
            const paths: string[] = JSON.parse(e.dataTransfer.getData('text/plain'));
            const isCopy = e.ctrlKey;
            for (const src of paths) {
                const name = src.split('/').pop() ?? '';
                const dst  = `${dest}/${name}`;
                if (isCopy) await SystemBridge.copyFile(src, dst);
                else        await SystemBridge.moveFile(src, dst);
            }
            notify('success', `${e.ctrlKey ? 'Copied' : 'Moved'} ${paths.length} item(s)`);
            loadFiles(activeTab.path);
        } catch { notify('error', 'Drop failed'); }
    };

    // Keyboard shortcuts
    useEffect(() => {
        const h = (e: KeyboardEvent) => {
            if (renaming) return;
            if (e.ctrlKey) {
                if (e.key === 'a') { e.preventDefault(); setSelected(new Set(sorted.map(f => f.path))); }
                if (e.key === 'c') { e.preventDefault(); copySelected(); }
                if (e.key === 'x') { e.preventDefault(); cutSelected(); }
                if (e.key === 'v') { e.preventDefault(); paste(); }
                if (e.key === 'f') { e.preventDefault(); setShowSearch(s => !s); setTimeout(() => searchRef.current?.focus(), 50); }
                if (e.key === 'n') { e.preventDefault(); createFolder(); }
                if (e.key === 't') { e.preventDefault(); addTab(); }
            }
            if (e.key === 'F2' && selected.size === 1) {
                const f = files.find(f => selected.has(f.path));
                if (f) startRename(f);
            }
            if (e.key === 'Delete') { e.preventDefault(); deleteSelected(); }
            if (e.key === 'Escape') { setSearchTerm(''); setShowSearch(false); setSelected(new Set()); }
        };
        window.addEventListener('keydown', h);
        return () => window.removeEventListener('keydown', h);
    }, [selected, files, renaming, clipboard, sorted, activeTab]);

    const breadcrumbs = activeTab.path.split('/').map((part, i, arr) => ({
        label: part === 'HOME' ? '~' : part,
        path: arr.slice(0, i+1).join('/') || '/',
    }));

    const SH = ({ label, sk }: { label: string; sk: SortKey }) => (
        <th className="p-2 text-left cursor-pointer hover:text-white select-none whitespace-nowrap"
        onClick={() => toggleSort(sk)}>
        <span className="flex items-center gap-1">
        {label}
        {sortBy === sk && (sortAsc ? <SortAsc size={11}/> : <SortDesc size={11}/>)}
        </span>
        </th>
    );

    const renderItem = (file: FileEntry) => {
        const isSel    = selected.has(file.path);
        const isDragTgt = dragOver === file.path && file.is_dir;
        const isCut    = clipboard?.action === 'cut' && clipboard.files.includes(file.path);

        if (viewMode === 'grid') {
            return (
                <div key={file.path}
                draggable
                onDragStart={e => onDragStart(e, file)}
                onDragOver={e => { if (file.is_dir) { e.preventDefault(); setDragOver(file.path); } }}
                onDrop={e => file.is_dir && onDrop(e, file)}
                onDragLeave={() => setDragOver(null)}
                className={`relative flex flex-col items-center p-2 rounded-xl cursor-pointer
                    transition-colors duration-100 select-none
                    ${isSel ? 'bg-blue-600/30 ring-2 ring-blue-500/60' : 'hover:bg-white/5'}
                    ${isDragTgt ? 'bg-blue-500/20 ring-2 ring-blue-400' : ''}
                    ${isCut ? 'opacity-50' : ''}
                    `}
                    onClick={e => { e.stopPropagation(); toggleSelect(file.path, e); }}
                    onDoubleClick={() => { if (!renaming) handleOpen(file); }}
                    >
                    <div className="mb-1.5">
                    {file.mime_type.startsWith('image/') && thumbnails[file.path] ? (
                        <img src={thumbnails[file.path]} alt={file.name}
                        className="w-12 h-12 object-cover rounded-lg" />
                    ) : getIcon(file, 40)}
                    </div>
                    {renaming === file.path ? (
                        <input ref={renameRef} value={renameVal}
                        onChange={e => setRenameVal(e.target.value)}
                        onBlur={commitRename}
                        onKeyDown={e => {
                            if (e.key === 'Enter') commitRename();
                            if (e.key === 'Escape') setRenaming(null);
                            e.stopPropagation();
                        }}
                        className="w-full text-xs text-center bg-slate-800 border border-blue-500 rounded px-1 focus:outline-none"
                        autoFocus onClick={e => e.stopPropagation()}
                        />
                    ) : (
                        <span className="text-xs text-center break-all line-clamp-2 leading-tight px-1">
                        {file.name}
                        </span>
                    )}
                    <span className="text-[10px] text-slate-600 mt-0.5">{file.size}</span>
                    </div>
            );
        }

        // list mode
        return (
            <tr key={file.path}
            draggable
            onDragStart={e => onDragStart(e, file)}
            onDragOver={e => { if (file.is_dir) { e.preventDefault(); setDragOver(file.path); } }}
            onDrop={e => file.is_dir && onDrop(e, file)}
            onDragLeave={() => setDragOver(null)}
            className={`border-b border-white/5 cursor-pointer group
                ${isSel ? 'bg-blue-600/20' : 'hover:bg-white/5'}
                ${isDragTgt ? 'bg-blue-500/15' : ''}
                ${isCut ? 'opacity-50' : ''}
                `}
                onClick={e => toggleSelect(file.path, e)}
                onDoubleClick={() => handleOpen(file)}
                >
                <td className="p-2">
                <div className="flex items-center gap-2 min-w-0">
                {file.mime_type.startsWith('image/') && thumbnails[file.path]
                    ? <img src={thumbnails[file.path]} alt="" className="w-5 h-5 object-cover rounded shrink-0" />
                    : getIcon(file, 16)}
                    {renaming === file.path ? (
                        <input ref={renameRef} value={renameVal}
                        onChange={e => setRenameVal(e.target.value)}
                        onBlur={commitRename}
                        onKeyDown={e => {
                            if (e.key === 'Enter') commitRename();
                            if (e.key === 'Escape') setRenaming(null);
                            e.stopPropagation();
                        }}
                        className="flex-1 bg-slate-800 border border-blue-500 rounded px-1 text-sm focus:outline-none"
                        autoFocus onClick={e => e.stopPropagation()}
                        />
                    ) : (
                        <span className="text-sm truncate">{file.name}</span>
                    )}
                    </div>
                    </td>
                    <td className="p-2 text-sm text-slate-500 whitespace-nowrap">{file.size}</td>
                    <td className="p-2 text-sm text-slate-500 whitespace-nowrap">{file.modified ?? '—'}</td>
                    <td className="p-2 text-xs text-slate-600 max-w-[100px] truncate">{file.mime_type}</td>
                    <td className="p-2">
                    <div className="flex gap-1 opacity-0 group-hover:opacity-100">
                    <button onClick={e => { e.stopPropagation(); startRename(file); }}
                    className="p-1 hover:bg-white/10 rounded text-slate-400"><Edit size={12}/></button>
                    <button onClick={e => {
                        e.stopPropagation();
                        setSelected(new Set([file.path]));
                        setTimeout(deleteSelected, 0);
                    }}
                    className="p-1 hover:bg-red-500/20 rounded text-red-400"><Trash2 size={12}/></button>
                    </div>
                    </td>
                    </tr>
        );
    };

    return (
        <div className="flex h-full bg-slate-900 text-white overflow-hidden"
        onDragOver={e => e.preventDefault()}
        onDrop={e => onDrop(e, null)}
        >
        {/* Notifications */}
        <div className="fixed top-16 right-4 z-50 flex flex-col gap-2 pointer-events-none">
        {notifs.map(n => (
            <div key={n.id} className={`flex items-center gap-2 px-4 py-2 rounded-xl shadow-lg text-sm
                ${n.type === 'success' ? 'bg-green-600/90' : n.type === 'error' ? 'bg-red-600/90' : 'bg-slate-700/90'}
                `}>
                {n.type === 'success' && <Check size={13}/>}
                {n.type === 'error'   && <AlertCircle size={13}/>}
                {n.type === 'info'    && <Info size={13}/>}
                {n.message}
                </div>
        ))}
        </div>

        {/* Sidebar */}
        <div className="w-44 bg-slate-800/50 border-r border-white/5 flex flex-col shrink-0">
        <div className="p-3 border-b border-white/5">
        <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">Places</span>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-0.5">
        {BOOKMARKS.map(bm => (
            <button key={bm.path} onClick={() => navigateTo(bm.path)}
            className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm transition-colors
                ${activeTab.path === bm.path ? 'bg-blue-600/20 text-blue-400' : 'text-slate-400 hover:bg-white/5 hover:text-white'}
                `}>
                <bm.icon size={14}/>{bm.name}
                </button>
        ))}
        <div className="h-px bg-white/5 my-2"/>
        <button onClick={() => navigateTo('/')}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm text-slate-400 hover:bg-white/5 hover:text-white">
        <HardDrive size={14}/> / (root)
        </button>
        </div>
        </div>

        {/* Main */}
        <div className="flex-1 flex flex-col min-w-0">
        {/* Tab bar */}
        <div className="flex items-center bg-slate-800/80 border-b border-white/5 overflow-x-auto shrink-0">
        {tabs.map(tab => (
            <div key={tab.id}
            onClick={() => { setActiveTabId(tab.id); loadFiles(tab.path); }}
            className={`flex items-center gap-1.5 px-3 py-2 cursor-pointer border-r border-white/5 shrink-0 group max-w-[140px]
                ${tab.id === activeTabId ? 'bg-slate-900 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}
                `}>
                <Folder size={12}/>
                <span className="text-xs truncate">
                {tab.path === 'HOME' ? '~' : tab.path.split('/').pop()}
                </span>
                {tabs.length > 1 && (
                    <button onClick={e => { e.stopPropagation(); closeTab(tab.id); }}
                    className="opacity-0 group-hover:opacity-100 hover:text-red-400 ml-auto shrink-0">
                    <X size={10}/>
                    </button>
                )}
                </div>
        ))}
        <button onClick={addTab} className="p-2 text-slate-500 hover:text-white shrink-0"><Plus size={12}/></button>
        <div className="ml-auto pr-2">
        <button onClick={() => setDualPane(d => !d)}
        title="Dual pane"
        className={`p-1.5 rounded ${dualPane ? 'text-blue-400' : 'text-slate-500 hover:text-white'}`}>
        <Columns size={13}/>
        </button>
        </div>
        </div>

        {/* Toolbar */}
        <div className="h-11 bg-slate-800 border-b border-white/5 flex items-center px-2 gap-1 shrink-0">
        <button onClick={goBack} disabled={activeTab.historyIndex === 0}
        className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><ArrowLeft size={15}/></button>
        <button onClick={goForward} disabled={activeTab.historyIndex >= activeTab.history.length - 1}
        className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><ArrowRight size={15}/></button>
        <button onClick={goUp} className="p-1.5 hover:bg-white/10 rounded"><HardDrive size={15}/></button>
        <button onClick={() => loadFiles(activeTab.path)} className="p-1.5 hover:bg-white/10 rounded">
        <RefreshCw size={15} className={loading ? 'animate-spin' : ''}/>
        </button>
        <div className="w-px h-5 bg-white/10 mx-1"/>
        <button onClick={createFolder} title="New folder (Ctrl+N)" className="p-1.5 hover:bg-white/10 rounded"><Plus size={15}/></button>
        <button onClick={deleteSelected} disabled={!selected.size}
        title="Delete (Del)" className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Trash2 size={15}/></button>
        <button onClick={copySelected} disabled={!selected.size}
        title="Copy (Ctrl+C)" className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Copy size={15}/></button>
        <button onClick={cutSelected} disabled={!selected.size}
        title="Cut (Ctrl+X)" className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Scissors size={15}/></button>
        <button onClick={paste} disabled={!clipboard}
        title="Paste (Ctrl+V)" className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30">
        <Clipboard size={15} className={clipboard ? 'text-blue-400' : ''}/>
        </button>
        <div className="w-px h-5 bg-white/10 mx-1"/>
        {/* Breadcrumbs */}
        <div className="flex items-center text-sm flex-1 min-w-0 overflow-hidden">
        {breadcrumbs.map((crumb, i) => (
            <React.Fragment key={i}>
            {i > 0 && <ChevronRight size={11} className="mx-0.5 text-slate-600 shrink-0"/>}
            <button onClick={() => navigateTo(crumb.path)}
            className="hover:text-blue-400 transition-colors shrink-0 truncate max-w-[80px]">
            {crumb.label}
            </button>
            </React.Fragment>
        ))}
        </div>
        <div className="flex items-center gap-1 ml-auto shrink-0">
        <button onClick={() => { setShowSearch(s => !s); setTimeout(() => searchRef.current?.focus(), 50); }}
        className={`p-1.5 rounded ${showSearch ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}><Search size={15}/></button>
        <button onClick={() => setShowHidden(h => !h)}
        className={`p-1.5 rounded ${showHidden ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Show hidden"><Eye size={15}/></button>
        <button onClick={() => setViewMode('grid')}
        className={`p-1.5 rounded ${viewMode==='grid' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}><Grid size={15}/></button>
        <button onClick={() => setViewMode('list')}
        className={`p-1.5 rounded ${viewMode==='list' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}><List size={15}/></button>
        </div>
        </div>

        {/* Search bar */}
        {showSearch && (
            <div className="flex items-center gap-2 px-3 py-1.5 bg-slate-800/50 border-b border-white/5">
            <Search size={13} className="text-slate-500 shrink-0"/>
            <input ref={searchRef} type="text" value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            placeholder="Search in current folder…"
            className="flex-1 bg-transparent text-sm focus:outline-none placeholder-slate-600"
            />
            {searchTerm && <span className="text-xs text-slate-500 shrink-0">{sorted.length}</span>}
            <button onClick={() => { setSearchTerm(''); setShowSearch(false); }}><X size={13} className="text-slate-500"/></button>
            </div>
        )}

        {/* Files + preview + dual pane */}
        <div className="flex-1 flex overflow-hidden">
        {/* File area */}
        <div ref={gridRef} className="flex-1 overflow-auto p-2"
        onClick={e => { if (e.target === gridRef.current) setSelected(new Set()); }}>
        {loading ? (
            <div className="flex items-center justify-center h-full">
            <Loader2 size={22} className="animate-spin text-blue-400"/>
            </div>
        ) : sorted.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-slate-600 gap-2">
            <Folder size={36}/>
            <span className="text-sm">{searchTerm ? 'No results' : 'Empty folder'}</span>
            </div>
        ) : viewMode === 'grid' ? (
            <div className="grid gap-1"
            style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(88px, 1fr))' }}>
            {sorted.map(renderItem)}
            </div>
        ) : (
            <table className="w-full text-sm border-collapse">
            <thead className="sticky top-0 bg-slate-800 z-10 text-slate-400 text-xs">
            <tr>
            <SH label="Name"     sk="name"/>
            <SH label="Size"     sk="size"/>
            <SH label="Modified" sk="modified"/>
            <SH label="Type"     sk="type"/>
            <th className="w-14"/>
            </tr>
            </thead>
            <tbody>{sorted.map(renderItem)}</tbody>
            </table>
        )}
        </div>

        {/* Preview panel */}
        {previewFile && (
            <div className="w-60 border-l border-white/5 bg-slate-800/50 flex flex-col shrink-0">
            <div className="flex items-center justify-between p-2 border-b border-white/5">
            <span className="text-xs font-medium truncate">{previewFile.name}</span>
            <button onClick={() => setPreviewFile(null)} className="p-1 hover:bg-white/10 rounded shrink-0"><X size={12}/></button>
            </div>
            <div className="flex-1 overflow-auto p-2">
            <div className="text-[10px] text-slate-500 mb-2 space-y-0.5">
            <div>Size: {previewFile.size}</div>
            <div>Modified: {previewFile.modified ?? '—'}</div>
            </div>
            {previewLoading ? (
                <Loader2 size={16} className="animate-spin text-blue-400"/>
            ) : previewFile.mime_type.startsWith('image/') && previewContent ? (
                <img src={previewContent} alt="preview" className="max-w-full rounded object-contain"/>
            ) : previewFile.mime_type.startsWith('text/') && previewContent ? (
                <pre className="text-[10px] bg-slate-900 p-2 rounded overflow-auto max-h-64 font-mono">{previewContent.slice(0, 3000)}</pre>
            ) : (
                <div className="text-center text-slate-600 text-xs py-4">No preview</div>
            )}
            </div>
            <div className="p-2 border-t border-white/5 flex gap-1.5">
            <button onClick={() => SystemBridge.launchApp(`xdg-open "${previewFile.path}"`)}
            className="flex-1 py-1 bg-blue-600 hover:bg-blue-500 rounded text-xs">Open</button>
            <button onClick={() => startRename(previewFile)}
            className="p-1.5 hover:bg-white/10 rounded"><Edit size={12}/></button>
            <button onClick={() => { setSelected(new Set([previewFile.path])); deleteSelected(); setPreviewFile(null); }}
            className="p-1.5 hover:bg-red-500/20 rounded text-red-400"><Trash2 size={12}/></button>
            </div>
            </div>
        )}

        {/* Dual pane */}
        {dualPane && (
            <div className="w-72 border-l border-white/5 bg-slate-900 flex flex-col shrink-0">
            <div className="flex items-center gap-2 p-2 bg-slate-800 border-b border-white/5 text-xs text-slate-400">
            <Columns size={11}/>
            <span className="truncate flex-1">{rightPath}</span>
            <button onClick={() => { const p = rightPath.includes('/') ? rightPath.split('/').slice(0,-1).join('/')||'/' : 'HOME'; setRightPath(p); }}
            className="hover:text-white shrink-0"><ArrowLeft size={11}/></button>
            </div>
            <RightPane path={rightPath} onNavigate={setRightPath} thumbnails={thumbnails}/>
            </div>
        )}
        </div>

        {/* Status bar */}
        <div className="h-5 border-t border-white/5 bg-slate-800/50 flex items-center px-3 gap-4 text-[11px] text-slate-600 shrink-0">
        <span>{sorted.length} items</span>
        {selected.size > 0 && <span>{selected.size} selected</span>}
        {clipboard && <span className="text-blue-500">{clipboard.action === 'copy' ? 'Copied' : 'Cut'} {clipboard.files.length}</span>}
        <span className="ml-auto">{activeTab.path}</span>
        </div>
        </div>
        </div>
    );
};

// Right pane for dual panel mode
const RightPane: React.FC<{ path: string; onNavigate: (p: string) => void; thumbnails: Record<string,string> }> = ({ path, onNavigate, thumbnails }) => {
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [loading, setLoading] = useState(false);
    useEffect(() => {
        setLoading(true);
        SystemBridge.getFiles(path).then((f: FileEntry[]) => { setFiles(f); setLoading(false); }).catch(() => setLoading(false));
    }, [path]);
    return (
        <div className="flex-1 overflow-auto p-2">
        {loading ? <Loader2 size={16} className="animate-spin text-blue-400 m-4"/> :
        <div className="grid gap-1" style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(72px, 1fr))' }}>
        {files.filter(f => !f.name.startsWith('.')).map(file => (
            <div key={file.path}
            onDoubleClick={() => file.is_dir && onNavigate(file.path)}
            className="flex flex-col items-center p-1.5 rounded-lg hover:bg-white/5 cursor-pointer">
            {file.mime_type.startsWith('image/') && thumbnails[file.path]
                ? <img src={thumbnails[file.path]} alt="" className="w-10 h-10 object-cover rounded"/>
                : getIcon(file, 32)
            }
            <span className="text-[10px] text-center line-clamp-2 mt-1 leading-tight">{file.name}</span>
            </div>
        ))}
        </div>
        }
        </div>
    );
};

export default ExplorerApp;
