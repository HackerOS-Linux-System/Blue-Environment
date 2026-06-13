import React from 'react';
import { ImageIcon, RefreshCw, Check } from 'lucide-react';
import type { UserConfig } from '../../../../types';
import { SystemBridge } from '../../../../utils/systemBridge';
import { applyResolution, applyRefreshRate } from '../display_helpers';

interface Props {
    config: UserConfig;
    onSave: (patch: Partial<UserConfig>) => Promise<void>;
    wallpapers: string[];
    wallpaperPreviews: Map<string, string>;
    onReloadWallpapers: () => void;
    brightness: number;
    onBrightnessChange: (v: number) => void;
    resolution: string;
    onResolutionChange: (v: string) => void;
    refreshRate: number;
    onRefreshRateChange: (v: number) => void;
    resolutionList: string[];
    rateList: number[];
}

const DisplaySection: React.FC<Props> = ({
    config, onSave, wallpapers, wallpaperPreviews, onReloadWallpapers,
    brightness, onBrightnessChange, resolution, onResolutionChange,
    refreshRate, onRefreshRateChange, resolutionList, rateList,
}) => (
    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
        <h2 className="text-2xl font-bold text-white">Display</h2>

        {/* Wallpaper */}
        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
            <label className="block text-sm font-medium text-slate-400 mb-4 flex items-center gap-2">
                <ImageIcon size={16} className="text-blue-400" /> Wallpaper
            </label>
            <div className="grid grid-cols-3 gap-4 max-h-72 overflow-y-auto p-1">
                {wallpapers.map((wp, idx) => (
                    <div key={idx}
                        className={`relative aspect-video rounded-xl overflow-hidden cursor-pointer border-2 transition-all hover:scale-105
                            ${config.wallpaper === wp ? 'border-blue-500 ring-2 ring-blue-500/20' : 'border-transparent'}`}
                        onClick={() => onSave({ wallpaper: wp })}>
                        {wallpaperPreviews.has(wp)
                            ? <img src={wallpaperPreviews.get(wp)} className="w-full h-full object-cover" alt="" />
                            : <div className="w-full h-full bg-slate-700 flex items-center justify-center"><ImageIcon size={24} className="text-slate-500" /></div>
                        }
                        {config.wallpaper === wp && (
                            <div className="absolute inset-0 flex items-center justify-center bg-blue-500/20">
                                <Check size={24} className="text-white" />
                            </div>
                        )}
                    </div>
                ))}
            </div>
            <button onClick={onReloadWallpapers} className="mt-3 text-xs text-slate-500 hover:text-white flex items-center gap-1">
                <RefreshCw size={12} /> Refresh wallpapers
            </button>
        </div>

        {/* Brightness */}
        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
            <label className="block text-sm font-medium text-slate-400 mb-2">Brightness</label>
            <input type="range" min="0" max="100" value={brightness}
                onChange={e => onBrightnessChange(parseInt(e.target.value))}
                onMouseUp={() => SystemBridge.setBrightness(brightness)}
                className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
            <div className="flex justify-between text-xs text-slate-500 mt-1">
                <span>0%</span><span>{brightness}%</span><span>100%</span>
            </div>
        </div>

        {/* Scale */}
        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
            <label className="block text-sm font-medium text-slate-400 mb-2">Display Scale</label>
            <select value={config.displayScale}
                onChange={e => onSave({ displayScale: parseFloat(e.target.value) })}
                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                {['1','1.25','1.5','1.75','2'].map(s => (
                    <option key={s} value={s}>{parseFloat(s)*100}%</option>
                ))}
            </select>
        </div>

        {/* Resolution & Refresh */}
        <div className="grid grid-cols-2 gap-4">
            <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                <label className="block text-sm font-medium text-slate-400 mb-2">Resolution</label>
                <select value={resolution}
                    onChange={async e => { onResolutionChange(e.target.value); await applyResolution(e.target.value); }}
                    className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                    {resolutionList.map(r => <option key={r}>{r}</option>)}
                </select>
            </div>
            <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                <label className="block text-sm font-medium text-slate-400 mb-2">Refresh Rate</label>
                <select value={refreshRate}
                    onChange={async e => { onRefreshRateChange(parseInt(e.target.value)); await applyRefreshRate(parseInt(e.target.value)); }}
                    className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                    {rateList.map(r => <option key={r} value={r}>{r} Hz</option>)}
                </select>
            </div>
        </div>
    </div>
);

export default DisplaySection;
