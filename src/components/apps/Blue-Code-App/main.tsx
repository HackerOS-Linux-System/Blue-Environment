import React, { useState, useEffect, useRef, useCallback } from "react";
import { AppProps } from "../../../types";
import { GitPanel } from "../../GitPanel";
import { SystemBridge } from "../../../utils/systemBridge";
import { useLanguage } from "../../../contexts/LanguageContext";
import Editor, { OnMount, Monaco } from "@monaco-editor/react";
import {
    Folder, File, ChevronRight, ChevronDown, Search, GitBranch,
    Settings, Terminal as TerminalIcon, Save, Plus, X, Code2,
    RefreshCw, FileCode, FolderOpen, Trash2, Edit, Command,
    Check, AlertCircle, Circle, Zap,
} from "lucide-react";

interface FileNode {
    name: string; path: string; type: "file" | "directory";
    children?: FileNode[]; expanded?: boolean;
}
interface SearchResult { file: string; line: number; content: string; }
interface Diagnostic { file: string; line: number; col: number; message: string; severity: "error" | "warning" | "info"; }

const TerminalPane: React.FC<{ windowId: string }> = ({ windowId }) => {
    const ref = useRef<HTMLDivElement>(null);
    const termRef = useRef<any>(null);

    useEffect(() => {
        if (!ref.current || termRef.current) return;

        const init = async () => {
            try {
                const { Terminal } = await import("xterm");
                const { FitAddon } = await import("xterm-addon-fit");
                const { WebLinksAddon } = await import("xterm-addon-web-links");

                const term = new Terminal({
                    cursorBlink: true, fontSize: 13,
                    fontFamily: "JetBrains Mono, monospace",
                    theme: { background: "#0f172a", foreground: "#e2e8f0", cursor: "#60a5fa" },
                });
                const fit = new FitAddon();
                term.loadAddon(fit);
                term.loadAddon(new WebLinksAddon());
                term.open(ref.current!);
                fit.fit();
                termRef.current = { term, fit };

                const ro = new ResizeObserver(() => fit.fit());
                ro.observe(ref.current!);

                const res = await SystemBridge.spawnTerminal(windowId);
                if (!res.success) {
                    term.writeln(`\x1b[33m[No PTY — limited mode]\x1b[0m`);
                    term.write("$ ");
                    let buf = "";
                    term.onKey(({ key, domEvent }: any) => {
                        if (domEvent.keyCode === 13) {
                            SystemBridge.executeCommand(buf).then(r => {
                                term.writeln(""); term.writeln(r.stdout || r.stderr || ""); term.write("$ ");
                            });
                            buf = "";
                        } else if (domEvent.keyCode === 8) {
                            if (buf.length) { buf = buf.slice(0, -1); term.write("\b \b"); }
                        } else {
                            buf += key; term.write(key);
                        }
                    });
                } else {
                    term.writeln("\x1b[32mTerminal ready.\x1b[0m");
                    term.onData((d: string) => SystemBridge.writeToTerminal(d));
                }

                window.addEventListener("terminal-output", ((e: CustomEvent) => {
                    term.write(e.detail.data);
                }) as EventListener);
            } catch (e) {
                if (ref.current) ref.current.innerHTML = '<div class="p-4 text-slate-500 text-xs">xterm not available</div>';
            }
        };
        init();
    }, [windowId]);

    return <div ref={ref} className="w-full h-full" />;
};

const BlueCodeApp: React.FC<AppProps> = ({ windowId }) => {
    const { t } = useLanguage();
    const [rootPath, setRootPath] = useState("");
    const [fileTree, setFileTree] = useState<FileNode[]>([]);
    const [openFiles, setOpenFiles] = useState<{ path: string; content: string; modified: boolean; language: string }[]>([]);
    const [activeIdx, setActiveIdx] = useState(0);
    const [sidebarTab, setSidebarTab] = useState<"files" | "search" | "git">("files");
    const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
    const [showTerminal, setShowTerminal] = useState(true);
    const [isLoading, setIsLoading] = useState(false);
    const [searchTerm, setSearchTerm] = useState("");
    const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
    const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
    const [showCommandPalette, setShowCommandPalette] = useState(false);
    const [commandInput, setCommandInput] = useState("");
    const [editorTheme, setEditorTheme] = useState("blue-dark");
    const [fontSize, setFontSize] = useState(13);
    const [lspStatus, setLspStatus] = useState<Record<string, boolean>>({});
    const [cursorPos, setCursorPos] = useState({ line: 1, col: 1 });
    const editorRef = useRef<any>(null);
    const monacoRef = useRef<Monaco | null>(null);

    useEffect(() => {
        SystemBridge.getDefaultDesktopPath().then(p => {
            const home = p.replace("/Desktop", "");
            setRootPath(home);
            loadTree(home);
        });
    }, []);

    const loadTree = async (path: string) => {
        setIsLoading(true);
        try {
            const entries = await SystemBridge.getFiles(path);
            setFileTree(entries.map(f => ({
                name: f.name, path: f.path,
                type: f.is_dir ? "directory" : "file",
                expanded: false, children: f.is_dir ? [] : undefined,
            })));
        } catch {}
        setIsLoading(false);
    };

    const loadChildren = async (node: FileNode) => {
        if (node.type !== "directory" || (node.children && node.children.length > 0)) return;
        const files = await SystemBridge.getFiles(node.path);
        node.children = files.map(f => ({
            name: f.name, path: f.path,
            type: f.is_dir ? "directory" : "file",
            expanded: false, children: f.is_dir ? [] : undefined,
        }));
        setFileTree([...fileTree]);
    };

    const toggleDir = async (node: FileNode) => {
        if (node.type !== "directory") return;
        node.expanded = !node.expanded;
        if (node.expanded && (!node.children || node.children.length === 0)) await loadChildren(node);
        else setFileTree([...fileTree]);
    };

        const openFile = async (path: string) => {
            const existing = openFiles.findIndex(f => f.path === path);
            if (existing >= 0) { setActiveIdx(existing); return; }
            const content = await SystemBridge.readFile(path);
            const language = getLang(path);
            setOpenFiles(prev => [...prev, { path, content, modified: false, language }]);
            setActiveIdx(openFiles.length);
            // Start LSP for this language if not running
            startLsp(language, rootPath);
        };

        const startLsp = async (language: string, root: string) => {
            if (lspStatus[language]) return;
            const LSP_LANGS = ["typescript", "javascript", "rust", "python", "go", "cpp", "c"];
            if (!LSP_LANGS.includes(language)) return;
            const res = await SystemBridge.startLanguageServer(language, root);
            if (res.success) {
                setLspStatus(prev => ({ ...prev, [language]: true }));
            }
        };

        const saveFile = async (idx: number) => {
            const file = openFiles[idx];
            if (!file) return;
            await SystemBridge.writeFile(file.path, file.content);
            setOpenFiles(prev => prev.map((f, i) => i === idx ? { ...f, modified: false } : f));
            runLint(idx);
        };

        const runLint = async (idx: number) => {
            const file = openFiles[idx];
            if (!file) return;
            const newDiags: Diagnostic[] = [];
            if (file.language === "rust") {
                const res = await SystemBridge.executeCommand(`cd "${rootPath}" && cargo check --message-format=short 2>&1 | head -20`);
                const out = res.stdout || res.stderr || "";
                for (const line of out.split("\n")) {
                    const m = line.match(/^(.+):(\d+):(\d+):\s*(error|warning):\s*(.+)$/);
                    if (m) newDiags.push({ file: m[1], line: parseInt(m[2]), col: parseInt(m[3]), severity: m[4] as any, message: m[5] });
                }
            } else if (file.language === "typescript" || file.language === "javascript") {
                const res = await SystemBridge.executeCommand(`npx --yes tsc --noEmit --allowJs "${file.path}" 2>&1 | head -20`);
                const out = res.stdout || res.stderr || "";
                for (const line of out.split("\n")) {
                    const m = line.match(/^(.+)\((\d+),(\d+)\):\s*(error|warning)\s+\w+:\s*(.+)$/);
                    if (m) newDiags.push({ file: m[1], line: parseInt(m[2]), col: parseInt(m[3]), severity: m[4] as any, message: m[5] });
                }
            }
            setDiagnostics(prev => [...prev.filter(d => d.file !== file.path), ...newDiags]);

            // Show diagnostics in Monaco
            if (monacoRef.current && editorRef.current) {
                const monaco = monacoRef.current;
                const model = editorRef.current.getModel();
                if (model) {
                    const markers = newDiags
                    .filter(d => d.file === file.path || file.path.endsWith(d.file))
                    .map(d => ({
                        severity: d.severity === "error" ? monaco.MarkerSeverity.Error : monaco.MarkerSeverity.Warning,
                        startLineNumber: d.line, startColumn: d.col,
                        endLineNumber: d.line, endColumn: d.col + 30,
                        message: d.message,
                    }));
                    monaco.editor.setModelMarkers(model, "blue-lint", markers);
                }
            }
        };

        const closeFile = (idx: number) => {
            if (openFiles.length <= 1) return;
            setOpenFiles(prev => prev.filter((_, i) => i !== idx));
            setActiveIdx(i => Math.max(0, i === idx ? i - 1 : i > idx ? i - 1 : i));
        };

        const newFile = async () => {
            const name = window.prompt("New file name:");
            if (!name) return;
            const path = `${rootPath}/${name}`;
            await SystemBridge.writeFile(path, "");
            await loadTree(rootPath);
            await openFile(path);
        };

        const deleteNode = async (node: FileNode) => {
            if (!window.confirm(`Delete ${node.name}?`)) return;
            await SystemBridge.deleteFile(node.path);
            await loadTree(rootPath);
            const idx = openFiles.findIndex(f => f.path === node.path);
            if (idx >= 0) closeFile(idx);
        };

            const searchFiles = async () => {
                if (!searchTerm.trim()) return;
                const results: SearchResult[] = [];
                const searchDir = async (dir: string) => {
                    const files = await SystemBridge.getFiles(dir);
                    for (const file of files) {
                        if (file.is_dir) { await searchDir(file.path); continue; }
                        if (!file.mime_type.startsWith("text/") && !file.name.match(/\.(ts|tsx|js|jsx|rs|py|go|md|json|toml)$/)) continue;
                        const content = await SystemBridge.readFile(file.path);
                        const lines = content.split("\n");
                        for (let i = 0; i < lines.length; i++) {
                            if (lines[i].toLowerCase().includes(searchTerm.toLowerCase())) {
                                results.push({ file: file.path, line: i + 1, content: lines[i].trim() });
                                if (results.length >= 50) return;
                            }
                        }
                    }
                };
                await searchDir(rootPath);
                setSearchResults(results);
            };

            const handleEditorChange = (value?: string) => {
                if (value === undefined) return;
                setOpenFiles(prev => prev.map((f, i) => i === activeIdx ? { ...f, content: value, modified: true } : f));
            };

            const handleEditorMount: OnMount = (editor, monaco) => {
                editorRef.current = editor;
                monacoRef.current = monaco;

                editor.onDidChangeCursorPosition(e => {
                    setCursorPos({ line: e.position.lineNumber, col: e.position.column });
                });

                // Keybindings
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => saveFile(activeIdx));
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyS,
                                  () => { openFiles.forEach((_, i) => { if (openFiles[i].modified) saveFile(i); }); });
                editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyP,
                                  () => setShowCommandPalette(true));

                // Define themes
                monaco.editor.defineTheme("blue-dark", {
                    base: "vs-dark", inherit: true, rules: [],
                    colors: {
                        "editor.background": "#0f172a",
                        "editor.lineHighlightBackground": "#1e293b50",
                        "editorLineNumber.foreground": "#475569",
                        "editorLineNumber.activeForeground": "#94a3b8",
                        "editorGutter.background": "#0f172a",
                        "editor.selectionBackground": "#2563eb40",
                    },
                });
                monaco.editor.defineTheme("blue-light", {
                    base: "vs", inherit: true, rules: [],
                    colors: { "editor.background": "#f8fafc" },
                });
                monaco.editor.setTheme(editorTheme);
            };

            useEffect(() => {
                monacoRef.current?.editor.setTheme(editorTheme);
            }, [editorTheme]);

            const commands = [
                { id: "save",      label: "Save File",           shortcut: "Ctrl+S",   action: () => saveFile(activeIdx) },
                { id: "saveAll",   label: "Save All",             shortcut: "Ctrl+Shift+S", action: () => openFiles.forEach((f, i) => f.modified && saveFile(i)) },
                { id: "newFile",   label: "New File",             shortcut: "Ctrl+N",   action: newFile },
                { id: "terminal",  label: "Toggle Terminal",      shortcut: "Ctrl+`",   action: () => setShowTerminal(s => !s) },
                { id: "sidebar",   label: "Toggle Sidebar",       shortcut: "Ctrl+B",   action: () => setSidebarCollapsed(s => !s) },
                { id: "refresh",   label: "Refresh Explorer",     shortcut: "",         action: () => loadTree(rootPath) },
                { id: "lint",      label: "Run Linter",           shortcut: "",         action: () => runLint(activeIdx) },
                { id: "search",    label: "Search in Files",      shortcut: "Ctrl+F",   action: () => setSidebarTab("search") },
                { id: "lightTheme",label: "Toggle Light/Dark",    shortcut: "",         action: () => setEditorTheme(t => t === "blue-dark" ? "blue-light" : "blue-dark") },
            ];

            const renderTree = (nodes: FileNode[], level = 0): React.ReactNode => nodes.map(node => (
                <div key={node.path}>
                <div
                className="flex items-center gap-1 py-0.5 px-1 rounded cursor-pointer hover:bg-white/5 group text-sm"
                style={{ paddingLeft: `${level * 12 + 4}px` }}
                onDoubleClick={() => node.type === "file" && openFile(node.path)}
                onClick={() => node.type === "directory" && toggleDir(node)}
                >
                {node.type === "directory" && (
                    <span className="text-slate-500 w-4 shrink-0">
                    {node.expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                    </span>
                )}
                {node.type === "directory"
                    ? <Folder size={14} className="text-blue-400 shrink-0" />
                    : <FileCode size={14} className="text-yellow-400 shrink-0" />
                }
                <span className="truncate flex-1">{node.name}</span>
                <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 ml-auto">
                {node.type === "file" && (
                    <button onClick={e => { e.stopPropagation(); openFile(node.path); }}
                    className="p-0.5 hover:bg-white/10 rounded text-slate-500"><FileCode size={11} /></button>
                )}
                <button onClick={e => { e.stopPropagation(); deleteNode(node); }}
                className="p-0.5 hover:bg-white/10 rounded text-slate-500 hover:text-red-400"><Trash2 size={11} /></button>
                </div>
                </div>
                {node.type === "directory" && node.expanded && node.children && renderTree(node.children, level + 1)}
                </div>
            ));

            const activeFile = openFiles[activeIdx];
            const fileDiagnostics = diagnostics.filter(d =>
            activeFile && (d.file === activeFile.path || activeFile.path.endsWith(d.file)));
            const errors   = fileDiagnostics.filter(d => d.severity === "error").length;
            const warnings = fileDiagnostics.filter(d => d.severity === "warning").length;

            return (
                <div className="flex flex-col h-full bg-slate-900 text-white text-sm relative">
                {/* Toolbar */}
                <div className="h-10 bg-slate-800 border-b border-white/5 flex items-center px-3 gap-1 shrink-0">
                <button onClick={() => setSidebarCollapsed(s => !s)} className="p-1.5 hover:bg-white/10 rounded" title="Toggle sidebar"><FolderOpen size={16} /></button>
                <button onClick={newFile} className="p-1.5 hover:bg-white/10 rounded" title="New file"><Plus size={16} /></button>
                <button onClick={() => saveFile(activeIdx)} disabled={!activeFile?.modified}
                className="p-1.5 hover:bg-white/10 rounded disabled:opacity-40" title="Save (Ctrl+S)"><Save size={16} /></button>
                <div className="w-px h-5 bg-white/10 mx-1" />
                <button onClick={() => setShowTerminal(s => !s)}
                className={`p-1.5 rounded ${showTerminal ? "bg-blue-600/20 text-blue-400" : "hover:bg-white/10"}`}
                title="Terminal"><TerminalIcon size={16} /></button>
                <button onClick={() => setSidebarTab("search")}
                className={`p-1.5 rounded ${sidebarTab === "search" ? "bg-blue-600/20 text-blue-400" : "hover:bg-white/10"}`}
                title="Search"><Search size={16} /></button>
                <button onClick={() => setSidebarTab("git")}
                className={`p-1.5 rounded ${sidebarTab === "git" ? "bg-blue-600/20 text-blue-400" : "hover:bg-white/10"}`}
                title="Git"><GitBranch size={16} /></button>
                <button onClick={() => setShowCommandPalette(true)} className="p-1.5 hover:bg-white/10 rounded" title="Command Palette (Ctrl+Shift+P)">
                <Command size={16} />
                </button>
                <div className="flex-1" />
                <div className="text-xs text-slate-500 truncate max-w-64">{activeFile?.path || "No file open"}</div>
                {/* LSP indicators */}
                {Object.entries(lspStatus).map(([lang, active]) => (
                    <div key={lang} title={`LSP: ${lang} ${active ? "active" : "inactive"}`}
                    className={`w-2 h-2 rounded-full ${active ? "bg-green-400" : "bg-slate-600"}`} />
                ))}
                <button onClick={() => loadTree(rootPath)} className="p-1.5 hover:bg-white/10 rounded"><RefreshCw size={14} /></button>
                </div>

                <div className="flex flex-1 overflow-hidden">
                {/* Sidebar */}
                {!sidebarCollapsed && (
                    <div className="w-56 bg-slate-800/50 border-r border-white/5 flex flex-col overflow-hidden">
                    {/* Sidebar tabs */}
                    <div className="flex border-b border-white/5 shrink-0">
                    {(["files", "search", "git"] as const).map(tab => (
                        <button key={tab} onClick={() => setSidebarTab(tab)}
                        className={`flex-1 py-1.5 text-xs capitalize transition-colors
                            ${sidebarTab === tab ? "bg-slate-900 text-white border-b-2 border-blue-500" : "text-slate-500 hover:text-white"}`}>
                            {tab}
                            </button>
                    ))}
                    </div>

                    {sidebarTab === "files" && (
                        <div className="flex-1 overflow-y-auto p-1">
                        <div className="flex items-center justify-between px-2 py-1 mb-1">
                        <span className="text-[10px] font-semibold text-slate-500 uppercase tracking-wider">Explorer</span>
                        <button onClick={() => loadTree(rootPath)} className="p-0.5 hover:bg-white/10 rounded text-slate-500">
                        <RefreshCw size={11} />
                        </button>
                        </div>
                        {isLoading
                            ? <div className="text-center py-4 text-slate-500 text-xs">Loading…</div>
                            : renderTree(fileTree)}
                            </div>
                    )}

                    {sidebarTab === "search" && (
                        <div className="flex-1 overflow-y-auto p-2">
                        <div className="flex gap-1 mb-2">
                        <input type="text" value={searchTerm} onChange={e => setSearchTerm(e.target.value)}
                        onKeyDown={e => e.key === "Enter" && searchFiles()}
                        placeholder="Search…"
                        className="flex-1 bg-slate-900 border border-white/10 rounded px-2 py-1 text-xs focus:outline-none focus:border-blue-500/50" />
                        <button onClick={searchFiles} className="px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded text-xs">Go</button>
                        </div>
                        <div className="space-y-1">
                        {searchResults.map((r, i) => (
                            <div key={i} onClick={() => openFile(r.file)}
                            className="cursor-pointer hover:bg-white/5 rounded p-1">
                            <div className="text-[10px] text-blue-400 truncate">{r.file.split("/").pop()}:{r.line}</div>
                            <div className="text-[10px] text-slate-400 truncate">{r.content}</div>
                            </div>
                        ))}
                        {searchResults.length === 0 && searchTerm && (
                            <div className="text-xs text-slate-600 text-center py-4">No results</div>
                        )}
                        </div>
                        </div>
                    )}

                    {sidebarTab === "git" && (
                        <div className="flex-1 overflow-hidden">
                        <GitPanel cwd={rootPath} />
                        </div>
                    )}
                    </div>
                )}

                {/* Editor area */}
                <div className="flex-1 flex flex-col overflow-hidden">
                {/* Tabs */}
                <div className="flex bg-slate-800 border-b border-white/5 overflow-x-auto shrink-0">
                {openFiles.map((file, idx) => (
                    <div key={idx} onClick={() => setActiveIdx(idx)}
                    className={`flex items-center gap-1.5 px-3 py-2 cursor-pointer border-b-2 shrink-0 group max-w-[160px]
                        ${activeIdx === idx ? "border-blue-500 text-white bg-slate-900" : "border-transparent text-slate-400 hover:text-white"}`}>
                        <FileCode size={13} />
                        <span className="text-xs truncate">{file.path.split("/").pop()}</span>
                        {file.modified && <span className="w-1.5 h-1.5 bg-yellow-400 rounded-full shrink-0" />}
                        <button onClick={e => { e.stopPropagation(); closeFile(idx); }}
                        className="p-0.5 hover:bg-white/10 rounded opacity-0 group-hover:opacity-100 shrink-0"><X size={11} /></button>
                        </div>
                ))}
                </div>

                {/* Diagnostics strip */}
                {fileDiagnostics.length > 0 && (
                    <div className="shrink-0 flex items-center gap-3 px-3 py-1 bg-red-500/5 border-b border-red-500/10 text-xs">
                    {errors > 0 && (
                        <span className="flex items-center gap-1 text-red-400">
                        <AlertCircle size={12} /> {errors} error{errors > 1 ? "s" : ""}
                        </span>
                    )}
                    {warnings > 0 && (
                        <span className="flex items-center gap-1 text-yellow-400">
                        <AlertCircle size={12} /> {warnings} warning{warnings > 1 ? "s" : ""}
                        </span>
                    )}
                    <span className="text-slate-600 text-[10px] ml-auto">
                    {fileDiagnostics[0].message.slice(0, 60)}
                    </span>
                    </div>
                )}

                {/* Monaco Editor */}
                <div className="flex-1 overflow-hidden">
                {activeFile ? (
                    <Editor
                    height="100%"
                    language={activeFile.language}
                    value={activeFile.content}
                    onChange={handleEditorChange}
                    onMount={handleEditorMount}
                    theme={editorTheme}
                    options={{
                        fontSize, minimap: { enabled: true },
                        scrollBeyondLastLine: false,
                        automaticLayout: true,
                        fontFamily: "JetBrains Mono, Fira Code, monospace",
                        renderWhitespace: "boundary",
                        tabSize: 4,
                        wordWrap: "off",
                        suggestOnTriggerCharacters: true,
                        quickSuggestions: { other: true, comments: true, strings: true },
                        parameterHints: { enabled: true },
                        formatOnPaste: true,
                            formatOnType: false,
                                bracketPairColorization: { enabled: true },
                                guides: { bracketPairs: true, indentation: true },
                                renderLineHighlight: "all",
                                scrollbar: { verticalScrollbarSize: 6, horizontalScrollbarSize: 6 },
                    }}
                    />
                ) : (
                    <div className="flex flex-col items-center justify-center h-full text-slate-600 gap-4">
                    <Code2 size={40} />
                    <div className="text-sm">Open a file or create a new one</div>
                    <button onClick={newFile} className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm text-white">
                    <Plus size={14} /> New File
                    </button>
                    </div>
                )}
                </div>

                {/* Terminal */}
                {showTerminal && (
                    <div className="h-44 bg-slate-900 border-t border-white/5 flex flex-col shrink-0">
                    <div className="flex items-center justify-between px-3 py-1 border-b border-white/5 bg-slate-800/50">
                    <span className="text-xs text-slate-400">Terminal</span>
                    <button onClick={() => setShowTerminal(false)} className="p-0.5 hover:bg-white/10 rounded text-slate-500"><X size={12} /></button>
                    </div>
                    <div className="flex-1 overflow-hidden">
                    <TerminalPane windowId={windowId} />
                    </div>
                    </div>
                )}
                </div>
                </div>

                {/* Status bar */}
                <div className="h-6 bg-slate-800 border-t border-white/5 flex items-center px-3 gap-4 text-[11px] text-slate-500 shrink-0">
                <span>{activeFile ? getLang(activeFile.path).toUpperCase() : "—"}</span>
                <span>Ln {cursorPos.line}, Col {cursorPos.col}</span>
                {errors > 0 && <span className="text-red-400 flex items-center gap-1"><AlertCircle size={11} /> {errors}</span>}
                {warnings > 0 && <span className="text-yellow-400 flex items-center gap-1"><AlertCircle size={11} /> {warnings}</span>}
                <div className="flex-1" />
                <button onClick={() => setEditorTheme(t => t === "blue-dark" ? "blue-light" : "blue-dark")}
                className="hover:text-white">{editorTheme === "blue-dark" ? "🌙" : "☀️"}</button>
                <button onClick={() => setFontSize(s => Math.max(10, s - 1))} className="hover:text-white">A-</button>
                <span>{fontSize}px</span>
                <button onClick={() => setFontSize(s => Math.min(24, s + 1))} className="hover:text-white">A+</button>
                </div>

                {/* Command palette */}
                {showCommandPalette && (
                    <div className="absolute inset-0 bg-black/50 flex items-start justify-center z-50 pt-20"
                    onClick={() => setShowCommandPalette(false)}>
                    <div className="bg-slate-800 rounded-xl w-96 p-4 shadow-2xl" onClick={e => e.stopPropagation()}>
                    <div className="flex items-center gap-2 mb-3">
                    <Command size={14} className="text-slate-500" />
                    <input type="text" value={commandInput} onChange={e => setCommandInput(e.target.value)}
                    placeholder="Type a command…"
                    className="flex-1 bg-transparent text-sm text-white focus:outline-none"
                    autoFocus />
                    <button onClick={() => setShowCommandPalette(false)} className="text-slate-500 hover:text-white"><X size={13} /></button>
                    </div>
                    <div className="space-y-0.5 max-h-56 overflow-y-auto">
                    {commands.filter(c => c.label.toLowerCase().includes(commandInput.toLowerCase())).map(cmd => (
                        <button key={cmd.id} onClick={() => { cmd.action(); setShowCommandPalette(false); setCommandInput(""); }}
                        className="w-full flex items-center justify-between px-3 py-2 hover:bg-white/10 rounded text-sm text-left">
                        <span>{cmd.label}</span>
                        {cmd.shortcut && <span className="text-xs text-slate-500 font-mono">{cmd.shortcut}</span>}
                        </button>
                    ))}
                    </div>
                    </div>
                    </div>
                )}
                </div>
            );
};

function getLang(path: string): string {
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    const map: Record<string, string> = {
        html: "html", css: "css", scss: "scss",
        js: "javascript", jsx: "javascript",
        ts: "typescript", tsx: "typescript",
        rs: "rust", go: "go", py: "python",
        rb: "ruby", php: "php", lua: "lua",
        c: "c", cpp: "cpp", h: "c", hpp: "cpp",
        json: "json", yaml: "yaml", yml: "yaml",
        toml: "toml", xml: "xml", md: "markdown",
        sh: "shell", bash: "shell", zsh: "shell",
        hk: "ini",
    };
    return map[ext] || "plaintext";
}

export default BlueCodeApp;
