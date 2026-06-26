import React from 'react';
import { PanelTop } from 'lucide-react';
import type { UserConfig } from '../../../../types';

interface Props { config: UserConfig; onSave: (p: Partial<UserConfig>) => Promise<void>; }

const PanelSection: React.FC<Props> = ({ config, onSave }) => (
    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
        <h2 className="text-2xl font-bold text-white">Panel</h2>

        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-6">
            <div className="flex items-center gap-3 text-slate-400">
                <PanelTop size={16} className="text-blue-400" />
                <span className="text-sm">Appearance of the top bar — applies live.</span>
            </div>

            <div>
                <label className="block text-sm font-medium text-slate-400 mb-1">
                    Height — {config.panelSize ?? 48}px
                </label>
                <input
                    type="range" min={36} max={64} step={1}
                    value={config.panelSize ?? 48}
                    onChange={e => onSave({ panelSize: parseInt(e.target.value, 10) })}
                    className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
                />
            </div>

            <div>
                <label className="block text-sm font-medium text-slate-400 mb-1">
                    Opacity — {Math.round((config.panelOpacity ?? 0.95) * 100)}%
                </label>
                <input
                    type="range" min={0.5} max={1} step={0.05}
                    value={config.panelOpacity ?? 0.95}
                    onChange={e => onSave({ panelOpacity: parseFloat(e.target.value) })}
                    className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
                />
            </div>

            <p className="text-xs text-slate-500">
                Window snapping is calibrated for the default 48px height — larger or smaller
                values may shift the snap zones slightly.
            </p>
        </div>
    </div>
);

export default PanelSection;
