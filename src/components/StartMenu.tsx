import React, { useState, useEffect, useMemo } from 'react';
import { APPS } from '../constants';
import { AppId } from '../types';
import { SystemBridge } from '../utils/systemBridge';
import {
    Search, Power, Grid, User, Box, Terminal as TerminalIcon,
    LogOut, Moon, RefreshCcw, HardDrive, Clock, ChevronRight,
    Loader2, Wifi, Bluetooth, Monitor, Settings, HelpCircle,
    Gamepad2, Video, Music, Image, FileText, Code, Globe,
    BookOpen, Calculator, Activity, FolderOpen, Bot, Info
} from 'lucide-react';

// Interfejs dla aplikacji z cache (z systemBridge) – ikona to string (ścieżka)
interface SystemApp {
    id: string;
    name: string;
    comment: string;
    icon: string; // ścieżka pliku
    exec: string;
    categories: string[];
    desktop_file: string;
    is_external: boolean;
}

// Interfejs dla aplikacji wewnętrznych (z APPS) – ikona to komponent React
interface InternalApp {
    id: string;
    name: string;
    icon: React.ComponentType<any>;
    categories: string[];
    isInternal: true;
}

type AnyApp = SystemApp | InternalApp;

// Kategorie i ich etykiety (kolejność wyświetlania)
const CATEGORY_ORDER = [
    { key: 'Recent',      label: 'Ostatnie',        icon: Clock,      keys: ['Recent'] },
{ key: 'Internet',    label: 'Internet',        icon: Globe,      keys: ['Network', 'WebBrowser'] },
{ key: 'Multimedia',  label: 'Multimedia',      icon: Video,      keys: ['AudioVideo', 'Audio', 'Video'] },
{ key: 'Graphics',    label: 'Grafika',         icon: Image,      keys: ['Graphics'] },
{ key: 'Office',      label: 'Biuro',           icon: FileText,   keys: ['Office'] },
{ key: 'Development', label: 'Programowanie',   icon: Code,       keys: ['Development', 'IDE'] },
{ key: 'Games',       label: 'Gry',             icon: Gamepad2,   keys: ['Game'] },
{ key: 'System',      label: 'System',          icon: Settings,   keys: ['System', 'Settings', 'Utility'] },
{ key: 'Other',       label: 'Pozostałe',       icon: Box,        keys: [] },
];

// Mapowanie wewnętrznych aplikacji na kategorie
const INTERNAL_APP_CATEGORIES: Record<string, string[]> = {
    [AppId.TERMINAL]: ['System'],
    [AppId.BLUE_WEB]: ['Internet'],
    [AppId.EXPLORER]: ['System'],
    [AppId.CALCULATOR]: ['Utility'],
    [AppId.SYSTEM_MONITOR]: ['System'],
    [AppId.AI_ASSISTANT]: ['Utility'],
    [AppId.SETTINGS]: ['System'],
    [AppId.ABOUT]: ['System'],
    [AppId.NOTEPAD]: ['Office'],
    [AppId.BLUE_EDIT]: ['Development'],
    [AppId.BLUE_IMAGES]: ['Graphics'],
    [AppId.BLUE_VIDEOS]: ['Multimedia'],
    [AppId.BLUE_MUSIC]: ['Multimedia'],
    [AppId.BLUE_SCREEN]: ['System'],
};

// Funkcja określająca kategorię dla aplikacji
function getCategory(app: AnyApp): string {
    if ('isInternal' in app) {
        // Aplikacja wewnętrzna – mapujemy na podstawie stałej
        const cats = INTERNAL_APP_CATEGORIES[app.id] || ['Other'];
        for (const cat of CATEGORY_ORDER) {
            if (cat.keys.some(k => cats.includes(k))) return cat.key;
        }
        return 'Other';
    } else {
        // Aplikacja z systemu – sprawdzamy jej kategorie
        for (const cat of CATEGORY_ORDER) {
            if (cat.keys.some(k => app.categories.includes(k))) return cat.key;
        }
        return 'Other';
    }
}

// Komponent ikony (z fallbackiem)
const AppIcon: React.FC<{ icon: any; name: string; size?: number }> = ({ icon, name, size = 32 }) => {
    const [failed, setFailed] = useState(false);

    // Jeśli ikona jest stringiem (ścieżka)
    if (typeof icon === 'string' && !failed) {
        const isUrl = icon.startsWith('http') || icon.startsWith('file://');
        if (isUrl) {
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
    }

    // Jeśli ikona jest komponentem React (np. z lucide-react)
    if (typeof icon !== 'string' && !failed) {
        const IconComponent = icon;
        return <IconComponent size={size} />;
    }

    // Fallback: litera
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

// Główny komponent
interface StartMenuProps {
    isOpen: boolean;
    isFullScreen: boolean;
    onOpenApp: (appId: string, isExternal?: boolean, exec?: string) => void;
    onClose: () => void;
    onToggleFullScreen: () => void;
}

const StartMenu: React.FC<StartMenuProps> = ({
    isOpen, isFullScreen, onOpenApp, onClose, onToggleFullScreen
}) => {
    const [searchTerm, setSearchTerm] = useState('');
    const [systemApps, setSystemApps] = useState<SystemApp[]>([]);
    const [recentApps, setRecentApps] = useState<string[]>([]);
    const [loading, setLoading] = useState(false);
    const [activeCategory, setActiveCategory] = useState('Recent');
    const [showPowerMenu, setShowPowerMenu] = useState(false);

    useEffect(() => {
        if (!isOpen) return;
        setLoading(true);
        Promise.all([
            SystemBridge.getSystemApps(false),
                    SystemBridge.getRecentApps(),
        ]).then(([apps, recent]) => {
            // Zakładamy, że getSystemApps zwraca tablicę obiektów zgodnych z SystemApp
            setSystemApps(apps as SystemApp[]);
            setRecentApps(recent);
            setLoading(false);
        });
    }, [isOpen]);

    // Wewnętrzne aplikacje (z APPS) – konwertujemy na format InternalApp
    const internalApps = useMemo((): InternalApp[] => {
        return Object.values(APPS)
        .filter(app => !app.isExternal && app.component) // tylko te z komponentem
        .map(app => ({
            id: app.id as string,
            name: app.title,
            icon: app.icon as React.ComponentType<any>,
            categories: INTERNAL_APP_CATEGORIES[app.id as string] || ['Other'],
            isInternal: true as const,
        }));
    }, []);

    // Wszystkie aplikacje (wewnętrzne + systemowe) + filtrowanie po search
    const allApps = useMemo((): AnyApp[] => {
        const term = searchTerm.toLowerCase().trim();
        const combined: AnyApp[] = [...internalApps, ...systemApps];

        if (!term) return combined;

        return combined.filter(app =>
        app.name.toLowerCase().includes(term) ||
        (!('isInternal' in app) && app.comment.toLowerCase().includes(term))
        );
    }, [searchTerm, internalApps, systemApps]);

    // Grupowanie według kategorii
    const groupedApps = useMemo(() => {
        const groups: Record<string, AnyApp[]> = {};

        for (const app of allApps) {
            const cat = getCategory(app);
            if (!groups[cat]) groups[cat] = [];
            groups[cat].push(app);
        }

        // Sortuj aplikacje w każdej grupie alfabetycznie
        for (const cat in groups) {
            groups[cat].sort((a, b) => a.name.localeCompare(b.name));
        }

        // Dodaj kategorię "Ostatnie" (tylko gdy nie ma wyszukiwania)
        if (!searchTerm && recentApps.length > 0) {
            const recentList = recentApps
            .map(id => allApps.find(a => a.id === id))
            .filter(Boolean) as AnyApp[];
            if (recentList.length) {
                groups['Recent'] = recentList.slice(0, 8);
            }
        }

        return groups;
    }, [allApps, recentApps, searchTerm]);

    // Widoczne kategorie (te, które mają aplikacje)
    const visibleCategories = CATEGORY_ORDER.filter(cat => groupedApps[cat.key]?.length > 0);

    const handleLaunch = (app: AnyApp) => {
        if ('isInternal' in app) {
            onOpenApp(app.id, false);
        } else {
            onOpenApp(app.id, app.is_external, app.exec);
        }
        SystemBridge.recordAppLaunch(app.id);
        onClose();
    };

    const handleRefresh = async () => {
        setLoading(true);
        const apps = await SystemBridge.getSystemApps(true);
        setSystemApps(apps as SystemApp[]);
        setLoading(false);
    };

    if (!isOpen) return null;

    // Menu zasilania
    const PowerMenu = () => (
        <div className="absolute bottom-14 left-4 bg-slate-800 border border-white/10 rounded-xl shadow-2xl p-2 flex flex-col gap-1 w-48 z-50 animate-in fade-in slide-in-from-bottom-2">
        {[
            { action: 'shutdown', icon: Power, label: 'Wyłącz', cls: 'hover:bg-red-500/20 hover:text-red-400' },
            { action: 'reboot', icon: RefreshCcw, label: 'Uruchom ponownie', cls: 'hover:bg-white/10' },
            { action: 'suspend', icon: Moon, label: 'Uśpij', cls: 'hover:bg-white/10' },
            { action: 'hibernate', icon: HardDrive, label: 'Hibernuj', cls: 'hover:bg-white/10' },
        ].map(({ action, icon: Icon, label, cls }) => (
            <button key={action} onClick={() => SystemBridge.powerAction(action)}
            className={`flex items-center gap-3 p-2 ${cls} rounded-lg transition-colors text-left text-sm text-slate-200`}>
            <Icon size={16} /> {label}
            </button>
        ))}
        <div className="h-px bg-white/10 my-1" />
        <button onClick={() => SystemBridge.powerAction('logout')}
        className="flex items-center gap-3 p-2 hover:bg-white/10 rounded-lg transition-colors text-left text-sm text-slate-200">
        <LogOut size={16} /> Wyloguj
        </button>
        </div>
    );

    // Widok pełnoekranowy
    if (isFullScreen) {
        return (
            <div
            className="absolute inset-0 bg-slate-900/97 backdrop-blur-xl z-40 flex animate-in fade-in duration-150"
            onClick={onClose}
            >
            {/* Lewy panel z kategoriami */}
            <div className="w-56 border-r border-white/5 flex flex-col pt-16 px-3 gap-1 shrink-0" onClick={e => e.stopPropagation()}>
            {visibleCategories.map(cat => {
                const Icon = cat.icon || Box;
                return (
                    <button
                    key={cat.key}
                    onClick={() => setActiveCategory(cat.key)}
                    className={`flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium text-left transition-all ${
                        activeCategory === cat.key
                        ? 'bg-blue-600 text-white shadow-lg'
                        : 'text-slate-400 hover:bg-white/5 hover:text-white'
                    }`}
                    >
                    <Icon size={18} />
                    <span>{cat.label}</span>
                    <span className="ml-auto text-xs opacity-50">
                    {groupedApps[cat.key]?.length || 0}
                    </span>
                    </button>
                );
            })}
            </div>

            {/* Główna część – wyszukiwanie i siatka aplikacji */}
            <div className="flex-1 flex flex-col" onClick={e => e.stopPropagation()}>
            {/* Pasek wyszukiwania */}
            <div className="px-8 pt-12 pb-6">
            <div className="relative max-w-xl">
            <Search className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" size={20} />
            <input
            type="text"
            autoFocus
            placeholder="Szukaj aplikacji..."
            className="w-full bg-slate-800 border border-white/10 rounded-2xl py-3 pl-12 pr-4 text-white text-lg focus:outline-none focus:border-blue-500/60"
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            />
            </div>
            </div>

            {/* Siatka aplikacji */}
            <div className="flex-1 overflow-y-auto px-8 pb-8">
            {loading && (
                <div className="flex items-center gap-2 text-slate-500 text-sm">
                <Loader2 size={16} className="animate-spin" /> Ładowanie aplikacji…
                </div>
            )}

            {!loading && searchTerm && (
                <div className="grid grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
                {allApps.slice(0, 60).map(app => (
                    <AppButton key={app.id} app={app} onLaunch={handleLaunch} size="large" />
                ))}
                {allApps.length === 0 && (
                    <p className="col-span-full text-center text-slate-500 text-sm py-12">
                    Brak wyników dla "{searchTerm}"
                    </p>
                )}
                </div>
            )}

            {!loading && !searchTerm && (
                <div>
                <h3 className="text-xl font-semibold text-white mb-4 flex items-center gap-2">
                {/* Pobierz ikonę kategorii */}
                {(() => {
                    const category = CATEGORY_ORDER.find(c => c.key === activeCategory);
                    if (category) {
                        const Icon = category.icon;
                        return <Icon size={20} />;
                    }
                    return null;
                })()}
                {CATEGORY_ORDER.find(c => c.key === activeCategory)?.label || activeCategory}
                </h3>
                <div className="grid grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
                {(groupedApps[activeCategory] || []).map(app => (
                    <AppButton key={app.id} app={app} onLaunch={handleLaunch} size="large" />
                ))}
                </div>
                </div>
            )}
            </div>
            </div>
            </div>
        );
    }

    // Widok kompaktowy (standardowe menu start)
    return (
        <div
        className="absolute top-14 left-3 w-80 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl overflow-visible z-50 flex flex-col animate-in fade-in slide-in-from-top-3 duration-150"
        onClick={e => e.stopPropagation()}
        >
        {/* Nagłówek z profilem */}
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
        <button onClick={handleRefresh} className="p-1.5 hover:bg-white/10 rounded-full text-slate-400 hover:text-white transition-colors" title="Odśwież listę">
        <RefreshCcw size={14} className={loading ? 'animate-spin' : ''} />
        </button>
        <button onClick={onToggleFullScreen} className="p-1.5 hover:bg-white/10 rounded-full text-slate-400 hover:text-white transition-colors" title="Wszystkie aplikacje (⊞+Tab)">
        <Grid size={16} />
        </button>
        </div>
        </div>

        {/* Przypięte aplikacje (szybki dostęp) */}
        <div className="p-2 grid grid-cols-5 gap-1 border-b border-white/5">
        {internalApps.slice(0, 5).map(app => (
            <button
            key={app.id}
            onClick={() => { handleLaunch(app); }}
            className="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-white/10 transition-colors group"
            title={app.name}
            >
            <div className="w-9 h-9 bg-slate-800 rounded-xl flex items-center justify-center border border-white/5 group-hover:bg-blue-600/30 transition-colors">
            <AppIcon icon={app.icon} name={app.name} size={18} />
            </div>
            <span className="text-[9px] text-slate-400 group-hover:text-white text-center leading-none truncate w-full">{app.name}</span>
            </button>
        ))}
        </div>

        {/* Wyszukiwanie */}
        <div className="px-3 pt-3">
        <div className="relative">
        <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
        <input
        type="text"
        placeholder="Szukaj aplikacji..."
        className="w-full bg-slate-800 border border-white/10 rounded-xl py-2 pl-8 pr-3 text-sm text-white focus:outline-none focus:border-blue-500/50 placeholder-slate-500"
        value={searchTerm}
        onChange={e => setSearchTerm(e.target.value)}
        />
        </div>
        </div>

        {/* Lista aplikacji */}
        <div className="flex-1 overflow-y-auto p-2 space-y-0.5 max-h-72 mt-2">
        {loading && (
            <div className="flex items-center gap-2 px-3 py-2 text-slate-500 text-xs">
            <Loader2 size={12} className="animate-spin" /> Ładowanie…
            </div>
        )}

        {!loading && !searchTerm && recentApps.length > 0 && (
            <>
            <div className="px-2 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider flex items-center gap-1">
            <Clock size={10} /> Ostatnie
            </div>
            {recentApps.slice(0, 3).map(id => {
                const app = allApps.find(a => a.id === id);
                if (!app) return null;
                return <CompactAppRow key={id} app={app} onLaunch={handleLaunch} />;
            })}
            <div className="h-px bg-white/5 my-1" />
            </>
        )}

        {!loading && (searchTerm ? allApps : allApps.filter(a => !recentApps.includes(a.id))).slice(0, 12).map(app => (
            <CompactAppRow key={app.id} app={app} onLaunch={handleLaunch} />
        ))}

        {!loading && allApps.length === 0 && searchTerm && (
            <div className="px-3 py-4 text-center text-slate-500 text-xs">Brak wyników</div>
        )}
        </div>

        {/* Stopka */}
        <div className="mt-auto p-3 border-t border-white/5 bg-slate-950/50 rounded-b-2xl flex items-center justify-between relative">
        <button
        onClick={onToggleFullScreen}
        className="text-xs text-slate-500 hover:text-white transition-colors flex items-center gap-1"
        >
        Wszystkie aplikacje <ChevronRight size={12} />
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

// Subkomponent: przycisk aplikacji w siatce (duży)
const AppButton: React.FC<{ app: AnyApp; onLaunch: (app: AnyApp) => void; size?: 'large' | 'small' }> = ({ app, onLaunch, size = 'large' }) => (
    <button
    onClick={() => onLaunch(app)}
    className="flex flex-col items-center gap-2 p-3 rounded-xl hover:bg-white/10 transition-all group"
    >
    <div className={`${size === 'large' ? 'w-14 h-14' : 'w-10 h-10'} bg-slate-800/60 border border-white/5 rounded-2xl flex items-center justify-center group-hover:bg-blue-600/20 transition-colors overflow-hidden`}>
    <AppIcon
    icon={'isInternal' in app ? app.icon : app.icon}
    name={app.name}
    size={size === 'large' ? 36 : 24}
    />
    </div>
    <span className="text-xs text-slate-300 group-hover:text-white text-center leading-tight line-clamp-2 w-full">
    {app.name}
    </span>
    </button>
);

// Subkomponent: wiersz w kompaktowej liście
const CompactAppRow: React.FC<{ app: AnyApp; onLaunch: (app: AnyApp) => void }> = ({ app, onLaunch }) => (
    <button
    onClick={() => onLaunch(app)}
    className="w-full flex items-center gap-3 px-2 py-1.5 rounded-xl hover:bg-white/5 transition-colors group"
    >
    <div className="w-8 h-8 bg-slate-800 rounded-xl flex items-center justify-center border border-white/5 shrink-0 overflow-hidden">
    <AppIcon
    icon={'isInternal' in app ? app.icon : app.icon}
    name={app.name}
    size={20}
    />
    </div>
    <div className="flex-1 min-w-0 text-left">
    <div className="text-sm text-slate-200 group-hover:text-white font-medium truncate">{app.name}</div>
    {!('isInternal' in app) && app.comment && <div className="text-[10px] text-slate-500 truncate">{app.comment}</div>}
    </div>
    </button>
);

export default StartMenu;
