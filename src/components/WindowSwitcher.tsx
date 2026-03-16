import React, { memo } from 'react';
import { WindowState, AppId } from '../types';
import { APPS } from '../constants';

interface WindowSwitcherProps {
    windows: WindowState[];
    selectedIndex: number;
    isVisible: boolean;
}

const WindowSwitcher: React.FC<WindowSwitcherProps> = ({ windows, selectedIndex, isVisible }) => {
    if (!isVisible || windows.length === 0) return null;

    return (
        <div className="absolute inset-0 z-[200] flex items-center justify-center pointer-events-none">
            {/* Backdrop */}
            <div className="absolute inset-0 bg-black/40 backdrop-blur-sm animate-in fade-in duration-150" />

            {/* Switcher card */}
            <div className="relative bg-slate-900/95 border border-white/10 p-5 rounded-2xl shadow-2xl shadow-black/50 flex flex-col items-center gap-4 max-w-[88vw] animate-in fade-in zoom-in-95 duration-150">

                {/* Hint */}
                <div className="flex items-center gap-3 text-xs text-slate-500 mb-1">
                    <kbd className="px-2 py-0.5 bg-slate-800 border border-white/10 rounded text-slate-400">Alt</kbd>
                    <span>+</span>
                    <kbd className="px-2 py-0.5 bg-slate-800 border border-white/10 rounded text-slate-400">Tab</kbd>
                    <span className="text-slate-600">to cycle</span>
                    <span className="ml-2 text-slate-600">release</span>
                    <kbd className="px-2 py-0.5 bg-slate-800 border border-white/10 rounded text-slate-400">Alt</kbd>
                    <span className="text-slate-600">to switch</span>
                </div>

                {/* App icons */}
                <div className="flex gap-3 overflow-x-auto max-w-[80vw] pb-1">
                    {windows.map((win, index) => {
                        const AppIcon = APPS[win.appId as AppId]?.icon;
                        const isSelected = index === selectedIndex;

                        return (
                            <div
                                key={win.id}
                                className={`flex flex-col items-center gap-2 p-3 rounded-xl transition-all duration-150 w-28 shrink-0
                                    ${isSelected
                                        ? 'bg-blue-600/80 text-white scale-105 shadow-lg shadow-blue-500/30 ring-2 ring-blue-400/50'
                                        : 'bg-slate-800/60 text-slate-300'
                                    }
                                `}
                            >
                                <div className="w-12 h-12 flex items-center justify-center">
                                    {AppIcon
                                        ? <AppIcon size={32} />
                                        : <div className="w-8 h-8 bg-slate-600 rounded-lg" />
                                    }
                                </div>
                                <span className="text-xs font-medium truncate w-full text-center leading-tight">
                                    {win.title}
                                </span>
                                {/* Minimized badge */}
                                {win.isMinimized && (
                                    <span className="text-[10px] bg-slate-700 text-slate-400 px-1.5 rounded-full">hidden</span>
                                )}
                                {/* Workspace badge */}
                                <span className="text-[10px] text-slate-500">
                                    WS {(win.workspace ?? 0) + 1}
                                </span>
                            </div>
                        );
                    })}
                </div>

                {/* Selected title */}
                <div className="text-sm font-semibold text-white/80">
                    {windows[selectedIndex]?.title ?? ''}
                </div>
            </div>
        </div>
    );
};

export default memo(WindowSwitcher);
