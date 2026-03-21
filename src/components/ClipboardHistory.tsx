import React, { useState, useEffect } from 'react';
import { Clipboard, Copy, Trash2, X, Clock } from 'lucide-react';
import { SystemBridge } from '../utils/systemBridge';

interface ClipboardHistoryItem {
    id: string;
    content: string;
    timestamp: number;
}

const ClipboardHistory: React.FC<{ onClose: () => void }> = ({ onClose }) => {
    const [history, setHistory] = useState<ClipboardHistoryItem[]>([]);
    const [currentContent, setCurrentContent] = useState('');

    useEffect(() => {
        loadHistory();
        const interval = setInterval(checkClipboard, 500);
        return () => clearInterval(interval);
    }, []);

    const loadHistory = async () => {
        const hist = await SystemBridge.getClipboardHistory();
        setHistory(hist);
    };

    const checkClipboard = async () => {
        const text = await SystemBridge.readText();
        if (text && text !== currentContent) {
            setCurrentContent(text);
            await SystemBridge.addToClipboardHistory(text);
            loadHistory();
        }
    };

    const handleCopy = async (content: string) => {
        await SystemBridge.copyText(content);
    };

    const handleClear = async () => {
        await SystemBridge.clearClipboardHistory();
        loadHistory();
    };

    const formatTime = (timestamp: number) => {
        const date = new Date(timestamp);
        return date.toLocaleTimeString();
    };

    return (
        <div className="absolute top-14 right-4 w-96 bg-slate-900/98 border border-white/10 rounded-2xl shadow-2xl p-4 z-50 animate-in fade-in slide-in-from-top-3 duration-150 backdrop-blur-xl">
        <div className="flex items-center justify-between mb-3">
        <h3 className="font-semibold text-white flex items-center gap-2">
        <Clipboard size={16} /> Historia schowka
        </h3>
        <button onClick={onClose} className="text-slate-400 hover:text-white p-1 rounded-full hover:bg-white/10">
        <X size={16} />
        </button>
        </div>
        <div className="max-h-96 overflow-y-auto space-y-2">
        {history.map(item => (
            <div key={item.id} className="bg-slate-800 rounded-xl p-3 group">
            <div className="flex items-center justify-between text-xs text-slate-500 mb-1">
            <span className="flex items-center gap-1"><Clock size={10} /> {formatTime(item.timestamp)}</span>
            <button
            onClick={() => handleCopy(item.content)}
            className="opacity-0 group-hover:opacity-100 p-1 hover:bg-white/10 rounded"
            >
            <Copy size={12} />
            </button>
            </div>
            <pre className="text-xs text-slate-200 whitespace-pre-wrap break-words font-sans">{item.content}</pre>
            </div>
        ))}
        {history.length === 0 && (
            <div className="text-center text-slate-500 py-8">Schowek jest pusty</div>
        )}
        </div>
        {history.length > 0 && (
            <button
            onClick={handleClear}
            className="mt-3 w-full flex items-center justify-center gap-2 bg-red-600/20 hover:bg-red-500/40 text-red-300 px-3 py-2 rounded-lg text-sm font-medium transition-colors"
            >
            <Trash2 size={14} /> Wyczyść historię
            </button>
        )}
        </div>
    );
};

export default ClipboardHistory;
