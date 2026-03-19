import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
    Folder, FileText, HardDrive, ArrowLeft, ArrowRight, RefreshCw,
    Plus, Trash2, Copy, Clipboard, Scissors, Home, Star, Image,
    File, Music, Video, Archive, Code, ChevronRight, ChevronDown,
    Grid, List, Eye, EyeOff, X, Download, Upload, Edit, Save,
    Search, Filter, SortAsc, SortDesc, MoreVertical
} from 'lucide-react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';

interface FileEntry {
    name: string;
    path: string;
    is_dir: boolean;
    size: string;
    mime_type: string;
    modified?: string;
}

interface Bookmark {
    name: string;
    path: string;
    icon?: React.ComponentType<any>;
}

const ExplorerApp: React.FC<AppProps> = () => {
    // State
    const [currentPath, setCurrentPath] = useState('HOME');
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [history, setHistory] = useState<string[]>(['HOME']);
    const [historyIndex, setHistoryIndex] = useState(0);
    const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
    const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
    const [showPreview, setShowPreview] = useState(false);
    const [previewFile, setPreviewFile] = useState<FileEntry | null>(null);
    const [previewContent, setPreviewContent] = useState<string>('');
    const [clipboard, setClipboard] = useState<{ action: 'copy' | 'cut'; files: string[] } | null>(null);
    const [bookmarks, setBookmarks] = useState<Bookmark[]>([
        { name: 'Home', path: 'HOME', icon: Home },
        { name: 'Desktop', path: 'HOME/Desktop', icon: Folder },
        { name: 'Documents', path: 'HOME/Documents', icon: FileText },
        { name: 'Downloads', path: 'HOME/Downloads', icon: Download },
        { name: 'Pictures', path: 'HOME/Pictures', icon: Image },
        { name: 'Music', path: 'HOME/Music', icon: Music },
        { name: 'Videos', path: 'HOME/Videos', icon: Video },
    ]);
    const [showHidden, setShowHidden] = useState(false);
    const [sortBy, setSortBy] = useState<'name' | 'size' | 'modified'>('name');
    const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc');
    const [searchTerm, setSearchTerm] = useState('');
    const [treeExpanded, setTreeExpanded] = useState<Set<string>>(new Set(['HOME']));
    const [treeDirs, setTreeDirs] = useState<Map<string, FileEntry[]>>(new Map());

    const fileGridRef = useRef<HTMLDivElement>(null);

    // Load files
    const loadFiles = async (path: string) => {
        setIsLoading(true);
        try {
            const entries = await SystemBridge.getFiles(path);
            setFiles(entries);
            setCurrentPath(path);
            // Also load subdirectories for tree view (only if expanded)
            if (treeExpanded.has(path)) {
                loadSubdirs(path, entries);
            }
        } catch (e) {
            console.error("Failed to load files", e);
        } finally {
            setIsLoading(false);
        }
    };

    const loadSubdirs = async (path: string, entries?: FileEntry[]) => {
        const dirs = entries ? entries.filter(e => e.is_dir) : await SystemBridge.getFiles(path).then(f => f.filter(e => e.is_dir));
        setTreeDirs(prev => new Map(prev).set(path, dirs));
    };

    useEffect(() => {
        loadFiles('HOME');
    }, []);

    const navigateTo = (path: string) => {
        const newHistory = history.slice(0, historyIndex + 1);
        newHistory.push(path);
        setHistory(newHistory);
        setHistoryIndex(newHistory.length - 1);
        loadFiles(path);
        setSelectedFiles([]);
        setPreviewFile(null);
        setShowPreview(false);
    };

    const handleNavigate = (entry: FileEntry) => {
        if (entry.is_dir) {
            navigateTo(entry.path);
        } else {
            // Preview or open
            if (entry.mime_type.startsWith('image/') || entry.mime_type.startsWith('text/')) {
                previewFileHandler(entry);
            } else {
                // Try to open with default app
                SystemBridge.launchApp(`xdg-open "${entry.path}"`);
            }
        }
    };

    const goBack = () => {
        if (historyIndex > 0) {
            const newIndex = historyIndex - 1;
            setHistoryIndex(newIndex);
            loadFiles(history[newIndex]);
            setSelectedFiles([]);
        }
    };

    const goForward = () => {
        if (historyIndex < history.length - 1) {
            const newIndex = historyIndex + 1;
            setHistoryIndex(newIndex);
            loadFiles(history[newIndex]);
            setSelectedFiles([]);
        }
    };

    const goUp = () => {
        if (currentPath === 'HOME' || currentPath === '/') return;
        const parent = currentPath.split('/').slice(0, -1).join('/') || '/';
        navigateTo(parent);
    };

    const refresh = () => {
        loadFiles(currentPath);
    };

    // Selection
    const toggleSelect = (path: string, e?: React.MouseEvent) => {
        if (e?.ctrlKey) {
            setSelectedFiles(prev =>
            prev.includes(path) ? prev.filter(p => p !== path) : [...prev, path]
            );
        } else if (e?.shiftKey && selectedFiles.length > 0) {
            // Range select
            const indices = files.map(f => f.path);
            const lastIdx = indices.indexOf(selectedFiles[selectedFiles.length - 1]);
            const currentIdx = indices.indexOf(path);
            if (lastIdx !== -1 && currentIdx !== -1) {
                const start = Math.min(lastIdx, currentIdx);
                const end = Math.max(lastIdx, currentIdx);
                const range = indices.slice(start, end + 1);
                setSelectedFiles(range);
            }
        } else {
            setSelectedFiles([path]);
        }
    };

    const selectAll = () => {
        setSelectedFiles(files.map(f => f.path));
    };

    // File operations
    const createFolder = async () => {
        const name = prompt('Nazwa nowego folderu:');
        if (!name) return;
        try {
            // await SystemBridge.createFolder(currentPath, name);
            refresh();
        } catch (e) {
            alert('Błąd tworzenia folderu');
        }
    };

    const deleteFiles = async () => {
        if (selectedFiles.length === 0) return;
        if (!confirm(`Czy na pewno usunąć ${selectedFiles.length} element(ów)?`)) return;
        for (const path of selectedFiles) {
            try {
                // await SystemBridge.deleteFile(path);
            } catch (e) {
                console.error('Błąd usuwania', path);
            }
        }
        refresh();
        setSelectedFiles([]);
    };

    const copyFiles = () => {
        if (selectedFiles.length === 0) return;
        setClipboard({ action: 'copy', files: [...selectedFiles] });
    };

    const cutFiles = () => {
        if (selectedFiles.length === 0) return;
        setClipboard({ action: 'cut', files: [...selectedFiles] });
    };

    const pasteFiles = async () => {
        if (!clipboard) return;
        const destPath = currentPath;
        for (const src of clipboard.files) {
            const fileName = src.split('/').pop() || '';
            const dest = `${destPath}/${fileName}`;
            try {
                if (clipboard.action === 'copy') {
                    // await SystemBridge.copyFile(src, dest);
                } else {
                    // await SystemBridge.moveFile(src, dest);
                }
            } catch (e) {
                console.error('Błąd kopiowania/przenoszenia', src);
            }
        }
        if (clipboard.action === 'cut') {
            setClipboard(null);
        }
        refresh();
    };

    // Preview
    const previewFileHandler = async (file: FileEntry) => {
        setPreviewFile(file);
        setShowPreview(true);
        if (file.mime_type.startsWith('text/')) {
            try {
                const content = await SystemBridge.readFile(file.path);
                setPreviewContent(content);
            } catch (e) {
                setPreviewContent('Nie można odczytać pliku');
            }
        } else if (file.mime_type.startsWith('image/')) {
            // Load image as data URL
            try {
                const data = await SystemBridge.readFileAsDataURL(file.path);
                setPreviewContent(data);
            } catch (e) {
                setPreviewContent('');
            }
        } else {
            setPreviewContent('');
        }
    };

    // Tree view
    const toggleTreeDir = async (path: string) => {
        const newExpanded = new Set(treeExpanded);
        if (newExpanded.has(path)) {
            newExpanded.delete(path);
        } else {
            newExpanded.add(path);
            if (!treeDirs.has(path)) {
                await loadSubdirs(path);
            }
        }
        setTreeExpanded(newExpanded);
    };

    const renderTree = (basePath: string, level = 0) => {
        const dirs = treeDirs.get(basePath) || [];
        return dirs.map(dir => (
            <div key={dir.path}>
            <div
            className={`flex items-center gap-1 py-1 px-2 rounded cursor-pointer hover:bg-white/5 ${
                currentPath === dir.path ? 'bg-blue-600/20 text-blue-400' : ''
            }`}
            style={{ paddingLeft: `${level * 16 + 8}px` }}
            onClick={() => navigateTo(dir.path)}
            >
            <button
            className="w-4 h-4 flex items-center justify-center"
            onClick={(e) => { e.stopPropagation(); toggleTreeDir(dir.path); }}
            >
            {treeExpanded.has(dir.path) ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            </button>
            <Folder size={16} className="text-blue-400" />
            <span className="text-sm truncate">{dir.name}</span>
            </div>
            {treeExpanded.has(dir.path) && renderTree(dir.path, level + 1)}
            </div>
        ));
    };

    // Sorting and filtering
    const filteredFiles = files.filter(f => showHidden || !f.name.startsWith('.')).filter(f => f.name.toLowerCase().includes(searchTerm.toLowerCase()));
    const sortedFiles = [...filteredFiles].sort((a, b) => {
        const dirOrder = (a.is_dir === b.is_dir) ? 0 : a.is_dir ? -1 : 1;
        if (dirOrder !== 0) return dirOrder;
        let compare = 0;
        if (sortBy === 'name') compare = a.name.localeCompare(b.name);
        else if (sortBy === 'size') compare = (parseFloat(a.size) || 0) - (parseFloat(b.size) || 0);
        else if (sortBy === 'modified') compare = (a.modified || '').localeCompare(b.modified || '');
        return sortOrder === 'asc' ? compare : -compare;
    });

    return (
        <div className="flex h-full bg-slate-900 text-white">
        {/* Sidebar */}
        <div className="w-56 bg-slate-800/50 border-r border-white/5 flex flex-col">
        <div className="p-3 border-b border-white/5">
        <h3 className="text-xs font-semibold text-slate-400 uppercase tracking-wider">Miejsca</h3>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {bookmarks.map(bm => (
            <div
            key={bm.path}
            className={`flex items-center gap-2 p-2 rounded cursor-pointer hover:bg-white/5 ${
                currentPath === bm.path ? 'bg-blue-600/20 text-blue-400' : ''
            }`}
            onClick={() => navigateTo(bm.path)}
            >
            {bm.icon ? <bm.icon size={16} /> : <Folder size={16} />}
            <span className="text-sm truncate">{bm.name}</span>
            </div>
        ))}
        <div className="border-t border-white/5 my-2" />
        <div className="p-2">
        <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">System</h4>
        <div className="space-y-1">
        <div className="flex items-center gap-2 p-2 rounded cursor-pointer hover:bg-white/5">
        <HardDrive size={16} />
        <span className="text-sm">/ (root)</span>
        </div>
        </div>
        </div>
        <div className="border-t border-white/5 my-2" />
        <div className="p-2">
        <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Drzewo katalogów</h4>
        <div className="space-y-1">
        <div
        className={`flex items-center gap-1 py-1 px-2 rounded cursor-pointer hover:bg-white/5 ${
            currentPath === 'HOME' ? 'bg-blue-600/20 text-blue-400' : ''
        }`}
        onClick={() => navigateTo('HOME')}
        >
        <button
        className="w-4 h-4 flex items-center justify-center"
        onClick={(e) => { e.stopPropagation(); toggleTreeDir('HOME'); }}
        >
        {treeExpanded.has('HOME') ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </button>
        <Home size={16} className="text-blue-400" />
        <span className="text-sm">Home</span>
        </div>
        {treeExpanded.has('HOME') && renderTree('HOME', 1)}
        </div>
        </div>
        </div>
        </div>

        {/* Main content */}
        <div className="flex-1 flex flex-col">
        {/* Toolbar */}
        <div className="h-12 bg-slate-800 border-b border-white/5 flex items-center px-2 gap-1">
        <button onClick={goBack} disabled={historyIndex === 0} className="p-2 hover:bg-white/10 rounded disabled:opacity-30">
        <ArrowLeft size={18} />
        </button>
        <button onClick={goForward} disabled={historyIndex === history.length - 1} className="p-2 hover:bg-white/10 rounded disabled:opacity-30">
        <ArrowRight size={18} />
        </button>
        <button onClick={goUp} className="p-2 hover:bg-white/10 rounded">
        <HardDrive size={18} />
        </button>
        <button onClick={refresh} className="p-2 hover:bg-white/10 rounded">
        <RefreshCw size={18} className={isLoading ? 'animate-spin' : ''} />
        </button>
        <div className="w-px h-6 bg-white/10 mx-2" />
        <button onClick={createFolder} className="p-2 hover:bg-white/10 rounded" title="Nowy folder">
        <Plus size={18} />
        </button>
        <button onClick={deleteFiles} disabled={selectedFiles.length === 0} className="p-2 hover:bg-white/10 rounded disabled:opacity-30" title="Usuń">
        <Trash2 size={18} />
        </button>
        <button onClick={copyFiles} disabled={selectedFiles.length === 0} className="p-2 hover:bg-white/10 rounded disabled:opacity-30" title="Kopiuj">
        <Copy size={18} />
        </button>
        <button onClick={cutFiles} disabled={selectedFiles.length === 0} className="p-2 hover:bg-white/10 rounded disabled:opacity-30" title="Wytnij">
        <Scissors size={18} />
        </button>
        <button onClick={pasteFiles} disabled={!clipboard} className="p-2 hover:bg-white/10 rounded disabled:opacity-30" title="Wklej">
        <Clipboard size={18} />
        </button>
        <div className="w-px h-6 bg-white/10 mx-2" />
        <div className="flex items-center bg-slate-900 rounded px-2 py-1 flex-1 max-w-md">
        <Search size={14} className="text-slate-500 mr-2" />
        <input
        type="text"
        placeholder="Szukaj..."
        className="bg-transparent text-sm text-white flex-1 focus:outline-none"
        value={searchTerm}
        onChange={(e) => setSearchTerm(e.target.value)}
        />
        </div>
        <div className="flex items-center gap-1">
        <button onClick={() => setShowHidden(!showHidden)} className={`p-2 rounded ${showHidden ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Pokaż ukryte">
        <Eye size={18} />
        </button>
        <button onClick={() => setViewMode('grid')} className={`p-2 rounded ${viewMode === 'grid' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}>
        <Grid size={18} />
        </button>
        <button onClick={() => setViewMode('list')} className={`p-2 rounded ${viewMode === 'list' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}>
        <List size={18} />
        </button>
        <button onClick={() => setShowPreview(!showPreview)} className={`p-2 rounded ${showPreview ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Podgląd">
        <Eye size={18} />
        </button>
        </div>
        </div>

        {/* Breadcrumbs */}
        <div className="h-8 bg-slate-800/50 border-b border-white/5 flex items-center px-3 text-sm">
        {currentPath.split('/').map((part, i, arr) => {
            const path = arr.slice(0, i + 1).join('/');
            return (
                <React.Fragment key={i}>
                {i > 0 && <ChevronRight size={12} className="mx-1 text-slate-600" />}
                <button
                className="hover:text-blue-400 transition-colors"
                onClick={() => navigateTo(path === '' ? '/' : path)}
                >
                {part === 'HOME' ? '🏠' : part}
                </button>
                </React.Fragment>
            );
        })}
        </div>

        {/* File view + Preview */}
        <div className="flex-1 flex overflow-hidden">
        <div className={`flex-1 overflow-auto p-2 ${showPreview ? 'w-2/3' : 'w-full'}`} ref={fileGridRef}>
        {viewMode === 'grid' ? (
            <div className="grid grid-cols-4 md:grid-cols-6 lg:grid-cols-8 gap-2">
            {sortedFiles.map(file => (
                <div
                key={file.path}
                className={`relative flex flex-col items-center p-2 rounded-lg cursor-pointer transition-colors group ${
                    selectedFiles.includes(file.path) ? 'bg-blue-600/30 ring-2 ring-blue-500/50' : 'hover:bg-white/5'
                }`}
                onClick={(e) => toggleSelect(file.path, e)}
                onDoubleClick={() => handleNavigate(file)}
                >
                <div className="mb-2">
                {file.is_dir ? (
                    <Folder size={48} className="text-blue-400" />
                ) : file.mime_type.startsWith('image/') ? (
                    <Image size={48} className="text-green-400" />
                ) : file.mime_type.startsWith('text/') ? (
                    <FileText size={48} className="text-yellow-400" />
                ) : file.mime_type.startsWith('audio/') ? (
                    <Music size={48} className="text-purple-400" />
                ) : file.mime_type.startsWith('video/') ? (
                    <Video size={48} className="text-red-400" />
                ) : (
                    <File size={48} className="text-slate-500" />
                )}
                </div>
                <span className="text-xs text-center break-all line-clamp-2">{file.name}</span>
                <span className="text-[10px] text-slate-500">{file.size}</span>
                </div>
            ))}
            </div>
        ) : (
            <table className="w-full text-sm">
            <thead className="bg-slate-800 sticky top-0">
            <tr>
            <th className="p-2 text-left">Nazwa</th>
            <th className="p-2 text-left">Rozmiar</th>
            <th className="p-2 text-left">Data modyfikacji</th>
            <th className="p-2 text-left">Typ</th>
            </tr>
            </thead>
            <tbody>
            {sortedFiles.map(file => (
                <tr
                key={file.path}
                className={`border-b border-white/5 cursor-pointer hover:bg-white/5 ${
                    selectedFiles.includes(file.path) ? 'bg-blue-600/20' : ''
                }`}
                onClick={(e) => toggleSelect(file.path, e)}
                onDoubleClick={() => handleNavigate(file)}
                >
                <td className="p-2 flex items-center gap-2">
                {file.is_dir ? <Folder size={16} className="text-blue-400" /> : <File size={16} />}
                {file.name}
                </td>
                <td className="p-2">{file.size}</td>
                <td className="p-2">{file.modified || '-'}</td>
                <td className="p-2">{file.mime_type}</td>
                </tr>
            ))}
            </tbody>
            </table>
        )}
        </div>

        {/* Preview panel */}
        {showPreview && previewFile && (
            <div className="w-1/3 border-l border-white/5 bg-slate-800/50 flex flex-col">
            <div className="p-3 border-b border-white/5 flex items-center justify-between">
            <h3 className="font-semibold">Podgląd</h3>
            <button onClick={() => setShowPreview(false)} className="p-1 hover:bg-white/10 rounded">
            <X size={16} />
            </button>
            </div>
            <div className="flex-1 overflow-auto p-3">
            <div className="mb-3 flex items-center gap-2">
            {previewFile.is_dir ? <Folder size={32} /> : <File size={32} />}
            <div>
            <div className="font-medium">{previewFile.name}</div>
            <div className="text-xs text-slate-400">{previewFile.size}</div>
            </div>
            </div>
            {previewFile.mime_type.startsWith('image/') && previewContent ? (
                <img src={previewContent} alt="preview" className="max-w-full max-h-96 object-contain mx-auto" />
            ) : previewFile.mime_type.startsWith('text/') ? (
                <pre className="text-xs bg-slate-900 p-2 rounded overflow-auto max-h-96">{previewContent}</pre>
            ) : (
                <div className="text-center text-slate-500 py-8">Brak podglądu</div>
            )}
            </div>
            </div>
        )}
        </div>
        </div>
        </div>
    );
};

export default ExplorerApp;
