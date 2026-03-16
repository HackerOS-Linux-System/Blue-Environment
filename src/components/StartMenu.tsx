import React, { useState, useEffect, useMemo } from 'react';
import { APPS } from '../constants';
import { AppId, DesktopEntry } from '../types';
import { SystemBridge } from '../utils/systemBridge';
import {
    Search, Power, Grid, User, Box, Terminal as TerminalIcon,
    LogOut, Moon, RefreshCcw, HardDrive, Clock, ChevronRight,
    Loader2
} from 'lucide-react';

interface CachedApp {
    id: string;
    name: string;
    comment: string;
    icon: string;
    exec: string;
    categories: string[];
    desktop_file: string;
    is_external: boolean;
}

interface StartMenuProps {
    isOpen: boolean;
    isFullScreen: boolean;
    onOpenApp: (appId: string, isExternal?: boolean, exec?: string) => void;
    onClose: () => void;
    onToggleFullScreen: () => void;
}

// Category display names and order
const CATEGORY_ORDER = [
    { key: 'Recent',      label: 'Recent',        keys: ['Recent'] },
    { key: 'Internet',    label: 'Internet',       keys: ['Network', 'WebBrowser'] },
    { key: 'Media',       label: 'Multimedia',     keys: ['AudioVideo', 'Audio', 'Video'] },
    { key: 'Graphics',    label: 'Graphics',       keys: ['Graphics'] },
    { key: 'Office',      label: 'Office',         keys: ['Office'] },
    { key: 'Dev',         label: 'Development',    keys: ['Development', 'IDE'] },
    { key: 'Games',       label: 'Games',          keys: ['Game'] },
    { key: 'System',      label: 'System',         keys: ['System', 'Settings', 'Utility'] },
    { key: 'Other',       label: 'Other',          keys: [] },
];

function getCategory(app: CachedApp): string {
    for (const cat of CATEGORY_ORDER) {
        if (cat.keys.some(k => app.categories.includes(k))) return cat.key;
    }
    return 'Other';
}

const AppIcon: React.FC<{ icon: string; name: string; size?: number }> = ({ icon, name, size = 32 }) => {
    const [failed, setFailed] = useState(false);
    const isUrl = icon.startsWith('http') || icon.startsWith('file://');

    if (!failed && isUrl) {
        return (
            <img
                src={icon}
                alt={name}
                width={size}
                height={size}
                className="object-contain"
                onError={() => setFailed(true)}
            />
        );
    }
    // Fallback: colored letter avatar
    const hue = name.charCodeAt(0) * 37 % 360;
    return (
        <div
            className="flex items-center justify-center rounded-lg font-bold text-white"
            style={{ width: size, height: size, background: `hsl(${hue},60%,40%)`, fontSize: size * 0.45 }}
        >
            {name.charAt(0).toUpperCase()}
        </div>
    );
};

const StartMenu: React.FC<StartMenuProps> = ({
    isOpen, isFullScreen, onOpenApp, onClose, onToggleFullScreen
}) => {
    const [searchTerm, setSearchTerm] = useState('');
    const [systemApps, setSystemApps] = useState<CachedApp[]>([]);
    const [recentApps, setRecentApps] = useState<string[]>([]);
    const [showPowerMenu, setShowPowerMenu] = useState(false);
    const [loading, setLoading] = useState(false);
    const [activeCategory, setActiveCategory] = useState('Recent');

    useEffect(() => {
        if (!isOpen) return;
        setLoading(true);
        Promise.all([
            SystemBridge.getSystemApps(false),
            SystemBridge.getRecentApps(),
        ]).then(([apps, recent]) => {
            setSystemApps(apps);
            setRecentApps(recent);
            setLoading(false);
        });
    }, [isOpen]);

    const internalApps = useMemo(() =>
        Object.values(APPS).filter(a => !a.isExternal),
        []
    );

    const allApps = useMemo(() => {
        const term = searchTerm.toLowerCase();
        if (term) {
            const filtered: CachedApp[] = [];
            // Internal Blue apps first
            internalApps
                .filter(a => a.title.toLowerCase().includes(term))
                .forEach(a => filtered.push({
                    id: a.id as string,
                    name: a.title,
                    comment: '',
                    icon: '',
                    exec: '',
                    categories: ['Blue'],
                    desktop_file: '',
                    is_external: false,
                }));
            // System apps
            systemApps
                .filter(a => a.name.toLowerCase().includes(term) || a.comment.toLowerCase().includes(term))
                .forEach(a => filtered.push(a));
            return filtered;
        }
        return systemApps;
    }, [searchTerm, systemApps, internalApps]);

    // Group apps by category
    const grouped = useMemo(() => {
        const map: Record<string, CachedApp[]> = {};
        for (const app of allApps) {
            const cat = getCategory(app);
            if (!map[cat]) map[cat] = [];
            map[cat].push(app);
        }
        // Recent
        if (recentApps.length > 0 && !searchTerm) {
            const recentList = recentApps
                .map(id => allApps.find(a => a.id === id))
                .filter(Boolean) as CachedApp[];
            if (recentList.length) map['Recent'] = recentList.slice(0, 8);
        }
        return map;
    }, [allApps, recentApps, searchTerm]);

    const visibleCategories = CATEGORY_ORDER.filter(c => grouped[c.key]?.length > 0);

    const handleLaunch = (app: CachedApp) => {
        if (app.is_external || app.exec) {
            onOpenApp(app.id, true, app.exec);
        } else {
            onOpenApp(app.id, false);
        }
        SystemBridge.recordAppLaunch(app.id);
        onClose();
    };

    const handleRefresh = async () => {
        setLoading(true);
        const apps = await SystemBridge.getSystemApps(true);
        setSystemApps(apps);
        setLoading(false);
    };

    if (!isOpen) return null;

    const PowerMenu = () => (
        <div className="absolute bottom-14 left-4 bg-slate-800 border border-white/10 rounded-xl shadow-2xl p-2 flex flex-col gap-1 w-48 z-50 animate-in fade-in slide-in-from-bottom-2">
            {[
                { action: 'shutdown', icon: Power, label: 'Shutdown', cls: 'hover:bg-red-500/20 hover:text-red-400' },
                { action: 'reboot', icon: RefreshCcw, label: 'Restart', cls: 'hover:bg-white/10' },
                { action: 'suspend', icon: Moon, label: 'Sleep', cls: 'hover:bg-white/10' },
                { action: 'hibernate', icon: HardDrive, label: 'Hibernate', cls: 'hover:bg-white/10' },
            ].map(({ action, icon: Icon, label, cls }) => (
                <button key={action} onClick={() => SystemBridge.powerAction(action)}
                    className={`flex items-center gap-3 p-2 ${cls} rounded-lg transition-colors text-left text-sm text-slate-200`}>
                    <Icon size={16} /> {label}
                </button>
            ))}
            <div className="h-px bg-white/10 my-1" />
            <button onClick={() => SystemBridge.powerAction('logout')}
                className="flex items-center gap-3 p-2 hover:bg-white/10 rounded-lg transition-colors text-left text-sm text-slate-200">
                <LogOut size={16} /> Log Out
            </button>
        </div>
    );

    // ── FULL SCREEN MODE ────────────────────────────────────────────────────
    if (isFullScreen) {
        return (
            <div
                className="absolute inset-0 bg-slate-900/97 backdrop-blur-xl z-40 flex animate-in fade-in duration-150"
                onClick={onClose}
            >
                {/* Sidebar categories */}
                <div className="w-48 border-r border-white/5 flex flex-col pt-16 px-3 gap-1 shrink-0" onClick={e => e.stopPropagation()}>
                    {visibleCategories.map(cat => (
                        <button
                            key={cat.key}
                            onClick={() => setActiveCategory(cat.key)}
                            className={`px-3 py-2 rounded-xl text-sm font-medium text-left transition-colors ${activeCategory === cat.key ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/5 hover:text-white'}`}
                        >
                            {cat.key === 'Recent' && <Clock size={12} className="inline mr-2 opacity-70" />}
                            {cat.label}
                        </button>
                    ))}
                </div>

                {/* Main grid */}
                <div className="flex-1 flex flex-col" onClick={e => e.stopPropagation()}>
                    {/* Search */}
                    <div className="px-8 pt-12 pb-6">
                        <div className="relative max-w-xl">
                            <Search className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" size={20} />
                            <input
                                type="text" autoFocus
                                placeholder="Search applications…"
                                className="w-full bg-slate-800 border border-white/10 rounded-2xl py-3 pl-12 pr-4 text-white text-lg focus:outline-none focus:border-blue-500/60"
                                value={searchTerm}
                                onChange={e => setSearchTerm(e.target.value)}
                            />
                        </div>
                    </div>

                    {/* Apps grid */}
                    <div className="flex-1 overflow-y-auto px-8 pb-8">
                        {loading && (
                            <div className="flex items-center gap-2 text-slate-500 text-sm">
                                <Loader2 size={16} className="animate-spin" /> Loading apps…
                            </div>
                        )}

                        {!loading && searchTerm ? (
                            <div className="grid grid-cols-5 gap-4">
                                {allApps.slice(0, 30).map(app => (
                                    <AppButton key={app.id} app={app} onLaunch={handleLaunch} size="large" />
                                ))}
                                {allApps.length === 0 && (
                                    <p className="col-span-5 text-slate-500 text-sm">No results for "{searchTerm}"</p>
                                )}
                            </div>
                        ) : (
                            <div className="grid grid-cols-5 gap-4">
                                {(grouped[activeCategory] || []).map(app => (
                                    <AppButton key={app.id} app={app} onLaunch={handleLaunch} size="large" />
                                ))}
                            </div>
                        )}
                    </div>
                </div>
            </div>
        );
    }

    // ── COMPACT MODE ────────────────────────────────────────────────────────
    return (
        <div
            className="absolute top-14 left-3 w-80 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl overflow-visible z-50 flex flex-col animate-in fade-in slide-in-from-top-3 duration-150"
            onClick={e => e.stopPropagation()}
        >
            {/* Profile header */}
            <div className="p-4 flex items-center justify-between border-b border-white/5">
                <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white font-bold shadow-lg">
                        <User size={18} />
                    </div>
                    <div>
                        <div className="text-sm font-bold text-white leading-none">
                            {SystemBridge.getUsername()}
                        </div>
                        <div className="text-[10px] text-blue-300 mt-0.5">Blue Environment</div>
                    </div>
                </div>
                <div className="flex gap-1">
                    <button onClick={handleRefresh} className="p-1.5 hover:bg-white/10 rounded-full text-slate-400 hover:text-white transition-colors" title="Refresh app list">
                        <RefreshCcw size={14} className={loading ? 'animate-spin' : ''} />
                    </button>
                    <button onClick={onToggleFullScreen} className="p-1.5 hover:bg-white/10 rounded-full text-slate-400 hover:text-white transition-colors" title="All apps (⊞+Tab)">
                        <Grid size={16} />
                    </button>
                </div>
            </div>

            {/* Pinned Blue apps */}
            <div className="p-2 grid grid-cols-5 gap-1 border-b border-white/5">
                {Object.values(APPS).filter(a => !a.isExternal).slice(0, 5).map(app => (
                    <button
                        key={app.id}
                        onClick={() => { onOpenApp(app.id as string, false); onClose(); }}
                        className="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-white/10 transition-colors group"
                        title={app.title}
                    >
                        <div className="w-9 h-9 bg-slate-800 rounded-xl flex items-center justify-center border border-white/5 group-hover:bg-blue-600/30 transition-colors">
                            {typeof app.icon !== 'string' && <app.icon size={18} className="text-blue-400" />}
                        </div>
                        <span className="text-[9px] text-slate-400 group-hover:text-white text-center leading-none truncate w-full">{app.title}</span>
                    </button>
                ))}
            </div>

            {/* Search */}
            <div className="px-3 pt-3">
                <div className="relative">
                    <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
                    <input
                        type="text"
                        placeholder="Search apps…"
                        className="w-full bg-slate-800 border border-white/10 rounded-xl py-2 pl-8 pr-3 text-sm text-white focus:outline-none focus:border-blue-500/50 placeholder-slate-500"
                        value={searchTerm}
                        onChange={e => setSearchTerm(e.target.value)}
                    />
                </div>
            </div>

            {/* App list */}
            <div className="flex-1 overflow-y-auto p-2 space-y-0.5 max-h-72 mt-2">
                {loading && (
                    <div className="flex items-center gap-2 px-3 py-2 text-slate-500 text-xs">
                        <Loader2 size={12} className="animate-spin" /> Loading…
                    </div>
                )}

                {!loading && !searchTerm && recentApps.length > 0 && (
                    <>
                        <div className="px-2 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider flex items-center gap-1">
                            <Clock size={10} /> Recent
                        </div>
                        {recentApps.slice(0, 3).map(id => {
                            const app = systemApps.find(a => a.id === id);
                            if (!app) return null;
                            return <CompactAppRow key={id} app={app} onLaunch={handleLaunch} />;
                        })}
                        <div className="h-px bg-white/5 my-1" />
                    </>
                )}

                {!loading && (searchTerm ? allApps : systemApps).slice(0, 12).map(app => (
                    <CompactAppRow key={app.id} app={app} onLaunch={handleLaunch} />
                ))}

                {!loading && allApps.length === 0 && searchTerm && (
                    <div className="px-3 py-4 text-center text-slate-500 text-xs">No results</div>
                )}
            </div>

            {/* Footer */}
            <div className="mt-auto p-3 border-t border-white/5 bg-slate-950/50 rounded-b-2xl flex items-center justify-between relative">
                <button
                    onClick={() => { onToggleFullScreen(); }}
                    className="text-xs text-slate-500 hover:text-white transition-colors flex items-center gap-1"
                >
                    All apps <ChevronRight size={12} />
                </button>
                <button
                    onClick={() => setShowPowerMenu(!showPowerMenu)}
                    className="p-2 rounded-full bg-red-500/10 hover:bg-red-500 text-red-400 hover:text-white transition-all"
                >
                    <Power size={15} />
                </button>
                {showPowerMenu && <PowerMenu />}
            </div>
        </div>
    );
};

// ── Sub-components ─────────────────────────────────────────────────────────

interface AppBtnProps {
    app: CachedApp;
    onLaunch: (app: CachedApp) => void;
    size?: 'large' | 'small';
}

const AppButton: React.FC<AppBtnProps> = ({ app, onLaunch, size = 'large' }) => (
    <button
        onClick={() => onLaunch(app)}
        className="flex flex-col items-center gap-2 p-3 rounded-xl hover:bg-white/10 transition-all group"
    >
        <div className={`${size === 'large' ? 'w-14 h-14' : 'w-10 h-10'} bg-slate-800/60 border border-white/5 rounded-2xl flex items-center justify-center group-hover:bg-blue-600/20 transition-colors overflow-hidden`}>
            <AppIcon icon={app.icon} name={app.name} size={size === 'large' ? 36 : 24} />
        </div>
        <span className="text-xs text-slate-300 group-hover:text-white text-center leading-tight line-clamp-2 w-full">
            {app.name}
        </span>
    </button>
);

const CompactAppRow: React.FC<AppBtnProps> = ({ app, onLaunch }) => (
    <button
        onClick={() => onLaunch(app)}
        className="w-full flex items-center gap-3 px-2 py-1.5 rounded-xl hover:bg-white/5 transition-colors group"
    >
        <div className="w-8 h-8 bg-slate-800 rounded-xl flex items-center justify-center border border-white/5 shrink-0 overflow-hidden">
            <AppIcon icon={app.icon} name={app.name} size={20} />
        </div>
        <div className="flex-1 min-w-0 text-left">
            <div className="text-sm text-slate-200 group-hover:text-white font-medium truncate">{app.name}</div>
            {app.comment && <div className="text-[10px] text-slate-500 truncate">{app.comment}</div>}
        </div>
    </button>
);

export default StartMenu;
