import React, { useState, useEffect, useRef, useCallback } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import { useLanguage } from '../../contexts/LanguageContext';
import Editor, { Monaco, OnMount } from '@monaco-editor/react';
import { Terminal } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import { WebLinksAddon } from 'xterm-addon-web-links';
import 'xterm/css/xterm.css';
import {
    Folder, File, ChevronRight, ChevronDown, Search, GitBranch,
    Settings, Terminal as TerminalIcon, Save, Plus, X, Code2,
    RefreshCw, FileCode, FolderOpen, Trash2, Edit, Copy, Scissors,
    Command, Check, AlertCircle
} from 'lucide-react';

interface FileNode {
    name: string;
    path: string;
    type: 'file' | 'directory';
    children?: FileNode[];
    expanded?: boolean;
}

interface SearchResult {
    file: string;
    line: number;
    content: string;
}

// Komponent terminala (wydzielony dla lepszego zarządzania)
const TerminalComponent: React.FC<{ windowId: string }> = ({ windowId }) => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const terminalInstance = useRef<Terminal | null>(null);
    const fitAddon = useRef<FitAddon | null>(null);

    useEffect(() => {
        if (!terminalRef.current || terminalInstance.current) return;

        const term = new Terminal({
            cursorBlink: true,
            fontSize: 13,
            fontFamily: 'monospace',
            theme: {
                background: '#0f172a',
                foreground: '#e2e8f0',
                    cursor: '#60a5fa',
            },
        });
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.loadAddon(new WebLinksAddon());
        term.open(terminalRef.current);
        fit.fit();
        terminalInstance.current = term;
        fitAddon.current = fit;

        // Obsługa resize
        const resizeObserver = new ResizeObserver(() => fit.fit());
        resizeObserver.observe(terminalRef.current);

        // Uruchom terminal przez backend
        SystemBridge.spawnTerminal(windowId).then((res) => {
            if (!res.success) {
                term.writeln(`\x1b[31m[Error] ${res.error || 'Cannot start terminal'}\x1b[0m`);
                term.writeln('You can still use the editor.');
            } else {
                term.writeln('\x1b[32mTerminal ready.\x1b[0m');
            }
        });

        // Nasłuchuj danych z backendu
        const handleOutput = (e: Event) => {
            const customEvent = e as CustomEvent<{ data: string; type?: string }>;
            term.write(customEvent.detail.data);
        };
        window.addEventListener('terminal-output', handleOutput);

        // Przekaż dane do backendu
        term.onData(data => {
            SystemBridge.writeToTerminal(data);
        });

        return () => {
            resizeObserver.disconnect();
            window.removeEventListener('terminal-output', handleOutput);
            term.dispose();
            terminalInstance.current = null;
        };
    }, [windowId]);

    return <div ref={terminalRef} className="w-full h-full" />;
};

const BlueCodeApp: React.FC<AppProps> = ({ windowId }) => {
    const { t } = useLanguage();
    // Ścieżka główna – domyślnie katalog domowy
    const [rootPath, setRootPath] = useState<string>('');
    const [fileTree, setFileTree] = useState<FileNode[]>([]);
    const [openFiles, setOpenFiles] = useState<{ path: string; content: string; modified: boolean }[]>([]);
    const [activeFileIndex, setActiveFileIndex] = useState(0);
    const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
    const [showTerminal, setShowTerminal] = useState(true);
    const [isLoading, setIsLoading] = useState(false);
    const [searchTerm, setSearchTerm] = useState('');
    const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
    const [showSearch, setShowSearch] = useState(false);
    const [showCommandPalette, setShowCommandPalette] = useState(false);
    const [commandInput, setCommandInput] = useState('');
    const [editorTheme, setEditorTheme] = useState('blue-dark');
    const [fontSize, setFontSize] = useState(13);
    const editorRef = useRef<any>(null);
    const monacoRef = useRef<Monaco | null>(null);

    // Wczytaj katalog domowy
    useEffect(() => {
        const init = async () => {
            const desktop = await SystemBridge.getDefaultDesktopPath();
            const home = desktop.replace('/Desktop', '');
            setRootPath(home);
            await loadFileTree(home);
        };
        init();
    }, []);

    // Ładowanie drzewa plików
    const loadFileTree = async (path: string) => {
        setIsLoading(true);
        try {
            const files = await SystemBridge.getFiles(path);
            const tree: FileNode[] = [];
            for (const file of files) {
                tree.push({
                    name: file.name,
                    path: file.path,
                    type: file.is_dir ? 'directory' : 'file',
                    expanded: false,
                    children: file.is_dir ? [] : undefined,
                });
            }
            setFileTree(tree);
        } catch (err) {
            console.error(err);
        } finally {
            setIsLoading(false);
        }
    };

    // Lazy load katalogu
    const loadDirectoryChildren = async (node: FileNode) => {
        if (node.type !== 'directory') return;
        if (node.children && node.children.length > 0) return;
        const files = await SystemBridge.getFiles(node.path);
        const children: FileNode[] = [];
        for (const file of files) {
            children.push({
                name: file.name,
                path: file.path,
                type: file.is_dir ? 'directory' : 'file',
                children: file.is_dir ? [] : undefined,
                expanded: false,
            });
        }
        node.children = children;
        setFileTree([...fileTree]);
    };

    const toggleDirectory = async (node: FileNode) => {
        if (node.type !== 'directory') return;
        node.expanded = !node.expanded;
        if (node.expanded && (!node.children || node.children.length === 0)) {
            await loadDirectoryChildren(node);
        } else {
            setFileTree([...fileTree]);
        }
    };

    const openFile = async (path: string) => {
        const existing = openFiles.find(f => f.path === path);
        if (existing) {
            setActiveFileIndex(openFiles.indexOf(existing));
            return;
        }
        const content = await SystemBridge.readFile(path);
        setOpenFiles(prev => [...prev, { path, content, modified: false }]);
        setActiveFileIndex(openFiles.length);
    };

    const saveFile = async (index: number) => {
        const file = openFiles[index];
        if (!file) return;
        await SystemBridge.writeFile(file.path, file.content);
        setOpenFiles(prev => prev.map((f, i) => i === index ? { ...f, modified: false } : f));
    };

    const saveAll = async () => {
        for (let i = 0; i < openFiles.length; i++) {
            if (openFiles[i].modified) await saveFile(i);
        }
    };

    const closeFile = (index: number) => {
        if (openFiles.length === 1) return;
        const newFiles = openFiles.filter((_, i) => i !== index);
        setOpenFiles(newFiles);
        if (activeFileIndex >= newFiles.length) setActiveFileIndex(Math.max(0, newFiles.length - 1));
        else if (activeFileIndex === index) setActiveFileIndex(0);
    };

        const newFile = async () => {
            const name = prompt('Enter file name (with extension):');
            if (!name) return;
            const path = `${rootPath}/${name}`;
            await SystemBridge.writeFile(path, '');
            await loadFileTree(rootPath);
            await openFile(path);
        };

        const deleteItem = async (node: FileNode) => {
            if (!confirm(`Delete ${node.name}?`)) return;
            await SystemBridge.deleteFile(node.path);
            await loadFileTree(rootPath);
            const idx = openFiles.findIndex(f => f.path === node.path);
            if (idx !== -1) closeFile(idx);
        };

            const renameItem = async (node: FileNode) => {
                const newName = prompt('New name:', node.name);
                if (!newName || newName === node.name) return;
                const newPath = node.path.replace(node.name, newName);
                await SystemBridge.moveFile(node.path, newPath);
                await loadFileTree(rootPath);
                // Zaktualizuj otwarte pliki
                const idx = openFiles.findIndex(f => f.path === node.path);
                if (idx !== -1) {
                    const content = openFiles[idx].content;
                    setOpenFiles(prev => prev.map((f, i) => i === idx ? { ...f, path: newPath, content } : f));
                }
            };

            const copyItem = async (node: FileNode) => {
                // proste kopiowanie – w praktyce lepiej użyć schowka systemowego
                const newPath = node.path + '.copy';
                await SystemBridge.copyFile(node.path, newPath);
                await loadFileTree(rootPath);
            };

            const refreshExplorer = () => loadFileTree(rootPath);

            // Wyszukiwanie w plikach (proste, przeszukuje wszystkie pliki w katalogu)
            const searchInFiles = async () => {
                if (!searchTerm.trim()) return;
                const results: SearchResult[] = [];
                const searchDir = async (dir: string) => {
                    const files = await SystemBridge.getFiles(dir);
                    for (const file of files) {
                        if (file.is_dir) {
                            await searchDir(file.path);
                        } else {
                            const content = await SystemBridge.readFile(file.path);
                            const lines = content.split('\n');
                            for (let i = 0; i < lines.length; i++) {
                                if (lines[i].toLowerCase().includes(searchTerm.toLowerCase())) {
                                    results.push({ file: file.path, line: i + 1, content: lines[i].trim() });
                                }
                            }
                        }
                    }
                };
                await searchDir(rootPath);
                setSearchResults(results);
                setShowSearch(true);
            };

            // Paleta komend
            const commands = [
                { id: 'save', label: 'Save File', action: () => saveFile(activeFileIndex) },
                { id: 'saveAll', label: 'Save All Files', action: saveAll },
                { id: 'newFile', label: 'New File', action: newFile },
                { id: 'toggleTerminal', label: 'Toggle Terminal', action: () => setShowTerminal(!showTerminal) },
                { id: 'toggleSidebar', label: 'Toggle Sidebar', action: () => setSidebarCollapsed(!sidebarCollapsed) },
                { id: 'refreshExplorer', label: 'Refresh Explorer', action: refreshExplorer },
                { id: 'searchFiles', label: 'Search in Files', action: () => setShowSearch(true) },
            ];

            const executeCommand = (cmd: typeof commands[0]) => {
                cmd.action();
                setShowCommandPalette(false);
            };

            // Edytor – obsługa zmian
            const handleEditorChange = (value: string | undefined) => {
                if (value === undefined) return;
                setOpenFiles(prev => prev.map((f, i) =>
                i === activeFileIndex ? { ...f, content: value, modified: true } : f
                ));
            };

            // Konfiguracja Monaco
            const handleEditorMount: OnMount = (editor, monaco) => {
                editorRef.current = editor;
                monacoRef.current = monaco;
                // Dodaj skróty klawiszowe
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => saveFile(activeFileIndex));
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyS, () => saveAll());
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyP, () => setShowCommandPalette(true));
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyF, () => editor.getAction('actions.find')?.run());
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyF, () => setShowSearch(true));

                // Motywy
                monaco.editor.defineTheme('blue-dark', {
                    base: 'vs-dark',
                    inherit: true,
                    rules: [],
                    colors: {
                        'editor.background': '#0f172a',
                        'editor.lineHighlightBackground': '#1e293b',
                        'editorLineNumber.foreground': '#475569',
                        'editorLineNumber.activeForeground': '#94a3b8',
                    },
                });
                monaco.editor.defineTheme('blue-light', {
                    base: 'vs',
                    inherit: true,
                    rules: [],
                    colors: {
                        'editor.background': '#f8fafc',
                        'editor.foreground': '#0f172a',
                        'editorLineNumber.foreground': '#cbd5e1',
                    },
                });
                monaco.editor.setTheme(editorTheme);
            };

            // Zmiana motywu edytora
            useEffect(() => {
                if (monacoRef.current) {
                    monacoRef.current.editor.setTheme(editorTheme);
                }
            }, [editorTheme]);

            // Rysowanie drzewa plików
            const renderTree = (nodes: FileNode[], level = 0) => {
                return nodes.map(node => (
                    <div key={node.path}>
                    <div
                    className="flex items-center gap-1 py-1 px-2 rounded cursor-pointer hover:bg-white/5 group"
                    style={{ paddingLeft: `${level * 16 + 8}px` }}
                    onDoubleClick={() => node.type === 'file' && openFile(node.path)}
                    onContextMenu={(e) => {
                        e.preventDefault();
                        const choice = prompt('Action: delete, rename, copy', '');
                        if (choice === 'delete') deleteItem(node);
                        else if (choice === 'rename') renameItem(node);
                        else if (choice === 'copy') copyItem(node);
                    }}
                    >
                    {node.type === 'directory' && (
                        <button
                        className="w-4 h-4 flex items-center justify-center"
                        onClick={(e) => { e.stopPropagation(); toggleDirectory(node); }}
                        >
                        {node.expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                        </button>
                    )}
                    {node.type === 'directory' ? (
                        <Folder size={14} className="text-blue-400" />
                    ) : (
                        <FileCode size={14} className="text-yellow-400" />
                    )}
                    <span className="text-sm truncate flex-1">{node.name}</span>
                    <div className="hidden group-hover:flex gap-1">
                    {node.type === 'file' && (
                        <button onClick={(e) => { e.stopPropagation(); openFile(node.path); }} className="p-0.5 hover:bg-white/10 rounded">
                        <FileCode size={12} />
                        </button>
                    )}
                    <button onClick={(e) => { e.stopPropagation(); renameItem(node); }} className="p-0.5 hover:bg-white/10 rounded">
                    <Edit size={12} />
                    </button>
                    <button onClick={(e) => { e.stopPropagation(); deleteItem(node); }} className="p-0.5 hover:bg-white/10 rounded">
                    <Trash2 size={12} />
                    </button>
                    </div>
                    </div>
                    {node.type === 'directory' && node.expanded && node.children && node.children.length > 0 && renderTree(node.children, level + 1)}
                    </div>
                ));
            };

            const activeFile = openFiles[activeFileIndex];

            return (
                <div className="flex flex-col h-full bg-slate-900 text-white relative">
                {/* Pasek narzędzi */}
                <div className="h-12 bg-slate-800 border-b border-white/5 flex items-center px-4 gap-2">
                <button onClick={() => setSidebarCollapsed(!sidebarCollapsed)} className="p-2 hover:bg-white/10 rounded" title="Toggle sidebar">
                <FolderOpen size={18} />
                </button>
                <button onClick={newFile} className="p-2 hover:bg-white/10 rounded" title="New File">
                <Plus size={18} />
                </button>
                <button onClick={() => saveFile(activeFileIndex)} className="p-2 hover:bg-white/10 rounded" title="Save">
                <Save size={18} />
                </button>
                <button onClick={saveAll} className="p-2 hover:bg-white/10 rounded" title="Save All">
                <Save size={18} className="text-blue-400" />
                </button>
                <div className="w-px h-6 bg-white/10 mx-2" />
                <button onClick={() => setShowTerminal(!showTerminal)} className={`p-2 rounded ${showTerminal ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Toggle terminal">
                <TerminalIcon size={18} />
                </button>
                <button onClick={() => setShowSearch(true)} className="p-2 hover:bg-white/10 rounded" title="Search">
                <Search size={18} />
                </button>
                <button onClick={() => setShowCommandPalette(true)} className="p-2 hover:bg-white/10 rounded" title="Command Palette">
                <Command size={18} />
                </button>
                <div className="flex-1" />
                <div className="text-xs text-slate-500">
                {activeFile?.path || 'No file'}
                </div>
                <button onClick={refreshExplorer} className="p-2 hover:bg-white/10 rounded" title="Refresh">
                <RefreshCw size={16} />
                </button>
                </div>

                <div className="flex flex-1 overflow-hidden">
                {/* Sidebar – eksplorer plików */}
                {!sidebarCollapsed && (
                    <div className="w-64 bg-slate-800/50 border-r border-white/5 overflow-y-auto p-2">
                    <div className="flex items-center justify-between mb-2 px-2">
                    <span className="text-xs font-semibold text-slate-400">EXPLORER</span>
                    <button className="p-1 hover:bg-white/10 rounded" onClick={refreshExplorer} title="Refresh">
                    <RefreshCw size={12} />
                    </button>
                    </div>
                    {isLoading && <div className="text-center py-4 text-slate-500">Loading...</div>}
                    {!isLoading && fileTree.length === 0 && <div className="text-center py-4 text-slate-500">No files</div>}
                    {renderTree(fileTree)}
                    </div>
                )}

                {/* Główny obszar edytora */}
                <div className="flex-1 flex flex-col overflow-hidden">
                {/* Tabs */}
                <div className="flex bg-slate-800 border-b border-white/5 overflow-x-auto">
                {openFiles.map((file, idx) => (
                    <div
                    key={idx}
                    onClick={() => setActiveFileIndex(idx)}
                    className={`flex items-center gap-2 px-4 py-2 cursor-pointer border-b-2 transition-colors ${
                        activeFileIndex === idx
                        ? 'border-blue-500 text-white bg-slate-900'
                        : 'border-transparent text-slate-400 hover:text-white'
                    }`}
                    >
                    <FileCode size={14} />
                    <span className="text-sm truncate max-w-[150px]">{file.path.split('/').pop()}</span>
                    {file.modified && <span className="w-2 h-2 bg-yellow-400 rounded-full" />}
                    <button
                    onClick={(e) => { e.stopPropagation(); closeFile(idx); }}
                    className="p-0.5 hover:bg-white/10 rounded"
                    >
                    <X size={12} />
                    </button>
                    </div>
                ))}
                </div>

                {/* Edytor Monaco */}
                <div className="flex-1">
                {activeFile && (
                    <Editor
                    height="100%"
                    language={getLanguageFromPath(activeFile.path)}
                    value={activeFile.content}
                    onChange={handleEditorChange}
                    onMount={handleEditorMount}
                    options={{
                        fontSize,
                        minimap: { enabled: false },
                        scrollBeyondLastLine: false,
                        automaticLayout: true,
                        fontFamily: 'JetBrains Mono, monospace',
                        renderWhitespace: 'boundary',
                    }}
                    />
                )}
                </div>

                {/* Terminal */}
                {showTerminal && (
                    <div className="h-48 bg-slate-900 border-t border-white/5">
                    <TerminalComponent windowId={windowId} />
                    </div>
                )}
                </div>
                </div>

                {/* Pasek statusu */}
                <div className="h-6 bg-slate-800 border-t border-white/5 flex items-center px-2 text-xs text-slate-400">
                <div className="flex items-center gap-2">
                <span>{activeFile ? getLanguageFromPath(activeFile.path).toUpperCase() : 'PLAINTEXT'}</span>
                <span>|</span>
                <span>Ln {editorRef.current?.getPosition()?.lineNumber || 1}, Col {editorRef.current?.getPosition()?.column || 1}</span>
                </div>
                <div className="flex-1" />
                <div className="flex gap-2">
                <button onClick={() => setEditorTheme(editorTheme === 'blue-dark' ? 'blue-light' : 'blue-dark')} className="hover:text-white">
                {editorTheme === 'blue-dark' ? '🌙' : '☀️'}
                </button>
                <button onClick={() => setFontSize(Math.max(8, fontSize - 2))}>A-</button>
                <span>{fontSize}px</span>
                <button onClick={() => setFontSize(Math.min(32, fontSize + 2))}>A+</button>
                </div>
                </div>

                {/* Panel wyszukiwania w plikach */}
                {showSearch && (
                    <div className="absolute inset-0 bg-black/50 flex items-start justify-center z-50 pt-20">
                    <div className="bg-slate-800 rounded-xl w-96 max-w-full p-4 shadow-2xl">
                    <div className="flex items-center gap-2 mb-4">
                    <Search size={16} />
                    <input
                    type="text"
                    placeholder="Search term..."
                    className="flex-1 bg-slate-900 border border-white/10 rounded px-3 py-1 text-sm text-white"
                    value={searchTerm}
                    onChange={(e) => setSearchTerm(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && searchInFiles()}
                    autoFocus
                    />
                    <button onClick={() => setShowSearch(false)} className="p-1 hover:bg-white/10 rounded">
                    <X size={14} />
                    </button>
                    </div>
                    <button onClick={searchInFiles} className="w-full bg-blue-600 hover:bg-blue-500 py-1 rounded text-sm mb-4">
                    Search
                    </button>
                    <div className="max-h-96 overflow-y-auto space-y-2">
                    {searchResults.map((res, idx) => (
                        <div key={idx} className="text-xs border-b border-white/10 pb-1">
                        <div className="text-blue-400">{res.file}:{res.line}</div>
                        <div className="text-slate-300 truncate">{res.content}</div>
                        </div>
                    ))}
                    {searchResults.length === 0 && searchTerm && <div className="text-center text-slate-500">No results</div>}
                    </div>
                    </div>
                    </div>
                )}

                {/* Paleta komend */}
                {showCommandPalette && (
                    <div className="absolute inset-0 bg-black/50 flex items-center justify-center z-50">
                    <div className="bg-slate-800 rounded-xl w-96 max-w-full p-4 shadow-2xl">
                    <div className="flex items-center gap-2 mb-4">
                    <Command size={16} />
                    <input
                    type="text"
                    placeholder="Type a command..."
                    className="flex-1 bg-slate-900 border border-white/10 rounded px-3 py-1 text-sm text-white"
                    value={commandInput}
                    onChange={(e) => setCommandInput(e.target.value)}
                    autoFocus
                    />
                    <button onClick={() => setShowCommandPalette(false)} className="p-1 hover:bg-white/10 rounded">
                    <X size={14} />
                    </button>
                    </div>
                    <div className="max-h-64 overflow-y-auto">
                    {commands
                        .filter(cmd => cmd.label.toLowerCase().includes(commandInput.toLowerCase()))
                        .map(cmd => (
                            <button
                            key={cmd.id}
                            onClick={() => executeCommand(cmd)}
                            className="w-full text-left px-3 py-2 hover:bg-white/10 rounded text-sm"
                            >
                            {cmd.label}
                            </button>
                        ))}
                        </div>
                        </div>
                        </div>
                )}
                </div>
            );
};

function getLanguageFromPath(path: string): string {
    const ext = path.split('.').pop()?.toLowerCase();
    const map: Record<string, string> = {
        // Web
        html: 'html', css: 'css', js: 'javascript', jsx: 'javascript', ts: 'typescript', tsx: 'typescript',
        // Backend & system
        rs: 'rust', go: 'go', py: 'python', rb: 'ruby', pl: 'perl', php: 'php', lua: 'lua', nim: 'nim', vala: 'vala',
        // C family
        c: 'c', cpp: 'cpp', h: 'c', hpp: 'cpp',
        // Mobile & desktop
        dart: 'dart', qml: 'qml', slint: 'slint',
        // Data & config
        json: 'json', yaml: 'yaml', yml: 'yaml', toml: 'toml', xml: 'xml',
        // Build
        meson: 'meson', cmake: 'cmake', sh: 'shell', bash: 'shell', zsh: 'shell',
        // Others
        zig: 'zig', odin: 'odin', cr: 'crystal', nimble: 'nim',
    };
    return map[ext || ''] || 'plaintext';
}

export default BlueCodeApp;
