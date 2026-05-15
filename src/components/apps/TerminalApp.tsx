import React, { useEffect, useRef, useState, useCallback } from 'react';
import { AppProps } from '../../types';
import { Plus, X, ChevronDown, Settings, Maximize2 } from 'lucide-react';

// Dynamic xterm import to avoid SSR issues
let Terminal: any = null;
let FitAddon: any = null;
let WebLinksAddon: any = null;

interface Tab {
    id: string;
    title: string;
    pid?: number;
}

const THEMES = {
    'Blue Dark': {
        background: '#0d1117', foreground: '#e6edf3', cursor: '#58a6ff',
        black: '#161b22', red: '#ff7b72', green: '#3fb950', yellow: '#d29922',
        blue: '#58a6ff', magenta: '#bc8cff', cyan: '#39d353', white: '#b1bac4',
        brightBlack: '#6e7681', brightRed: '#ffa198', brightGreen: '#56d364',
        brightYellow: '#e3b341', brightBlue: '#79c0ff', brightMagenta: '#d2a8ff',
        brightCyan: '#56d364', brightWhite: '#f0f6fc',
    },
    'Dracula': {
        background: '#282a36', foreground: '#f8f8f2', cursor: '#f8f8f2',
        black: '#21222c', red: '#ff5555', green: '#50fa7b', yellow: '#f1fa8c',
        blue: '#bd93f9', magenta: '#ff79c6', cyan: '#8be9fd', white: '#f8f8f2',
        brightBlack: '#6272a4', brightRed: '#ff6e6e', brightGreen: '#69ff94',
        brightYellow: '#ffffa5', brightBlue: '#d6acff', brightMagenta: '#ff92df',
        brightCyan: '#a4ffff', brightWhite: '#ffffff',
    },
    'Solarized': {
        background: '#002b36', foreground: '#839496', cursor: '#839496',
        black: '#073642', red: '#dc322f', green: '#859900', yellow: '#b58900',
        blue: '#268bd2', magenta: '#d33682', cyan: '#2aa198', white: '#eee8d5',
        brightBlack: '#002b36', brightRed: '#cb4b16', brightGreen: '#586e75',
        brightYellow: '#657b83', brightBlue: '#839496', brightMagenta: '#6c71c4',
        brightCyan: '#93a1a1', brightWhite: '#fdf6e3',
    },
} as const;

type ThemeName = keyof typeof THEMES;

const TerminalApp: React.FC<AppProps> = ({ windowId }) => {
    const [tabs, setTabs] = useState<Tab[]>([]);
    const [activeTab, setActiveTab] = useState<string | null>(null);
    const [themeName, setThemeName] = useState<ThemeName>('Blue Dark');
    const [fontSize, setFontSize] = useState(14);
    const [showSettings, setShowSettings] = useState(false);
    const [loaded, setLoaded] = useState(false);
    const termRefs = useRef<Map<string, any>>(new Map()); // tabId -> {term, fitAddon, container}

    // Dynamically load xterm
    useEffect(() => {
        import('xterm').then(m => { Terminal = m.Terminal; })
            .then(() => import('xterm-addon-fit').then(m => { FitAddon = m.FitAddon; }))
            .then(() => import('xterm-addon-web-links').then(m => { WebLinksAddon = m.WebLinksAddon; }))
            .then(() => { setLoaded(true); })
            .catch(() => setLoaded(true)); // fall through even if xterm missing
    }, []);

    const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI__;

    const newTab = useCallback(async () => {
        const id = `tab-${Date.now()}`;
        const tab: Tab = { id, title: 'Terminal' };
        setTabs(prev => [...prev, tab]);
        setActiveTab(id);
    }, []);

    const closeTab = useCallback((id: string, e?: React.MouseEvent) => {
        e?.stopPropagation();
        // Cleanup PTY
        if (isTauri) {
            (window as any).__TAURI__?.invoke('pty_close', { id }).catch(() => {});
        }
        const ref = termRefs.current.get(id);
        if (ref?.term) ref.term.dispose();
        termRefs.current.delete(id);
        setTabs(prev => {
            const next = prev.filter(t => t.id !== id);
            setActiveTab(curr => curr === id ? next[next.length - 1]?.id || null : curr);
            return next;
        });
    }, [isTauri]);

    // Initialize terminal when tab becomes active
    const initTerminal = useCallback((id: string, container: HTMLDivElement | null) => {
        if (!container || !loaded || !Terminal || termRefs.current.get(id)?.term) return;

        const theme = THEMES[themeName];
        const term = new Terminal({
            theme, fontSize, fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
            cursorBlink: true, scrollback: 5000, allowTransparency: false,
            bellStyle: 'none', convertEol: true,
        });
        const fitAddon = FitAddon ? new FitAddon() : null;
        const webLinksAddon = WebLinksAddon ? new WebLinksAddon() : null;
        term.loadAddon(fitAddon);
        if (webLinksAddon) term.loadAddon(webLinksAddon);
        term.open(container);
        fitAddon?.fit();
        termRefs.current.set(id, { term, fitAddon, container });

        if (isTauri) {
            // Real PTY via Tauri
            const { invoke, event } = (window as any).__TAURI__;
            invoke('pty_create', { id, cols: term.cols, rows: term.rows }).catch((e: any) => {
                term.write(`\r\x1b[31mFailed to start PTY: ${e}\x1b[0m\r\n`);
            });

            // Listen for output
            event.listen(`pty-data-${id}`, ({ payload }: any) => {
                term.write(payload);
            });
            event.listen(`pty-exit-${id}`, () => {
                term.write('\r\n\x1b[33m[Process exited — press any key to close]\x1b[0m\r\n');
            });

            // Send input to PTY
            term.onData((data: string) => {
                invoke('pty_write', { id, data }).catch(() => {});
            });

            // Handle resize
            term.onResize(({ cols, rows }: any) => {
                invoke('pty_resize', { id, cols, rows }).catch(() => {});
            });
        } else {
            // Fallback: simulate bash-like environment
            term.write('\x1b[32mBlue Terminal v0.5\x1b[0m — xterm.js shell emulator\r\n');
            term.write('\x1b[33mℹ Running without Tauri PTY — limited functionality\x1b[0m\r\n');
            term.write('$ ');

            let line = '';
            const simPrompt = () => { term.write('\r\n$ '); line = ''; };
            const simRun = async (cmd: string) => {
                const trimmed = cmd.trim();
                if (!trimmed) { simPrompt(); return; }
                try {
                    const result = await fetch('/dev/null').catch(() => null);
                    term.write(`\r\n\x1b[2m[Command: ${trimmed}]\x1b[0m`);
                } catch {}
                simPrompt();
            };

            term.onKey(({ key, domEvent }: any) => {
                const code = domEvent.keyCode;
                if (code === 13) { simRun(line); }
                else if (code === 8) {
                    if (line.length > 0) { line = line.slice(0, -1); term.write('\b \b'); }
                } else if (key.charCodeAt(0) >= 32) {
                    line += key; term.write(key);
                }
            });
        }

        // Resize observer
        const ro = new ResizeObserver(() => fitAddon?.fit());
        ro.observe(container);
        return () => ro.disconnect();
    }, [loaded, themeName, fontSize, isTauri]);

    // Open first tab on load
    useEffect(() => { if (loaded && tabs.length === 0) newTab(); }, [loaded]);

    // Apply theme changes to existing terminals
    useEffect(() => {
        termRefs.current.forEach(({ term }) => {
            term?.setOption?.('theme', THEMES[themeName]);
        });
    }, [themeName]);

    useEffect(() => {
        termRefs.current.forEach(({ term, fitAddon }) => {
            term?.setOption?.('fontSize', fontSize);
            fitAddon?.fit();
        });
    }, [fontSize]);

    return (
        <div className="flex flex-col h-full bg-slate-950 text-white overflow-hidden select-none">
            {/* Tab bar */}
            <div className="shrink-0 flex items-center bg-slate-900 border-b border-white/5 overflow-x-auto scrollbar-hide">
                {tabs.map(tab => (
                    <div
                        key={tab.id}
                        onClick={() => setActiveTab(tab.id)}
                        className={`flex items-center gap-2 px-4 py-2.5 cursor-pointer shrink-0 border-r border-white/5 min-w-0 group transition-colors ${
                            activeTab === tab.id ? 'bg-slate-950 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-800'
                        }`}
                        style={{ maxWidth: 160 }}
                    >
                        <span className="text-xs truncate">{tab.title}</span>
                        <button
                            onClick={e => closeTab(tab.id, e)}
                            className="ml-1 p-0.5 opacity-0 group-hover:opacity-100 hover:text-red-400 rounded transition-all shrink-0"
                        >
                            <X size={10} />
                        </button>
                    </div>
                ))}
                <button onClick={newTab} className="p-2.5 hover:bg-slate-800 text-slate-500 hover:text-white transition-colors shrink-0">
                    <Plus size={14} />
                </button>
                <div className="ml-auto flex items-center gap-1 px-2">
                    <button onClick={() => setShowSettings(s => !s)}
                        className={`p-1.5 rounded transition-colors ${showSettings ? 'text-blue-400 bg-blue-500/10' : 'text-slate-500 hover:text-white'}`}>
                        <Settings size={13} />
                    </button>
                </div>
            </div>

            {/* Settings panel */}
            {showSettings && (
                <div className="shrink-0 flex items-center gap-4 px-4 py-2 bg-slate-900 border-b border-white/5 text-sm">
                    <div className="flex items-center gap-2">
                        <span className="text-slate-400 text-xs">Theme:</span>
                        <select value={themeName} onChange={e => setThemeName(e.target.value as ThemeName)}
                            className="bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-white focus:outline-none">
                            {Object.keys(THEMES).map(t => <option key={t} value={t}>{t}</option>)}
                        </select>
                    </div>
                    <div className="flex items-center gap-2">
                        <span className="text-slate-400 text-xs">Size:</span>
                        <input type="number" value={fontSize} min={10} max={24}
                            onChange={e => setFontSize(parseInt(e.target.value) || 14)}
                            className="bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-white w-14 focus:outline-none" />
                    </div>
                    <button onClick={() => setShowSettings(false)} className="ml-auto text-slate-500 hover:text-white">
                        <X size={13} />
                    </button>
                </div>
            )}

            {/* Terminal panes */}
            <div className="flex-1 relative overflow-hidden">
                {tabs.map(tab => (
                    <div
                        key={tab.id}
                        className={`absolute inset-0 ${activeTab === tab.id ? '' : 'invisible pointer-events-none'}`}
                        ref={el => el && initTerminal(tab.id, el)}
                        style={{ padding: '4px 4px' }}
                    />
                ))}
                {tabs.length === 0 && (
                    <div className="flex items-center justify-center h-full text-slate-600">
                        <button onClick={newTab} className="flex items-center gap-2 hover:text-white transition-colors">
                            <Plus size={16} /> New Terminal
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
};
export default TerminalApp;
