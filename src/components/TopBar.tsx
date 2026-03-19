import React, { useState, useEffect, memo } from 'react';
import { AppId } from '../types';
import { APPS } from '../constants';
import { Search, Wifi, Bell, Command, CloudSun, Clipboard } from 'lucide-react';
import { SystemBridge } from '../utils/systemBridge';

interface TopBarProps {
    openWindows: { id: string; appId: AppId; isMinimized: boolean; isActive: boolean; workspace: number }[];
    currentWorkspace: number;
    workspaceCount: number;
    onOpenApp: (appId: AppId) => void;
    onToggleWindow: (windowId: string) => void;
    onStartClick: () => void;
    onStartDoubleClick: () => void;
    onToggleControlCenter: () => void;
    onToggleNotifications: () => void;
    onSwitchWorkspace: (index: number) => void;
    isStartMenuOpen: boolean;
    isClipboardOpen: boolean;
    onToggleClipboard: () => void;
}

const TopBar: React.FC<TopBarProps> = ({
    openWindows,
    currentWorkspace,
    workspaceCount,
    onOpenApp,
    onToggleWindow,
    onStartClick,
    onStartDoubleClick,
    onToggleControlCenter,
    onToggleNotifications,
    onSwitchWorkspace,
    isStartMenuOpen,
    isClipboardOpen,
    onToggleClipboard,
}) => {
    const [time, setTime] = useState(new Date());
    const [weather, setWeather] = useState({ temp: '--', condition: 'Loading' });
    const [hasClipboardContent, setHasClipboardContent] = useState(false);

    useEffect(() => {
        const timer = setInterval(() => setTime(new Date()), 1000);
        fetch('https://api.open-meteo.com/v1/forecast?latitude=52.52&longitude=13.41&current_weather=true')
        .then(r => r.json())
        .then(data => {
            if (data.current_weather) {
                setWeather({ temp: data.current_weather.temperature + '°C', condition: 'Clear' });
            }
        })
        .catch(() => {});
        return () => clearInterval(timer);
    }, []);

    useEffect(() => {
        const checkClipboard = async () => {
            const hasText = await SystemBridge.hasText();
            setHasClipboardContent(hasText);
        };
        checkClipboard();
        const interval = setInterval(checkClipboard, 2000);
        return () => clearInterval(interval);
    }, []);

    const handleStartClick = (e: React.MouseEvent) => {
        if (e.detail === 2) onStartDoubleClick();
        else onStartClick();
    };

        const pinnedApps = [AppId.TERMINAL, AppId.EXPLORER, AppId.AI_ASSISTANT, AppId.SETTINGS];

        return (
            <div className="absolute top-0 left-0 right-0 h-12 bg-slate-900/95 backdrop-blur-sm border-b border-white/5 flex items-center justify-between px-3 z-50 select-none">

            {/* Left: Start + search */}
            <div className="flex items-center gap-3 w-1/3">
            <button
            onClick={handleStartClick}
            className={`flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg transition-all duration-200 group
                ${isStartMenuOpen ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/5 text-slate-300 hover:text-white'}
                `}
                title="Start menu (⊞ or Alt+F1) | Full screen (⊞+⊞)"
                >
                <div className="relative">
                <Command size={18} className="group-hover:rotate-12 transition-transform duration-200" />
                <div className="absolute -top-1 -right-1 w-1.5 h-1.5 bg-blue-500 rounded-full" />
                </div>
                <span className="font-bold text-sm tracking-tight hidden sm:block">Blue</span>
                </button>

                <div
                className="hidden md:flex items-center gap-2 bg-slate-800/80 hover:bg-slate-700/80 border border-white/5 rounded-full px-3 py-1 text-xs text-slate-400 cursor-text transition-colors w-44"
                onClick={onStartClick}
                >
                <Search size={12} />
                <span>Search apps…</span>
                </div>
                </div>

                {/* Center: pinned apps */}
                <div className="flex items-center justify-center w-1/3">
                <div className="flex items-center gap-1 bg-slate-800/60 border border-white/5 rounded-2xl px-2 py-1 shadow-lg">
                {pinnedApps.map(appId => {
                    const app = APPS[appId];
                    const openInstances = openWindows.filter(w => w.appId === appId);
                    const isOpen = openInstances.length > 0;
                    const isActive = openInstances.some(w => w.isActive && !w.isMinimized);

                    return (
                        <button
                        key={appId}
                        onClick={() => {
                            const openInstance = openWindows.find(w => w.appId === appId);
                            if (openInstance) onToggleWindow(openInstance.id);
                            else onOpenApp(appId);
                        }}
                        className="relative group p-2 rounded-xl transition-all hover:bg-white/10"
                        title={app.title}
                        >
                        <app.icon
                        size={20}
                        className={`transition-colors duration-200 ${isOpen ? 'text-blue-400' : 'text-slate-400 group-hover:text-slate-200'}`}
                        />
                        {isOpen && (
                            <span className={`absolute -bottom-0.5 left-1/2 -translate-x-1/2 h-0.5 rounded-full transition-all duration-200
                                ${isActive ? 'w-3.5 bg-blue-400' : 'w-1 bg-slate-500'}
                                `} />
                        )}
                        </button>
                    );
                })}
                </div>
                </div>

                {/* Right: workspace dots, weather, clipboard, clock, notifications */}
                <div className="flex items-center justify-end gap-2 w-1/3">

                {/* Workspace switcher dots */}
                <div className="hidden lg:flex items-center gap-1 px-2 py-1 rounded-full hover:bg-white/5 transition-colors">
                {Array.from({ length: workspaceCount }, (_, i) => {
                    const hasWindows = openWindows.some(w => w.workspace === i && !w.isMinimized);
                    return (
                        <button
                        key={i}
                        onClick={() => onSwitchWorkspace(i)}
                        title={`Workspace ${i + 1} (Alt+⊞+${i + 1})`}
                        className={`transition-all duration-200 rounded-full
                            ${i === currentWorkspace
                                ? 'w-4 h-2 bg-blue-400'
                    : `w-2 h-2 ${hasWindows ? 'bg-slate-400' : 'bg-slate-600'} hover:bg-slate-300`
                            }
                            `}
                            />
                    );
                })}
                </div>

                {/* Weather */}
                <div className="hidden lg:flex items-center gap-2 px-2 py-1 rounded-full hover:bg-white/5 cursor-default transition-colors">
                <CloudSun size={15} className="text-yellow-300" />
                <span className="text-xs font-medium text-slate-200">{weather.temp}</span>
                </div>

                {/* Clipboard */}
                <button
                onClick={onToggleClipboard}
                className={`relative p-2 rounded-full transition-colors group ${
                    isClipboardOpen ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-300'
                }`}
                title="Schowek (Ctrl+Shift+V)"
                >
                <Clipboard size={15} className="group-hover:text-white" />
                {hasClipboardContent && (
                    <span className="absolute top-1.5 right-1.5 w-1.5 h-1.5 bg-blue-500 rounded-full" />
                )}
                </button>

                {/* Clock + control center */}
                <button
                onClick={onToggleControlCenter}
                className="flex items-center gap-2 px-3 py-1.5 rounded-full hover:bg-white/10 transition-colors border border-transparent hover:border-white/5"
                >
                <Wifi size={13} className="text-slate-300" />
                <span className="text-xs font-medium text-slate-200 tabular-nums">
                {time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                </span>
                </button>

                {/* Notifications */}
                <button
                onClick={onToggleNotifications}
                className="relative p-2 rounded-full hover:bg-white/10 transition-colors group"
                >
                <Bell size={15} className="text-slate-300 group-hover:text-white" />
                <span className="absolute top-1.5 right-1.5 w-1.5 h-1.5 bg-red-500 border border-slate-900 rounded-full" />
                </button>
                </div>
                </div>
        );
};

export default memo(TopBar);
