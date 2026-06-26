import React from 'react';
import { X } from 'lucide-react';
import { THEMES, ThemeName } from './themes';
import { TerminalSession } from './useTerminalSession';

interface Props {
    session: TerminalSession;
    onClose: () => void;
}

const SettingsPanel: React.FC<Props> = ({ session, onClose }) => {
    const { themeName, setThemeName, fontSize, setFontSize } = session;

    return (
        <div className="shrink-0 flex items-center gap-4 px-4 py-2 bg-slate-900 border-b border-white/5 text-sm">
            <div className="flex items-center gap-2">
                <span className="text-slate-400 text-xs">Theme:</span>
                <select
                    value={themeName}
                    onChange={e => setThemeName(e.target.value as ThemeName)}
                    className="bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-white focus:outline-none"
                >
                    {Object.keys(THEMES).map(t => <option key={t} value={t}>{t}</option>)}
                </select>
            </div>
            <div className="flex items-center gap-2">
                <span className="text-slate-400 text-xs">Font size:</span>
                <input
                    type="number" value={fontSize} min={10} max={24}
                    onChange={e => setFontSize(parseInt(e.target.value, 10) || 14)}
                    className="bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-white w-14 focus:outline-none"
                />
            </div>
            <button onClick={onClose} className="ml-auto text-slate-500 hover:text-white">
                <X size={13} />
            </button>
        </div>
    );
};

export default SettingsPanel;
