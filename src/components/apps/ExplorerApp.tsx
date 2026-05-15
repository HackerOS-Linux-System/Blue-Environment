import React, { useState, useEffect, useCallback, useRef } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import {
    Folder, File, ChevronRight, ChevronLeft, Home, RefreshCw, Grid, List,
    Search, Eye, EyeOff, Copy, Scissors, Clipboard, Trash2, Plus,
    FolderPlus, Edit3, ArrowUp, HardDrive, Download, Image, FileText,
    Film, Music, Archive, Code, X, Check, AlertTriangle
} from 'lucide-react';

interface FileEntry {
    name: string; path: string; isDir: boolean;
    size: number; modified: string; extension: string;
}

interface ClipboardItem { paths: string[]; mode: 'copy' | 'cut'; }
type SortKey = 'name' | 'size' | 'modified';

// Inline dialog component - replaces window.prompt/confirm
interface DialogProps {
    type: 'input' | 'confirm';
    title: string;
    message?: string;
    defaultValue?: string;
    confirmLabel?: string;
    danger?: boolean;
    onConfirm: (value?: string) => void;
    onCancel: () => void;
}

function InlineDialog({ type, title, message, defaultValue = '', confirmLabel = 'OK', danger, onConfirm, onCancel }: DialogProps) {
    const [value, setValue] = useState(defaultValue);
    const inputRef = useRef<HTMLInputElement>(null);
    useEffect(() => { inputRef.current?.focus(); inputRef.current?.select(); }, []);

    const handleKey = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') onConfirm(type === 'input' ? value : undefined);
        if (e.key === 'Escape') onCancel();
    };

    return (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
            <div className="bg-slate-800 rounded-2xl border border-white/10 shadow-2xl p-6 w-80" onKeyDown={handleKey}>
                <div className="flex items-center gap-3 mb-4">
                    {danger && <AlertTriangle size={18} className="text-red-400 shrink-0" />}
                    <h3 className="font-semibold text-white">{title}</h3>
                </div>
                {message && <p className="text-sm text-slate-400 mb-4">{message}</p>}
                {type === 'input' && (
                    <input
                        ref={inputRef}
                        value={value}
                        onChange={e => setValue(e.target.value)}
                        className="w-full bg-slate-900 border border-white/10 rounded-xl px-4 py-2.5 text-white text-sm focus:outline-none focus:border-blue-500/60 mb-4"
                        placeholder="Enter name..."
                    />
                )}
                <div className="flex gap-2 justify-end">
                    <button onClick={onCancel}
                        className="px-4 py-2 text-sm text-slate-400 hover:text-white hover:bg-white/10 rounded-xl transition-colors">
                        Cancel
                    </button>
                    <button onClick={() => onConfirm(type === 'input' ? value : undefined)}
                        className={`px-4 py-2 text-sm font-medium rounded-xl transition-colors ${danger ? 'bg-red-600 hover:bg-red-500 text-white' : 'bg-blue-600 hover:bg-blue-500 text-white'}`}>
                        {confirmLabel}
                    </button>
                </div>
            </div>
        </div>
    );
}

const ICON_MAP: Record<string, React.ComponentType<any>> = {
    png: Image, jpg: Image, jpeg: Image, gif: Image, webp: Image, svg: Image,
    mp4: Film, mkv: Film, avi: Film, mov: Film,
    mp3: Music, wav: Music, flac: Music, ogg: Music,
    txt: FileText, md: FileText, pdf: FileText, doc: FileText, docx: FileText,
    ts: Code, tsx: Code, js: Code, jsx: Code, rs: Code, py: Code, sh: Code, json: Code, toml: Code, hk: Code, cr: Code,
    zip: Archive, tar: Archive, gz: Archive,
};

function getIcon(e: FileEntry) { return e.isDir ? Folder : ICON_MAP[e.extension?.toLowerCase()] || File; }
function fmtSize(b: number) {
    if (!b) return '—';
    if (b < 1024) return `${b} B`;
    if (b < 1048576) return `${(b / 1024).toFixed(1)} KB`;
    if (b < 1073741824) return `${(b / 1048576).toFixed(1)} MB`;
    return `${(b / 1073741824).toFixed(2)} GB`;
}

const BOOKMARKS = [
    { label: 'Home', path: '', icon: Home },
    { label: 'Downloads', path: '/Downloads', icon: Download },
    { label: 'Documents', path: '/Documents', icon: FileText },
    { label: 'Pictures', path: '/Pictures', icon: Image },
    { label: 'Music', path: '/Music', icon: Music },
    { label: 'Videos', path: '/Videos', icon: Film },
];

const ExplorerApp: React.FC<AppProps> = () => {
    const [path, setPath] = useState('');
    const [entries, setEntries] = useState<FileEntry[]>([]);
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [clipboard, setClipboard] = useState<ClipboardItem | null>(null);
    const [view, setView] = useState<'grid' | 'list'>('grid');
    const [showHidden, setShowHidden] = useState(false);
    const [query, setQuery] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [history, setHistory] = useState<string[]>([]);
    const [histIdx, setHistIdx] = useState(-1);
    const [renaming, setRenaming] = useState<string | null>(null);
    const [renameVal, setRenameVal] = useState('');
    const [dialog, setDialog] = useState<DialogProps | null>(null);
    const [homeDir, setHomeDir] = useState('');
    const renameRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        SystemBridge.executeCommand('echo $HOME').then(r => {
            const h = (typeof r === 'string' ? r : r?.stdout || '').trim();
            setHomeDir(h); go(h, true);
        });
    }, []);

    const run = useCallback(async (cmd: string) => {
        const r = await SystemBridge.executeCommand(cmd);
        return typeof r === 'string' ? r : r?.stdout || '';
    }, []);

    const go = useCallback(async (p: string, addHist = true) => {
        setLoading(true); setError(null); setSelected(new Set()); setQuery('');
        try {
            const out = await run(`ls -la --time-style="+%Y-%m-%d %H:%M" "${p}" 2>&1`);
            if (!out.trim() || out.startsWith('ls: cannot access')) { setError(`Cannot access: ${p}`); setLoading(false); return; }
            const parsed: FileEntry[] = [];
            for (const line of out.split('\n').filter(l => l.trim() && !l.startsWith('total'))) {
                const parts = line.trim().split(/\s+/);
                if (parts.length < 9) continue;
                const name = parts.slice(8).join(' ');
                if (name === '.' || name === '..') continue;
                const isDir = parts[0].startsWith('d');
                const size = parseInt(parts[4]) || 0;
                const ext = name.includes('.') ? name.split('.').pop()! : '';
                parsed.push({ name, path: `${p}/${name}`.replace('//', '/'), isDir, size, modified: `${parts[6]} ${parts[7]}`, extension: ext });
            }
            parsed.sort((a, b) => { if (a.isDir !== b.isDir) return a.isDir ? -1 : 1; return a.name.localeCompare(b.name); });
            setEntries(parsed); setPath(p);
            if (addHist) {
                const next = [...history.slice(0, histIdx + 1), p];
                setHistory(next); setHistIdx(next.length - 1);
            }
        } catch { setError('Failed to read directory'); }
        setLoading(false);
    }, [history, histIdx, run]);

    const goBack    = () => { if (histIdx > 0) { const p = history[histIdx - 1]; setHistIdx(h => h - 1); go(p, false); } };
    const goFwd     = () => { if (histIdx < history.length - 1) { const p = history[histIdx + 1]; setHistIdx(h => h + 1); go(p, false); } };
    const goUp      = () => { const parent = path.split('/').slice(0, -1).join('/') || '/'; go(parent); };
    const goRefresh = () => go(path, false);

    const open = (e: FileEntry) => { e.isDir ? go(e.path) : run(`xdg-open "${e.path}" &`); };

    const sel = useCallback((e: React.MouseEvent, p: string) => {
        if (e.ctrlKey || e.metaKey) {
            setSelected(prev => { const n = new Set(prev); n.has(p) ? n.delete(p) : n.add(p); return n; });
        } else setSelected(new Set([p]));
    }, []);

    const showDialog = (d: Omit<DialogProps, 'onConfirm' | 'onCancel'>) =>
        new Promise<string | boolean | null>(resolve => {
            setDialog({
                ...d,
                onConfirm: (v) => { setDialog(null); resolve(d.type === 'input' ? v ?? '' : true); },
                onCancel: () => { setDialog(null); resolve(null); },
            });
        });

    const newFolder = async () => {
        const name = await showDialog({ type: 'input', title: 'New Folder', defaultValue: 'New Folder' });
        if (!name) return;
        await run(`mkdir -p "${path}/${name}"`); goRefresh();
    };

    const newFile = async () => {
        const name = await showDialog({ type: 'input', title: 'New File', defaultValue: 'new-file.txt' });
        if (!name) return;
        await run(`touch "${path}/${name}"`); goRefresh();
    };

    const doCopy = (paths: string[]) => setClipboard({ paths, mode: 'copy' });
    const doCut  = (paths: string[]) => setClipboard({ paths, mode: 'cut' });

    const doPaste = async () => {
        if (!clipboard) return;
        const srcs = clipboard.paths.map(p => `"${p}"`).join(' ');
        await run(`${clipboard.mode === 'copy' ? 'cp -r' : 'mv'} ${srcs} "${path}/"`);
        if (clipboard.mode === 'cut') setClipboard(null);
        goRefresh();
    };

    const doDelete = async (paths: string[]) => {
        const names = paths.map(p => p.split('/').pop()).join(', ');
        const ok = await showDialog({ type: 'confirm', title: 'Delete files', message: `Delete ${names}?`, confirmLabel: 'Delete', danger: true });
        if (!ok) return;
        await run(`rm -rf ${paths.map(p => `"${p}"`).join(' ')}`);
        setSelected(new Set()); goRefresh();
    };

    const startRename = (e: FileEntry) => { setRenaming(e.path); setRenameVal(e.name); setTimeout(() => renameRef.current?.select(), 50); };
    const finishRename = async () => {
        if (!renaming || !renameVal.trim()) { setRenaming(null); return; }
        const dir = renaming.split('/').slice(0, -1).join('/');
        const np = `${dir}/${renameVal.trim()}`;
        if (np !== renaming) { await run(`mv "${renaming}" "${np}"`); goRefresh(); }
        setRenaming(null);
    };

    const filtered = entries.filter(e => {
        if (!showHidden && e.name.startsWith('.')) return false;
        if (query && !e.name.toLowerCase().includes(query.toLowerCase())) return false;
        return true;
    });

    const selPaths = Array.from(selected);
    const pathParts = path.split('/').filter(Boolean);

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white overflow-hidden relative" onClick={() => setSelected(new Set())}>
            {/* Inline dialog overlay */}
            {dialog && <InlineDialog {...dialog} />}

            {/* Toolbar */}
            <div className="shrink-0 flex items-center gap-1 px-3 py-2 bg-slate-800 border-b border-white/5">
                <button onClick={goBack} disabled={histIdx <= 0} className="p-1.5 hover:bg-white/10 rounded-lg disabled:opacity-30"><ChevronLeft size={16} /></button>
                <button onClick={goFwd} disabled={histIdx >= history.length - 1} className="p-1.5 hover:bg-white/10 rounded-lg disabled:opacity-30"><ChevronRight size={16} /></button>
                <button onClick={goUp} disabled={path === '/'} className="p-1.5 hover:bg-white/10 rounded-lg disabled:opacity-30"><ArrowUp size={16} /></button>
                <button onClick={() => go(homeDir)} className="p-1.5 hover:bg-white/10 rounded-lg"><Home size={16} /></button>
                <button onClick={goRefresh} className="p-1.5 hover:bg-white/10 rounded-lg"><RefreshCw size={14} className={loading ? 'animate-spin' : ''} /></button>

                {/* Breadcrumb */}
                <div className="flex-1 flex items-center gap-0.5 bg-slate-900/50 rounded-lg px-2 py-1 mx-1 overflow-x-auto text-sm">
                    <button onClick={() => go('/')} className="hover:text-blue-400 text-slate-400 shrink-0">/</button>
                    {pathParts.map((part, i) => (
                        <React.Fragment key={i}>
                            <ChevronRight size={12} className="text-slate-600 shrink-0" />
                            <button onClick={() => go('/' + pathParts.slice(0, i + 1).join('/'))} className="hover:text-blue-400 whitespace-nowrap shrink-0">{part}</button>
                        </React.Fragment>
                    ))}
                </div>

                {/* Search */}
                <div className="relative">
                    <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-slate-500" />
                    <input value={query} onChange={e => setQuery(e.target.value)} placeholder="Search..." className="bg-slate-900/50 border border-white/10 rounded-lg pl-7 pr-3 py-1 text-sm w-36 focus:outline-none focus:border-blue-500/50" />
                </div>

                <button onClick={() => setShowHidden(s => !s)} className={`p-1.5 rounded-lg ${showHidden ? 'text-blue-400 bg-blue-500/10' : 'hover:bg-white/10 text-slate-400'}`}>{showHidden ? <Eye size={15} /> : <EyeOff size={15} />}</button>
                <button onClick={() => setView('grid')} className={`p-1.5 rounded-lg ${view === 'grid' ? 'text-blue-400 bg-blue-500/10' : 'hover:bg-white/10 text-slate-400'}`}><Grid size={15} /></button>
                <button onClick={() => setView('list')} className={`p-1.5 rounded-lg ${view === 'list' ? 'text-blue-400 bg-blue-500/10' : 'hover:bg-white/10 text-slate-400'}`}><List size={15} /></button>
                <button onClick={newFolder} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><FolderPlus size={15} /></button>
                <button onClick={newFile} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><Plus size={15} /></button>
                {clipboard && <button onClick={doPaste} className="p-1.5 hover:bg-white/10 rounded-lg text-green-400"><Clipboard size={15} /></button>}
            </div>

            <div className="flex flex-1 overflow-hidden">
                {/* Sidebar */}
                <div className="w-40 shrink-0 bg-slate-800/50 border-r border-white/5 overflow-y-auto py-2">
                    <div className="px-3 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">Bookmarks</div>
                    {BOOKMARKS.map(bm => {
                        const full = bm.path ? homeDir + bm.path : homeDir;
                        const Icon = bm.icon;
                        return (
                            <button key={bm.label} onClick={() => go(full)} className={`w-full flex items-center gap-2 px-3 py-1.5 text-sm transition-colors text-left ${path === full ? 'text-blue-400 bg-blue-500/10' : 'text-slate-300 hover:bg-white/10'}`}>
                                <Icon size={14} className="shrink-0" />{bm.label}
                            </button>
                        );
                    })}
                    <div className="px-3 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider mt-3">Devices</div>
                    <button onClick={() => go('/')} className="w-full flex items-center gap-2 px-3 py-1.5 text-sm text-slate-300 hover:bg-white/10 transition-colors text-left">
                        <HardDrive size={14} className="shrink-0" />Root (/)
                    </button>
                </div>

                {/* Content */}
                <div className="flex-1 overflow-y-auto p-3" onClick={() => setSelected(new Set())}>
                    {error ? (
                        <div className="flex flex-col items-center justify-center h-full text-red-400 gap-2"><X size={32} className="opacity-50" /><p className="text-sm">{error}</p></div>
                    ) : loading ? (
                        <div className="flex items-center justify-center h-full"><RefreshCw size={24} className="animate-spin text-blue-400" /></div>
                    ) : filtered.length === 0 ? (
                        <div className="flex flex-col items-center justify-center h-full text-slate-500"><Folder size={48} className="opacity-20 mb-2" /><p className="text-sm">{query ? 'No results' : 'Empty folder'}</p></div>
                    ) : view === 'grid' ? (
                        <div className="grid grid-cols-[repeat(auto-fill,minmax(88px,1fr))] gap-2">
                            {filtered.map(e => {
                                const Icon = getIcon(e);
                                const isSel = selected.has(e.path);
                                const isRen = renaming === e.path;
                                return (
                                    <div key={e.path} onClick={ev => { ev.stopPropagation(); sel(ev, e.path); }}
                                        onDoubleClick={() => !isRen && open(e)}
                                        className={`flex flex-col items-center gap-1 p-2 rounded-xl cursor-pointer select-none transition-all ${isSel ? 'bg-blue-600/30 ring-1 ring-blue-500/40' : 'hover:bg-white/5'}`}>
                                        <Icon size={34} className={e.isDir ? 'text-blue-400' : 'text-slate-400'} />
                                        {isRen ? (
                                            <input ref={renameRef} value={renameVal} onChange={ev => setRenameVal(ev.target.value)}
                                                onBlur={finishRename} onKeyDown={ev => { if (ev.key === 'Enter') finishRename(); if (ev.key === 'Escape') setRenaming(null); }}
                                                onClick={ev => ev.stopPropagation()}
                                                className="w-full bg-slate-700 text-white text-xs text-center rounded px-1 focus:outline-none" autoFocus />
                                        ) : (
                                            <span className="text-xs text-center break-all line-clamp-2 text-slate-200 w-full">{e.name}</span>
                                        )}
                                    </div>
                                );
                            })}
                        </div>
                    ) : (
                        <table className="w-full text-sm">
                            <thead>
                                <tr className="text-slate-500 text-xs border-b border-white/5">
                                    <th className="text-left py-1 px-2 font-medium">Name</th>
                                    <th className="text-right py-1 px-2 font-medium w-24">Size</th>
                                    <th className="text-right py-1 px-2 font-medium w-36">Modified</th>
                                </tr>
                            </thead>
                            <tbody>
                                {filtered.map(e => {
                                    const Icon = getIcon(e);
                                    const isSel = selected.has(e.path);
                                    const isRen = renaming === e.path;
                                    return (
                                        <tr key={e.path} onClick={ev => { ev.stopPropagation(); sel(ev, e.path); }} onDoubleClick={() => open(e)}
                                            className={`cursor-pointer transition-colors rounded-lg ${isSel ? 'bg-blue-600/20' : 'hover:bg-white/5'}`}>
                                            <td className="py-1 px-2">
                                                <div className="flex items-center gap-2">
                                                    <Icon size={15} className={e.isDir ? 'text-blue-400' : 'text-slate-400'} />
                                                    {isRen ? (
                                                        <input ref={renameRef} value={renameVal} onChange={ev => setRenameVal(ev.target.value)}
                                                            onBlur={finishRename} onKeyDown={ev => { if (ev.key === 'Enter') finishRename(); if (ev.key === 'Escape') setRenaming(null); }}
                                                            onClick={ev => ev.stopPropagation()}
                                                            className="bg-slate-700 text-white text-sm rounded px-1 focus:outline-none w-48" autoFocus />
                                                    ) : <span className="text-slate-200 truncate">{e.name}</span>}
                                                </div>
                                            </td>
                                            <td className="py-1 px-2 text-right text-slate-500 text-xs">{e.isDir ? '—' : fmtSize(e.size)}</td>
                                            <td className="py-1 px-2 text-right text-slate-500 text-xs">{e.modified}</td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    )}
                </div>
            </div>

            {/* Status bar */}
            <div className="shrink-0 flex items-center justify-between px-4 py-1.5 bg-slate-800/50 border-t border-white/5 text-xs text-slate-500">
                <span>{filtered.length} items{selected.size > 0 ? ` · ${selected.size} selected` : ''}</span>
                <div className="flex gap-2 items-center">
                    {clipboard && (
                        <span className={`flex items-center gap-1 ${clipboard.mode === 'cut' ? 'text-orange-400' : 'text-green-400'}`}>
                            {clipboard.paths.length} ready to {clipboard.mode}
                            <button onClick={() => setClipboard(null)}><X size={10} /></button>
                        </span>
                    )}
                    {selPaths.length > 0 && (
                        <div className="flex gap-1">
                            <button onClick={() => doCopy(selPaths)} className="hover:text-white p-0.5" title="Copy"><Copy size={12} /></button>
                            <button onClick={() => doCut(selPaths)} className="hover:text-white p-0.5" title="Cut"><Scissors size={12} /></button>
                            <button onClick={() => { const e = filtered.find(f => f.path === selPaths[0]); if (e) startRename(e); }} className="hover:text-white p-0.5" title="Rename"><Edit3 size={12} /></button>
                            <button onClick={() => doDelete(selPaths)} className="hover:text-red-400 p-0.5" title="Delete"><Trash2 size={12} /></button>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
};
export default ExplorerApp;
