import React, { useRef } from 'react';
import { ArrowLeft, ArrowRight, RefreshCw, Home, Search, Lock, Unlock, Star, StarOff, Bookmark, History, X } from 'lucide-react';

interface Props {
    url: string; isNew: boolean; isSecure: boolean; isBookmarked: boolean;
    canGoBack: boolean; canGoForward: boolean;
    onBack: () => void; onForward: () => void; onRefresh: () => void; onHome: () => void;
    onNavigate: (url: string) => void;
    onToggleBookmark: () => void;
    onToggleBookmarks: () => void;
    onToggleHistory: () => void;
    panelOpen: 'bookmarks' | 'history' | 'none';
}

const AddressBar: React.FC<Props> = ({
    url, isNew, isSecure, isBookmarked,
    canGoBack, canGoForward,
    onBack, onForward, onRefresh, onHome, onNavigate,
    onToggleBookmark, onToggleBookmarks, onToggleHistory, panelOpen,
}) => {
    const inputRef = useRef<HTMLInputElement>(null);
    const [input, setInput] = React.useState(isNew ? '' : url);
    React.useEffect(() => setInput(isNew ? '' : url), [url, isNew]);

    return (
        <div className="flex items-center gap-1.5 px-2 py-1.5 bg-slate-900 border-b border-white/5 shrink-0">
            <button onClick={onBack} disabled={!canGoBack} className="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><ArrowLeft size={15}/></button>
            <button onClick={onForward} disabled={!canGoForward} className="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><ArrowRight size={15}/></button>
            <button onClick={onRefresh} disabled={isNew} className="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30"><RefreshCw size={14}/></button>
            <button onClick={onHome} className="p-1.5 rounded-lg hover:bg-white/10"><Home size={14}/></button>
            <div className="flex-1 flex items-center gap-1.5 bg-slate-800 border border-white/10 rounded-xl px-3 py-1.5 focus-within:border-blue-500/40">
                {isNew ? <Search size={13} className="text-slate-500 shrink-0"/> : isSecure ? <Lock size={13} className="text-green-400 shrink-0"/> : <Unlock size={13} className="text-amber-400 shrink-0"/>}
                <input ref={inputRef} value={input} onChange={e => setInput(e.target.value)}
                    onFocus={e => e.target.select()}
                    onKeyDown={e => { if (e.key === 'Enter') onNavigate(input); if (e.key === 'Escape') { setInput(isNew ? '' : url); inputRef.current?.blur(); }}}
                    placeholder="Search or enter URL…"
                    className="flex-1 bg-transparent text-sm text-white placeholder:text-slate-500 focus:outline-none min-w-0"/>
                {input && <button onClick={() => { setInput(''); inputRef.current?.focus(); }}><X size={12} className="text-slate-500 hover:text-white"/></button>}
            </div>
            <button onClick={onToggleBookmark} disabled={isNew} className="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-30">
                {isBookmarked ? <Star size={15} className="text-yellow-400 fill-yellow-400"/> : <StarOff size={15} className="text-slate-400"/>}
            </button>
            <button onClick={onToggleBookmarks} className={`p-1.5 rounded-lg hover:bg-white/10 ${panelOpen==='bookmarks'?'text-blue-400':''}`}><Bookmark size={15}/></button>
            <button onClick={onToggleHistory}   className={`p-1.5 rounded-lg hover:bg-white/10 ${panelOpen==='history' ?'text-blue-400':''}`}><History size={15}/></button>
        </div>
    );
};

export default AddressBar;
