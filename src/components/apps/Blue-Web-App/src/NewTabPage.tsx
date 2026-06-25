import React, { useRef } from 'react';
import { Globe, ExternalLink, Info } from 'lucide-react';
import { SPEED_DIALS } from './types';
import { SystemBridge } from '../../../../utils/systemBridge';

interface Props { onNavigate: (url: string) => void; error?: string | null; }

const NewTabPage: React.FC<Props> = ({ onNavigate, error }) => {
    const inputRef = useRef<HTMLInputElement>(null);
    const [input, setInput] = React.useState('');
    React.useEffect(() => { setTimeout(() => inputRef.current?.focus(), 80); }, []);

    return (
        <div className="flex-1 flex flex-col items-center justify-center gap-8 p-8 overflow-y-auto">
            <div className="text-center">
                <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center mx-auto mb-4 shadow-lg shadow-blue-500/30">
                    <Globe size={32} className="text-white"/>
                </div>
                <h1 className="text-xl font-bold text-white mb-1">Blue Web</h1>
                <p className="text-slate-400 text-sm">Sites open in native windows for full compatibility</p>
            </div>

            <div className="w-full max-w-xl">
                <input ref={inputRef} value={input} onChange={e => setInput(e.target.value)}
                    onKeyDown={e => { if (e.key === 'Enter' && input.trim()) onNavigate(input); }}
                    placeholder="Search DuckDuckGo or enter URL…"
                    className="w-full bg-slate-800 border border-white/10 rounded-2xl px-5 py-3.5 text-sm text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/50"/>
            </div>

            <div className="grid grid-cols-4 gap-3 w-full max-w-xl">
                {SPEED_DIALS.map(sd => (
                    <button key={sd.url} onClick={() => onNavigate(sd.url)}
                        className="flex flex-col items-center gap-2 p-3 bg-slate-800/50 hover:bg-slate-700/60 border border-white/5 rounded-xl transition-colors group">
                        <span className="text-2xl">{sd.icon}</span>
                        <span className="text-[11px] text-slate-400 group-hover:text-white text-center leading-tight">{sd.label}</span>
                    </button>
                ))}
            </div>

            {error && (
                <div className="flex items-start gap-2 bg-red-500/10 border border-red-500/20 rounded-xl p-3 max-w-lg text-xs text-red-300">
                    <Info size={14} className="shrink-0 mt-0.5"/><div>{error}</div>
                </div>
            )}

            <div className="flex items-center gap-2 text-xs text-slate-600">
                <Globe size={11}/>
                {SystemBridge.isTauri() ? 'Sites open in native webview windows — full web compatibility' : 'Dev mode — sites open in new browser tabs'}
            </div>
        </div>
    );
};

export default NewTabPage;
