import React from 'react';
import type { CaptureMode } from './types';
import { Monitor, Crop, AppWindow, Timer } from 'lucide-react';

interface Props {
    mode: CaptureMode;
    label: string;
    description: string;
    isActive: boolean;
    onClick: () => void;
}

const ICONS: Record<CaptureMode, React.ComponentType<{size?: number; className?: string}>> = {
    fullscreen: Monitor,
    region:     Crop,
    window:     AppWindow,
    timer:      Timer,
};

const CaptureButton: React.FC<Props> = ({ mode, label, description, isActive, onClick }) => {
    const Icon = ICONS[mode];
    return (
        <button
            onClick={onClick}
            className={`flex flex-col items-center gap-2 p-4 rounded-2xl border transition-all duration-200 group
                ${isActive
                    ? 'bg-blue-600/20 border-blue-500 shadow-lg shadow-blue-500/10'
                    : 'bg-slate-800/60 border-white/5 hover:border-white/20 hover:bg-slate-700/60'}`}
        >
            <div className={`w-12 h-12 rounded-xl flex items-center justify-center transition-colors
                ${isActive ? 'bg-blue-600' : 'bg-slate-700 group-hover:bg-slate-600'}`}>
                <Icon size={22} className={isActive ? 'text-white' : 'text-slate-300'} />
            </div>
            <div className="text-center">
                <div className={`text-sm font-medium ${isActive ? 'text-blue-300' : 'text-slate-200'}`}>
                    {label}
                </div>
                <div className="text-[10px] text-slate-500 mt-0.5">{description}</div>
            </div>
        </button>
    );
};

export default CaptureButton;
