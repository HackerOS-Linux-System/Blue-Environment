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
    RefreshCw, FileCode, FolderOpen
} from 'lucide-react';

interface FileNode {
    name: string;
    path: string;
    type: 'file' | 'directory';
    children?: FileNode[];
    content?: string;
}

const BlueCodeApp: React.FC<AppProps> = ({ windowId }) => {
    const { t } = useLanguage();
    const [rootPath, setRootPath] = useState<string>('/home/user/projects');
    const [fileTree, setFileTree] = useState<FileNode[]>([]);
    const [openFiles, setOpenFiles] = useState<{ path: string; content: string; modified: boolean }[]>([]);
    const [activeFileIndex, setActiveFileIndex] = useState(0);
    const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
    const [showTerminal, setShowTerminal] = useState(true);
    const [terminalReady, setTerminalReady] = useState(false);
    const terminalRef = useRef<HTMLDivElement>(null);
    const terminalInstance = useRef<Terminal | null>(null);
    const fitAddon = useRef<FitAddon | null>(null);
    const editorRef = useRef<any>(null);
    const monacoRef = useRef<Monaco | null>(null);

    // Inicjalizacja terminala
    useEffect(() => {
        if (!showTerminal || !terminalRef.current || terminalInstance.current) return;

        const term = new Terminal({
            cursorBlink: true,
            fontSize: 13,
            theme: {
                background: '#1e1e2e',
                foreground: '#cdd6f4',
                    cursor: '#f5e0dc',
            },
        });
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.loadAddon(new WebLinksAddon());
        term.open(terminalRef.current);
        fit.fit();
        terminalInstance.current = term;
        fitAddon.current = fit;

        // Połącz z backendem Tauri (jeśli istnieje)
        SystemBridge.spawnTerminal(windowId).then(() => {
            // Nasłuchuj wyjścia – poprawiona obsługa zdarzenia
            window.addEventListener('terminal-output', (e: Event) => {
                const customEvent = e as CustomEvent<{ data: string; type?: string }>;
                term.write(customEvent.detail.data);
            });
        });

        term.onData(data => {
            SystemBridge.writeToTerminal(data);
        });

        const resizeObserver = new ResizeObserver(() => fit.fit());
        resizeObserver.observe(terminalRef.current);
        setTerminalReady(true);
        return () => resizeObserver.disconnect();
    }, [showTerminal, windowId]);

    // Załaduj drzewo plików
    useEffect(() => {
        loadFileTree(rootPath);
    }, [rootPath]);

    const loadFileTree = async (path: string) => {
        const files = await SystemBridge.getFiles(path);
        const tree: FileNode[] = [];
        for (const file of files) {
            const node: FileNode = {
                name: file.name,
                path: file.path,
                type: file.is_dir ? 'directory' : 'file',
            };
            if (file.is_dir) {
                node.children = await loadSubTree(file.path);
            }
            tree.push(node);
        }
        setFileTree(tree);
    };

    const loadSubTree = async (dirPath: string): Promise<FileNode[]> => {
        const files = await SystemBridge.getFiles(dirPath);
        const nodes: FileNode[] = [];
        for (const file of files) {
            const node: FileNode = {
                name: file.name,
                path: file.path,
                type: file.is_dir ? 'directory' : 'file',
            };
            if (file.is_dir) {
                node.children = []; // wstępnie puste, będą ładowane na żądanie
            }
            nodes.push(node);
        }
        return nodes;
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

    const handleEditorChange = (value: string | undefined) => {
        if (value === undefined) return;
        setOpenFiles(prev => prev.map((f, i) => i === activeFileIndex ? { ...f, content: value, modified: true } : f));
    };

    const handleEditorMount: OnMount = (editor, monaco) => {
        editorRef.current = editor;
        monacoRef.current = monaco;
        // Ustaw motyw
        monaco.editor.defineTheme('blue-dark', {
            base: 'vs-dark',
            inherit: true,
            rules: [],
            colors: {
                'editor.background': '#0f172a',
                'editor.lineHighlightBackground': '#1e293b',
            },
        });
        monaco.editor.setTheme('blue-dark');
    };

    const renderTree = (nodes: FileNode[], level = 0) => {
        return nodes.map(node => (
            <div key={node.path}>
            <div
            className="flex items-center gap-1 py-1 px-2 rounded cursor-pointer hover:bg-white/5"
            style={{ paddingLeft: `${level * 16 + 8}px` }}
            onClick={() => node.type === 'file' && openFile(node.path)}
            >
            {node.type === 'directory' && (
                <button className="w-4 h-4 flex items-center justify-center">
                {node.children && node.children.length > 0 ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                </button>
            )}
            {node.type === 'directory' ? <Folder size={14} className="text-blue-400" /> : <FileCode size={14} className="text-yellow-400" />}
            <span className="text-sm truncate">{node.name}</span>
            </div>
            {node.type === 'directory' && node.children && node.children.length > 0 && renderTree(node.children, level + 1)}
            </div>
        ));
    };

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
        {/* Pasek narzędzi */}
        <div className="h-12 bg-slate-800 border-b border-white/5 flex items-center px-4 gap-2">
        <button onClick={() => setSidebarCollapsed(!sidebarCollapsed)} className="p-2 hover:bg-white/10 rounded">
        <FolderOpen size={18} />
        </button>
        <button className="p-2 hover:bg-white/10 rounded" title="New File">
        <Plus size={18} />
        </button>
        <button onClick={() => saveFile(activeFileIndex)} className="p-2 hover:bg-white/10 rounded" title="Save">
        <Save size={18} />
        </button>
        <div className="w-px h-6 bg-white/10 mx-2" />
        <button onClick={() => setShowTerminal(!showTerminal)} className={`p-2 rounded ${showTerminal ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}>
        <TerminalIcon size={18} />
        </button>
        <div className="flex-1" />
        <div className="text-xs text-slate-500">
        {openFiles[activeFileIndex]?.path || 'No file'}
        </div>
        </div>

        <div className="flex flex-1 overflow-hidden">
        {/* Sidebar – drzewo plików */}
        {!sidebarCollapsed && (
            <div className="w-64 bg-slate-800/50 border-r border-white/5 overflow-y-auto p-2">
            <div className="flex items-center justify-between mb-2 px-2">
            <span className="text-xs font-semibold text-slate-400">EXPLORER</span>
            <button className="p-1 hover:bg-white/10 rounded" onClick={() => loadFileTree(rootPath)}>
            <RefreshCw size={12} />
            </button>
            </div>
            {renderTree(fileTree)}
            </div>
        )}

        {/* Główny obszar edytora */}
        <div className="flex-1 flex flex-col overflow-hidden">
        {/* Tabs otwartych plików */}
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
            onClick={(e) => {
                e.stopPropagation();
                setOpenFiles(prev => prev.filter((_, i) => i !== idx));
                if (activeFileIndex === idx && idx > 0) setActiveFileIndex(idx - 1);
                else if (activeFileIndex === idx && openFiles.length === 1) setActiveFileIndex(0);
                else if (activeFileIndex > idx) setActiveFileIndex(activeFileIndex - 1);
            }}
            className="p-0.5 hover:bg-white/10 rounded"
            >
            <X size={12} />
            </button>
            </div>
        ))}
        </div>

        {/* Edytor Monaco */}
        <div className="flex-1">
        {openFiles[activeFileIndex] && (
            <Editor
            height="100%"
            language={getLanguageFromPath(openFiles[activeFileIndex].path)}
            value={openFiles[activeFileIndex].content}
            onChange={handleEditorChange}
            onMount={handleEditorMount}
            options={{
                fontSize: 13,
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                automaticLayout: true,
            }}
            />
        )}
        </div>

        {/* Terminal */}
        {showTerminal && (
            <div className="h-48 bg-slate-900 border-t border-white/5">
            <div ref={terminalRef} className="w-full h-full" />
            </div>
        )}
        </div>
        </div>
        </div>
    );
};

function getLanguageFromPath(path: string): string {
    const ext = path.split('.').pop()?.toLowerCase();
    switch (ext) {
        case 'ts': return 'typescript';
        case 'tsx': return 'typescript';
        case 'js': return 'javascript';
        case 'jsx': return 'javascript';
        case 'py': return 'python';
        case 'rs': return 'rust';
        case 'go': return 'go';
        case 'html': return 'html';
        case 'css': return 'css';
        case 'json': return 'json';
        case 'md': return 'markdown';
        default: return 'plaintext';
    }
}

export default BlueCodeApp;
