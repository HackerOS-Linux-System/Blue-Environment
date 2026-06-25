import React from 'react';
import {
    FolderOpen, Plus, Save, Terminal as TerminalIcon, Search,
    GitBranch, Command, RefreshCw, Zap,
} from 'lucide-react';
import { OpenFile, SidebarTab } from './types';

interface Props {
    sidebarCollapsed: boolean;
    setSidebarCollapsed: (fn: (s: boolean) => boolean) => void;
    onNewFile: () => void;
    onSave: () => void;
    canSave: boolean;
    showTerminal: boolean;
    setShowTerminal: (fn: (s: boolean) => boolean) => void;
    sidebarTab: SidebarTab;
    setSidebarTab: (t: SidebarTab) => void;
    onCommandPalette: () => void;
    activeFile?: OpenFile;
    lspStatus: Record<string, boolean>;
    onRefresh: () => void;
}

const Toolbar: React.FC<Props> = ({
    sidebarCollapsed, setSidebarCollapsed, onNewFile, onSave, canSave,
    showTerminal, setShowTerminal, sidebarTab, setSidebarTab,
    onCommandPalette, activeFile, lspStatus, onRefresh,
}) => (
    <div className="h-10 bg-slate-800 border-b border-white/5 flex items-center px-3 gap-1 shrink-0">
        <button onClick={() => setSidebarCollapsed(s => !s)} className="p-1.5 hover:bg-white/10 rounded" title="Toggle sidebar">
            <FolderOpen size={16} />
        </button>
        <button onClick={onNewFile} className="p-1.5 hover:bg-white/10 rounded" title="New file"><Plus size={16} /></button>
        <button onClick={onSave} disabled={!canSave}
            className="p-1.5 hover:bg-white/10 rounded disabled:opacity-40" title="Save (Ctrl+S)"><Save size={16} /></button>
        <div className="w-px h-5 bg-white/10 mx-1" />
        <button onClick={() => setShowTerminal(s => !s)}
            className={`p-1.5 rounded ${showTerminal ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Terminal">
            <TerminalIcon size={16} />
        </button>
        <button onClick={() => setSidebarTab('search')}
            className={`p-1.5 rounded ${sidebarTab === 'search' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Search">
            <Search size={16} />
        </button>
        <button onClick={() => setSidebarTab('git')}
            className={`p-1.5 rounded ${sidebarTab === 'git' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Git">
            <GitBranch size={16} />
        </button>
        <button onClick={() => setSidebarTab('dev')}
            className={`p-1.5 rounded ${sidebarTab === 'dev' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`} title="Active Dev Mode">
            <Zap size={16} />
        </button>
        <button onClick={onCommandPalette} className="p-1.5 hover:bg-white/10 rounded" title="Command Palette (Ctrl+Shift+P)">
            <Command size={16} />
        </button>
        <div className="flex-1" />
        <div className="text-xs text-slate-500 truncate max-w-64">{activeFile?.path || 'No file open'}</div>
        {Object.entries(lspStatus).map(([lang, active]) => (
            <div key={lang} title={`LSP: ${lang} ${active ? 'active' : 'inactive'}`}
                className={`w-2 h-2 rounded-full ${active ? 'bg-green-400' : 'bg-slate-600'}`} />
        ))}
        <button onClick={onRefresh} className="p-1.5 hover:bg-white/10 rounded"><RefreshCw size={14} /></button>
    </div>
);

export default Toolbar;
