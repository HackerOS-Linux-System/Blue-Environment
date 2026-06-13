import React from 'react';
import { Trash2, Download, Copy, FolderOpen } from 'lucide-react';
import type { Screenshot } from './types';
import { SystemBridge } from '../../../utils/systemBridge';

interface Props {
    screenshots: Screenshot[];
    selected: string | null;
    onSelect: (id: string) => void;
    onDelete: (id: string) => void;
}

const HistoryPanel: React.FC<Props> = ({ screenshots, selected, onSelect, onDelete }) => {
    if (screenshots.length === 0) {
        return (
            <div className="flex flex-col items-center justify-center h-full text-slate-600 gap-3 p-6">
                <div className="w-16 h-16 bg-slate-800 rounded-2xl flex items-center justify-center">
                    <FolderOpen size={28} />
                </div>
                <p className="text-sm">No screenshots yet</p>
                <p className="text-xs text-slate-700">Capture something to see it here</p>
            </div>
        );
    }

    return (
        <div className="flex flex-col gap-2 p-3 overflow-y-auto h-full">
            {screenshots.slice().reverse().map(s => (
                <div
                    key={s.id}
                    onClick={() => onSelect(s.id)}
                    className={`group relative rounded-xl overflow-hidden border cursor-pointer transition-all
                        ${selected === s.id
                            ? 'border-blue-500 ring-2 ring-blue-500/30'
                            : 'border-white/5 hover:border-white/20'}`}
                >
                    <img
                        src={s.dataUrl}
                        alt="screenshot"
                        className="w-full h-20 object-cover bg-slate-800"
                    />
                    <div className="absolute inset-0 bg-gradient-to-t from-black/70 to-transparent opacity-0 group-hover:opacity-100 transition-opacity flex items-end p-2 gap-1">
                        <button
                            onClick={e => { e.stopPropagation(); SystemBridge.writeClipboardImage(s.dataUrl); }}
                            className="p-1.5 bg-white/20 hover:bg-white/30 rounded-lg"
                            title="Copy to clipboard"
                        >
                            <Copy size={12} />
                        </button>
                        <button
                            onClick={e => { e.stopPropagation(); SystemBridge.saveFile(s.path, s.dataUrl); }}
                            className="p-1.5 bg-white/20 hover:bg-white/30 rounded-lg"
                            title="Save as"
                        >
                            <Download size={12} />
                        </button>
                        <button
                            onClick={e => { e.stopPropagation(); onDelete(s.id); }}
                            className="p-1.5 bg-red-500/40 hover:bg-red-500/60 rounded-lg ml-auto"
                            title="Delete"
                        >
                            <Trash2 size={12} />
                        </button>
                    </div>
                    <div className="px-2 py-1 bg-slate-900/80 text-[10px] text-slate-400 flex justify-between">
                        <span>{s.width}×{s.height}</span>
                        <span>{new Date(s.timestamp).toLocaleTimeString()}</span>
                    </div>
                </div>
            ))}
        </div>
    );
};

export default HistoryPanel;
