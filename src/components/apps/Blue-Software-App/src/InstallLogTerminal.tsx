import React from 'react';
import { Terminal, Check, AlertCircle, Loader2, X } from 'lucide-react';
import { InstallLog } from './types';

interface Props {
    log:      InstallLog;
    onClose:  () => void;
}

const InstallLogTerminal: React.FC<Props> = ({ log, onClose }) => (
    <div className="border-t border-white/5 bg-slate-950 shrink-0" style={{ maxHeight: 160 }}>
        <div className="flex items-center justify-between px-3 py-1.5 border-b border-white/5">
            <span className="text-xs text-slate-400 flex items-center gap-1.5">
                <Terminal size={12} />
                {log.done
                    ? log.success
                        ? <span className="text-green-400 flex items-center gap-1"><Check size={11} /> Done</span>
                        : <span className="text-red-400 flex items-center gap-1"><AlertCircle size={11} /> Failed</span>
                    : <span className="flex items-center gap-1"><Loader2 size={11} className="animate-spin" /> Working…</span>}
            </span>
            {log.done && (
                <button onClick={onClose} className="text-slate-500 hover:text-white"><X size={12} /></button>
            )}
        </div>
        <div className="overflow-y-auto p-3 font-mono text-xs text-slate-300 space-y-0.5" style={{ maxHeight: 110 }}>
            {log.lines.map((line, i) => (
                <div key={i} className={
                    line.startsWith('✓') ? 'text-green-400' :
                    line.startsWith('✗') ? 'text-red-400' : ''
                }>{line}</div>
            ))}
        </div>
    </div>
);

export default InstallLogTerminal;
