import React, { useState, useEffect } from 'react';
import { Clipboard, Copy, Trash2, X } from 'lucide-react';
import { SystemBridge } from '../utils/systemBridge';

interface ClipboardPanelProps {
    onClose: () => void;
}

const ClipboardPanel: React.FC<ClipboardPanelProps> = ({ onClose }) => {
    const [content, setContent] = useState('');
    const [loading, setLoading] = useState(true);

    const refresh = async () => {
        setLoading(true);
        const text = await SystemBridge.readText();
        setContent(text);
        setLoading(false);
    };

    useEffect(() => {
        refresh();
        // Odświeżaj co sekundę, aby reagować na zmiany z zewnątrz
        const interval = setInterval(refresh, 1000);
        return () => clearInterval(interval);
    }, []);

    const handleCopyAgain = async () => {
        if (content) {
            await SystemBridge.copyText(content);
            // Opcjonalnie: krótkie powiadomienie
        }
    };

    const handleClear = async () => {
        await SystemBridge.clear();
        setContent('');
    };

    return (
        <div className="absolute top-14 right-4 w-80 bg-slate-900/98 border border-white/10 rounded-2xl shadow-2xl p-4 z-50 animate-in fade-in slide-in-from-top-3 duration-150 backdrop-blur-xl">
        <div className="flex items-center justify-between mb-3">
        <h3 className="font-semibold text-white flex items-center gap-2">
        <Clipboard size={16} /> Schowek
        </h3>
        <button
        onClick={onClose}
        className="text-slate-400 hover:text-white p-1 rounded-full hover:bg-white/10"
        >
        <X size={16} />
        </button>
        </div>
        <div className="bg-slate-800 rounded-xl p-3 min-h-[100px] max-h-60 overflow-y-auto mb-3">
        {loading ? (
            <div className="flex items-center justify-center h-full text-slate-500 text-sm">Ładowanie…</div>
        ) : content ? (
            <pre className="text-xs text-slate-200 whitespace-pre-wrap break-words font-sans">{content}</pre>
        ) : (
            <div className="flex items-center justify-center h-full text-slate-500 text-sm">Schowek jest pusty</div>
        )}
        </div>
        <div className="flex gap-2">
        <button
        onClick={handleCopyAgain}
        disabled={!content}
        className="flex-1 flex items-center justify-center gap-2 bg-blue-600 hover:bg-blue-500 disabled:bg-slate-700 disabled:text-slate-500 text-white px-3 py-2 rounded-lg text-sm font-medium transition-colors"
        >
        <Copy size={14} /> Kopiuj ponownie
        </button>
        <button
        onClick={handleClear}
        disabled={!content}
        className="flex items-center justify-center gap-2 bg-red-600/20 hover:bg-red-500/40 text-red-300 disabled:opacity-30 disabled:hover:bg-transparent px-3 py-2 rounded-lg text-sm font-medium transition-colors"
        >
        <Trash2 size={14} />
        </button>
        </div>
        </div>
    );
};

export default ClipboardPanel;
