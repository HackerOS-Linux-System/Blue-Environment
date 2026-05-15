import React, { useState, useEffect, useRef, useCallback } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import {
    Save, FolderOpen, Plus, X, FileText, RefreshCw,
    Bold, Italic, AlignLeft, AlignCenter, AlignRight,
    Search, Replace, Hash, ChevronDown
} from 'lucide-react';

interface NoteTab {
    id: string;
    title: string;
    content: string;
    path?: string;
    modified: boolean;
}

interface FindState {
    query: string;
    replace: string;
    show: boolean;
    matchCase: boolean;
    count: number;
}

const NotepadApp: React.FC<AppProps> = () => {
    const [tabs, setTabs] = useState<NoteTab[]>([]);
    const [activeId, setActiveId] = useState<string | null>(null);
    const [find, setFind] = useState<FindState>({ query: '', replace: '', show: false, matchCase: false, count: 0 });
    const [wordCount, setWordCount] = useState(0);
    const [lineCount, setLineCount] = useState(0);
    const [cursorPos, setCursorPos] = useState({ line: 1, col: 1 });
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const autosaveRef = useRef<ReturnType<typeof setInterval>>();
    const fileInputRef = useRef<HTMLInputElement>(null);

    const activeTab = tabs.find(t => t.id === activeId) ?? null;

    // Create initial tab
    useEffect(() => {
        const id = `note-${Date.now()}`;
        setTabs([{ id, title: 'Untitled', content: '', modified: false }]);
        setActiveId(id);
    }, []);

    // Autosave every 30s
    useEffect(() => {
        autosaveRef.current = setInterval(() => {
            tabs.forEach(tab => {
                if (tab.modified) saveToCacheNote(tab);
            });
        }, 30000);
        return () => clearInterval(autosaveRef.current);
    }, [tabs]);

    // Load from cache on first tab creation
    useEffect(() => {
        if (tabs.length === 1 && activeId) {
            SystemBridge.executeCommand(`cat ~/.cache/Blue-Environment/notepad-autosave.json 2>/dev/null`)
                .then(r => {
                    const out = typeof r === 'string' ? r : r?.stdout || '';
                    try {
                        const saved = JSON.parse(out);
                        if (saved.content) {
                            setTabs(prev => prev.map(t => t.id === activeId ? {
                                ...t, content: saved.content, title: saved.title || 'Untitled', modified: false
                            } : t));
                        }
                    } catch {}
                });
        }
    }, []);

    const saveToCacheNote = async (tab: NoteTab) => {
        const data = JSON.stringify({ content: tab.content, title: tab.title });
        await SystemBridge.executeCommand(`mkdir -p ~/.cache/Blue-Environment && cat > ~/.cache/Blue-Environment/notepad-autosave.json << 'JSON'\n${data}\nJSON`).catch(() => {});
    };

    const updateContent = (id: string, content: string) => {
        setTabs(prev => prev.map(t => t.id === id ? { ...t, content, modified: true } : t));
        const lines = content.split('\n');
        setLineCount(lines.length);
        setWordCount(content.trim() ? content.trim().split(/\s+/).length : 0);
    };

    const newTab = () => {
        const id = `note-${Date.now()}`;
        setTabs(prev => [...prev, { id, title: 'Untitled', content: '', modified: false }]);
        setActiveId(id);
    };

    const closeTab = (id: string, e: React.MouseEvent) => {
        e.stopPropagation();
        setTabs(prev => {
            const next = prev.filter(t => t.id !== id);
            if (next.length === 0) {
                const newId = `note-${Date.now()}`;
                setActiveId(newId);
                return [{ id: newId, title: 'Untitled', content: '', modified: false }];
            }
            if (activeId === id) setActiveId(next[next.length - 1].id);
            return next;
        });
    };

    const openFile = () => fileInputRef.current?.click();

    const handleFileOpen = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const text = await file.text();
        const id = `note-${Date.now()}`;
        setTabs(prev => [...prev, { id, title: file.name, content: text, path: file.name, modified: false }]);
        setActiveId(id);
        e.target.value = '';
    };

    const saveFile = async () => {
        if (!activeTab) return;
        const blob = new Blob([activeTab.content], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url; a.download = activeTab.title.endsWith('.txt') ? activeTab.title : `${activeTab.title}.txt`;
        a.click(); URL.revokeObjectURL(url);
        setTabs(prev => prev.map(t => t.id === activeTab.id ? { ...t, modified: false } : t));
    };

    const doFind = useCallback(() => {
        if (!activeTab || !find.query) { setFind(f => ({ ...f, count: 0 })); return; }
        const flags = find.matchCase ? 'g' : 'gi';
        const matches = [...activeTab.content.matchAll(new RegExp(find.query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), flags))];
        setFind(f => ({ ...f, count: matches.length }));
        // Scroll to first match
        if (matches.length > 0 && textareaRef.current) {
            textareaRef.current.focus();
            textareaRef.current.setSelectionRange(matches[0].index!, matches[0].index! + find.query.length);
        }
    }, [activeTab, find.query, find.matchCase]);

    const doReplace = () => {
        if (!activeTab || !find.query) return;
        const flags = find.matchCase ? 'g' : 'gi';
        const newContent = activeTab.content.replace(
            new RegExp(find.query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), flags),
            find.replace
        );
        updateContent(activeTab.id, newContent);
    };

    const handleCursorMove = (e: React.SyntheticEvent<HTMLTextAreaElement>) => {
        const t = e.target as HTMLTextAreaElement;
        const before = t.value.substring(0, t.selectionStart);
        const lines = before.split('\n');
        setCursorPos({ line: lines.length, col: lines[lines.length - 1].length + 1 });
    };

    const insertText = (before: string, after: string = '') => {
        const ta = textareaRef.current;
        if (!ta || !activeTab) return;
        const start = ta.selectionStart, end = ta.selectionEnd;
        const sel = activeTab.content.substring(start, end);
        const newContent = activeTab.content.substring(0, start) + before + sel + after + activeTab.content.substring(end);
        updateContent(activeTab.id, newContent);
        setTimeout(() => { ta.selectionStart = start + before.length; ta.selectionEnd = start + before.length + sel.length; ta.focus(); }, 0);
    };

    const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
        // Tab key inserts spaces instead of changing focus
        if (e.key === 'Tab') {
            e.preventDefault();
            insertText('    ');
        }
        // Ctrl+S
        if (e.ctrlKey && e.key === 's') { e.preventDefault(); saveFile(); }
        // Ctrl+F
        if (e.ctrlKey && e.key === 'f') { e.preventDefault(); setFind(f => ({ ...f, show: !f.show })); }
    };

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white overflow-hidden">
            <input ref={fileInputRef} type="file" accept=".txt,.md,.json,.csv,.log,.hk,.yaml,.yml,.sh,.py,.js,.ts,.css,.html" className="hidden" onChange={handleFileOpen} />

            {/* Toolbar */}
            <div className="shrink-0 flex items-center gap-1 px-3 py-2 bg-slate-800 border-b border-white/5">
                <button onClick={newTab} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="New (Ctrl+T)"><Plus size={15} /></button>
                <button onClick={openFile} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Open file"><FolderOpen size={15} /></button>
                <button onClick={saveFile} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Save (Ctrl+S)"><Save size={15} /></button>
                <div className="w-px h-5 bg-white/10 mx-1" />
                <button onClick={() => insertText('**', '**')} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Bold"><Bold size={15} /></button>
                <button onClick={() => insertText('*', '*')} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Italic"><Italic size={15} /></button>
                <button onClick={() => insertText('# ')} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Heading"><Hash size={15} /></button>
                <div className="w-px h-5 bg-white/10 mx-1" />
                <button onClick={() => setFind(f => ({ ...f, show: !f.show }))} className={`p-1.5 rounded-lg ${find.show ? 'bg-blue-500/20 text-blue-400' : 'hover:bg-white/10 text-slate-400'}`} title="Find (Ctrl+F)"><Search size={15} /></button>
            </div>

            {/* Tabs */}
            <div className="shrink-0 flex bg-slate-800/50 border-b border-white/5 overflow-x-auto scrollbar-hide">
                {tabs.map(tab => (
                    <div key={tab.id} onClick={() => setActiveId(tab.id)}
                        className={`flex items-center gap-1.5 px-4 py-2 cursor-pointer shrink-0 border-r border-white/5 text-sm group max-w-[150px] ${activeId === tab.id ? 'bg-slate-900 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}`}>
                        <FileText size={13} className="shrink-0" />
                        <span className="truncate">{tab.title}</span>
                        {tab.modified && <span className="w-1.5 h-1.5 rounded-full bg-blue-400 shrink-0" />}
                        <button onClick={e => closeTab(tab.id, e)} className="opacity-0 group-hover:opacity-100 hover:text-red-400 ml-0.5 shrink-0"><X size={11} /></button>
                    </div>
                ))}
                <button onClick={newTab} className="p-2.5 hover:bg-slate-700/50 text-slate-500 hover:text-white shrink-0"><Plus size={13} /></button>
            </div>

            {/* Find bar */}
            {find.show && (
                <div className="shrink-0 flex items-center gap-2 px-3 py-2 bg-slate-800/80 border-b border-white/5">
                    <div className="relative">
                        <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-slate-500" />
                        <input value={find.query} onChange={e => setFind(f => ({ ...f, query: e.target.value }))}
                            onKeyDown={e => e.key === 'Enter' && doFind()} placeholder="Find..."
                            className="bg-slate-900 border border-white/10 rounded-lg pl-7 pr-3 py-1.5 text-sm text-white w-44 focus:outline-none focus:border-blue-500/50" autoFocus />
                    </div>
                    <input value={find.replace} onChange={e => setFind(f => ({ ...f, replace: e.target.value }))}
                        placeholder="Replace with..."
                        className="bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white w-40 focus:outline-none" />
                    <button onClick={doFind} className="px-2.5 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs transition-colors">Find</button>
                    <button onClick={doReplace} className="px-2.5 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs transition-colors">Replace All</button>
                    <label className="flex items-center gap-1 text-xs text-slate-400 cursor-pointer">
                        <input type="checkbox" checked={find.matchCase} onChange={e => setFind(f => ({ ...f, matchCase: e.target.checked }))} className="accent-blue-500" />
                        Aa
                    </label>
                    {find.count > 0 && <span className="text-xs text-slate-500">{find.count} matches</span>}
                    <button onClick={() => setFind(f => ({ ...f, show: false }))} className="ml-auto text-slate-500 hover:text-white"><X size={14} /></button>
                </div>
            )}

            {/* Editor */}
            <div className="flex-1 overflow-hidden">
                {activeTab && (
                    <textarea
                        ref={textareaRef}
                        value={activeTab.content}
                        onChange={e => updateContent(activeTab.id, e.target.value)}
                        onKeyDown={handleKeyDown}
                        onKeyUp={handleCursorMove}
                        onClick={handleCursorMove}
                        spellCheck={false}
                        className="w-full h-full bg-slate-950 text-slate-100 font-mono text-sm p-4 resize-none focus:outline-none leading-relaxed"
                        placeholder="Start typing..."
                        style={{ fontFamily: "'JetBrains Mono', 'Fira Code', monospace", tabSize: 4 }}
                    />
                )}
            </div>

            {/* Status bar */}
            <div className="shrink-0 flex items-center justify-between px-4 py-1 bg-slate-800 border-t border-white/5 text-xs text-slate-500">
                <div className="flex gap-4">
                    <span>Ln {cursorPos.line}, Col {cursorPos.col}</span>
                    <span>{lineCount} lines</span>
                    <span>{wordCount} words</span>
                    {activeTab?.content.length ? <span>{activeTab.content.length} chars</span> : null}
                </div>
                <div className="flex gap-3">
                    {activeTab?.modified && <span className="text-yellow-500">Modified</span>}
                    <span>UTF-8</span>
                    <span>Plain Text</span>
                </div>
            </div>
        </div>
    );
};
export default NotepadApp;
