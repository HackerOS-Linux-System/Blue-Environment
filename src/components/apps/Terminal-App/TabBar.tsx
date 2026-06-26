import React from 'react';
import { Plus, X, Settings } from 'lucide-react';
import { TerminalSession } from './useTerminalSession';

interface Props {
    session: TerminalSession;
    showSettings: boolean;
    onToggleSettings: () => void;
}

const TabBar: React.FC<Props> = ({ session, showSettings, onToggleSettings }) => {
    const { tabs, activeTab, setActiveTab, newTab, closeTab } = session;

    return (
        <div className="shrink-0 flex items-center bg-slate-900 border-b border-white/5 overflow-x-auto">
            {tabs.map(tab => (
                <div
                    key={tab.id}
                    onClick={() => setActiveTab(tab.id)}
                    className={`flex items-center gap-2 px-4 py-2.5 cursor-pointer shrink-0 border-r border-white/5 group transition-colors ${
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
                <button
                    onClick={onToggleSettings}
                    className={`p-1.5 rounded transition-colors ${showSettings ? 'text-blue-400 bg-blue-500/10' : 'text-slate-500 hover:text-white'}`}
                >
                    <Settings size={13} />
                </button>
            </div>
        </div>
    );
};

export default TabBar;
