import React, { useState, useEffect, useCallback, useRef } from 'react';
import { AppProps, PackageInfo } from '../../types';
import { useLanguage } from '../../contexts/LanguageContext';
import { SystemBridge } from '../../utils/systemBridge';
import {
    Package, Download, Trash2, RefreshCw, Search,
    Zap, HardDrive, AlertCircle, Loader2, LayoutGrid, List
} from 'lucide-react';

// Typ dla stanu każdego źródła
interface SourceState {
    packages: PackageInfo[];
    loading: boolean;
    error?: string;
}

// Mock danych (używane tylko w trybie deweloperskim, bez Tauri)
const MOCK_DATA: Record<string, PackageInfo[]> = {
    apt: [
        { id: 'firefox', name: 'Firefox', description: 'Fast, private, and free web browser', version: '120.0.1', source: 'apt', installed: true },
        { id: 'vlc', name: 'VLC', description: 'Versatile media player', version: '3.0.20', source: 'apt', installed: true, updateAvailable: true },
        { id: 'gimp', name: 'GIMP', description: 'GNU Image Manipulation Program', version: '2.10.34', source: 'apt', installed: false },
        { id: 'libreoffice', name: 'LibreOffice', description: 'Office suite', version: '7.5.8', source: 'apt', installed: true },
        { id: 'code', name: 'VS Code', description: 'Code editor', version: '1.85.0', source: 'apt', installed: false },
        { id: 'discord', name: 'Discord', description: 'Chat for gamers', version: '0.0.45', source: 'apt', installed: false },
    ],
    flatpak: [
        { id: 'org.flatpak.Firefox', name: 'Firefox (Flatpak)', description: 'Web browser (Flatpak)', version: '120.0', source: 'flatpak', installed: false },
        { id: 'org.gnome.Calculator', name: 'Calculator', description: 'GNOME Calculator', version: '46.0', source: 'flatpak', installed: true },
        { id: 'com.spotify.Client', name: 'Spotify', description: 'Music streaming', version: '1.2.35', source: 'flatpak', installed: false },
    ],
    snap: [
        { id: 'firefox', name: 'Firefox (Snap)', description: 'Web browser (Snap)', version: '120.0', source: 'snap', installed: true },
        { id: 'core20', name: 'Core20', description: 'Snap runtime', version: '20240101', source: 'snap', installed: true },
        { id: 'spotify', name: 'Spotify (Snap)', description: 'Music streaming', version: '1.2.35', source: 'snap', installed: false },
    ],
    appimage: [
        { id: 'obsidian', name: 'Obsidian', description: 'Knowledge base', version: '1.4.16', source: 'appimage', installed: false, icon: '' },
    ],
};

const BlueSoftwareApp: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [activeTab, setActiveTab] = useState<'available' | 'installed' | 'updates'>('available');
    const [searchQuery, setSearchQuery] = useState('');
    const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
    const [sources, setSources] = useState<Record<'apt' | 'flatpak' | 'snap' | 'appimage', SourceState>>({
        apt: { packages: [], loading: false },
        flatpak: { packages: [], loading: false },
        snap: { packages: [], loading: false },
        appimage: { packages: [], loading: false },
    });
    const [installedPackages, setInstalledPackages] = useState<PackageInfo[]>([]);
    const [updatesAvailable, setUpdatesAvailable] = useState<PackageInfo[]>([]);
    const [globalError, setGlobalError] = useState<string | null>(null);
    const [activeAction, setActiveAction] = useState<string | null>(null);
    const isMounted = useRef(true);

    const isTauriEnv = SystemBridge.isTauri();

    // Funkcja do pobierania danych z konkretnego źródła (zabezpieczona)
    const fetchSource = async (source: 'apt' | 'flatpak' | 'snap' | 'appimage'): Promise<PackageInfo[]> => {
        if (!isTauriEnv) {
            // Mock – zwróć dane z pamięci
            return MOCK_DATA[source] || [];
        }

        try {
            let data: PackageInfo[] = [];
            switch (source) {
                case 'apt':
                    data = await SystemBridge.getAptPackages();
                    break;
                case 'flatpak':
                    data = await SystemBridge.getFlatpakPackages();
                    break;
                case 'snap':
                    data = await SystemBridge.getSnapPackages();
                    break;
                case 'appimage':
                    data = await SystemBridge.getAppImagePackages();
                    break;
            }
            return data || []; // zabezpieczenie
        } catch (err: any) {
            console.error(`Error fetching ${source}:`, err);
            throw new Error(`Failed to load ${source} packages: ${err.message}`);
        }
    };

    // Ładowanie wszystkich pakietów
    const loadAllPackages = useCallback(async () => {
        setGlobalError(null);
        // Ustaw stan ładowania dla wszystkich źródeł
        setSources(prev => ({
            apt: { ...prev.apt, loading: true, error: undefined },
            flatpak: { ...prev.flatpak, loading: true, error: undefined },
            snap: { ...prev.snap, loading: true, error: undefined },
            appimage: { ...prev.appimage, loading: true, error: undefined },
        }));

        const results = await Promise.allSettled([
            fetchSource('apt'),
                                                 fetchSource('flatpak'),
                                                 fetchSource('snap'),
                                                 fetchSource('appimage'),
        ]);

        if (!isMounted.current) return;

        const newSources = { ...sources };
        const allPackages: PackageInfo[] = [];

        results.forEach((result, idx) => {
            const source = ['apt', 'flatpak', 'snap', 'appimage'][idx] as keyof typeof sources;
            if (result.status === 'fulfilled') {
                newSources[source] = { packages: result.value, loading: false, error: undefined };
                allPackages.push(...result.value);
            } else {
                newSources[source] = { packages: [], loading: false, error: result.reason?.message || 'Unknown error' };
            }
        });

        setSources(newSources);
        const installed = allPackages.filter(p => p.installed);
        const updates = allPackages.filter(p => p.installed && p.updateAvailable);
        setInstalledPackages(installed);
        setUpdatesAvailable(updates);
    }, [sources, isTauriEnv]);

    useEffect(() => {
        loadAllPackages();
        return () => {
            isMounted.current = false;
        };
    }, [loadAllPackages]);

    // Filtrowanie pakietów
    const filterPackages = (packages: PackageInfo[]): PackageInfo[] => {
        if (!searchQuery.trim()) return packages;
        const q = searchQuery.toLowerCase();
        return packages.filter(p => p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q));
    };

    // Pakiety do wyświetlenia w aktywnej karcie
    let displayPackages: PackageInfo[] = [];
    if (activeTab === 'available') {
        displayPackages = filterPackages(Object.values(sources).flatMap(s => s.packages).filter(p => !p.installed));
    } else if (activeTab === 'installed') {
        displayPackages = filterPackages(installedPackages);
    } else {
        displayPackages = filterPackages(updatesAvailable);
    }

    // Akcje
    const installPackage = async (pkg: PackageInfo) => {
        setActiveAction(pkg.id);
        try {
            let success = false;
            if (isTauriEnv) {
                switch (pkg.source) {
                    case 'apt': success = await SystemBridge.installAptPackage(pkg.id); break;
                    case 'flatpak': success = await SystemBridge.installFlatpakPackage(pkg.id); break;
                    case 'snap': success = await SystemBridge.installSnapPackage(pkg.id); break;
                    case 'appimage': success = await SystemBridge.installAppImage(pkg.id); break;
                }
            } else {
                console.log(`[Mock] Install ${pkg.name}`);
                success = true;
            }
            if (success) await loadAllPackages();
            else setGlobalError(`Failed to install ${pkg.name}`);
        } catch (err: any) {
            setGlobalError(err.message);
        } finally {
            setActiveAction(null);
        }
    };

    const removePackage = async (pkg: PackageInfo) => {
        if (!confirm(`Remove ${pkg.name}?`)) return;
        setActiveAction(pkg.id);
        try {
            let success = false;
            if (isTauriEnv) {
                switch (pkg.source) {
                    case 'apt': success = await SystemBridge.removeAptPackage(pkg.id); break;
                    case 'flatpak': success = await SystemBridge.removeFlatpakPackage(pkg.id); break;
                    case 'snap': success = await SystemBridge.removeSnapPackage(pkg.id); break;
                    case 'appimage': success = await SystemBridge.removeAppImage(pkg.id); break;
                }
            } else {
                console.log(`[Mock] Remove ${pkg.name}`);
                success = true;
            }
            if (success) await loadAllPackages();
            else setGlobalError(`Failed to remove ${pkg.name}`);
        } catch (err: any) {
            setGlobalError(err.message);
        } finally {
            setActiveAction(null);
        }
    };

    const updatePackage = async (pkg: PackageInfo) => {
        setActiveAction(pkg.id);
        try {
            let success = false;
            if (isTauriEnv) {
                switch (pkg.source) {
                    case 'apt': success = await SystemBridge.updateAptPackage(pkg.id); break;
                    case 'flatpak': success = await SystemBridge.updateFlatpakPackage(pkg.id); break;
                    case 'snap': success = await SystemBridge.updateSnapPackage(pkg.id); break;
                    case 'appimage': success = await SystemBridge.updateAppImage(pkg.id); break;
                }
            } else {
                console.log(`[Mock] Update ${pkg.name}`);
                success = true;
            }
            if (success) await loadAllPackages();
            else setGlobalError(`Failed to update ${pkg.name}`);
        } catch (err: any) {
            setGlobalError(err.message);
        } finally {
            setActiveAction(null);
        }
    };

    const sourceIcon = (source: string) => {
        switch (source) {
            case 'apt': return <HardDrive size={14} className="text-blue-400" />;
            case 'flatpak': return <Package size={14} className="text-green-400" />;
            case 'snap': return <Zap size={14} className="text-yellow-400" />;
            case 'appimage': return <Download size={14} className="text-purple-400" />;
            default: return null;
        }
    };

    // Renderowanie pojedynczego pakietu
    const renderPackage = (pkg: PackageInfo) => {
        const isActionActive = activeAction === pkg.id;

        if (viewMode === 'grid') {
            return (
                <div key={pkg.id} className="bg-slate-800/50 rounded-xl p-4 border border-white/5 hover:border-blue-500/50 transition-all group">
                <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                <div className="w-12 h-12 bg-slate-700 rounded-xl flex items-center justify-center">
                {pkg.icon ? (
                    <img src={pkg.icon} className="w-8 h-8" alt="" />
                ) : (
                    <Package size={24} className="text-slate-400" />
                )}
                </div>
                <div>
                <h3 className="font-semibold text-white">{pkg.name}</h3>
                <div className="flex items-center gap-1 text-xs text-slate-400">
                {sourceIcon(pkg.source)}
                <span className="uppercase">{pkg.source}</span>
                <span className="mx-1">•</span>
                <span>{pkg.version}</span>
                </div>
                </div>
                </div>
                {activeTab === 'installed' && pkg.updateAvailable && (
                    <div className="bg-yellow-500/20 text-yellow-400 px-2 py-0.5 rounded-full text-xs">Update</div>
                )}
                </div>
                <p className="text-sm text-slate-300 mt-3 line-clamp-2">{pkg.description}</p>
                <div className="mt-4 flex justify-end">
                {activeTab === 'available' && (
                    <button
                    onClick={() => installPackage(pkg)}
                    disabled={isActionActive}
                    className="px-4 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm flex items-center gap-1 disabled:opacity-50 transition-colors"
                    >
                    {isActionActive ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}
                    Install
                    </button>
                )}
                {activeTab === 'installed' && (
                    <button
                    onClick={() => removePackage(pkg)}
                    disabled={isActionActive}
                    className="px-4 py-1.5 bg-red-600/20 hover:bg-red-500/40 text-red-400 rounded-lg text-sm flex items-center gap-1 disabled:opacity-50 transition-colors"
                    >
                    {isActionActive ? <Loader2 size={14} className="animate-spin" /> : <Trash2 size={14} />}
                    Remove
                    </button>
                )}
                {activeTab === 'updates' && (
                    <button
                    onClick={() => updatePackage(pkg)}
                    disabled={isActionActive}
                    className="px-4 py-1.5 bg-green-600 hover:bg-green-500 rounded-lg text-sm flex items-center gap-1 disabled:opacity-50 transition-colors"
                    >
                    {isActionActive ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
                    Update
                    </button>
                )}
                </div>
                </div>
            );
        } else {
            return (
                <div key={pkg.id} className="flex items-center justify-between p-3 border-b border-white/5 hover:bg-white/5 group">
                <div className="flex items-center gap-3 flex-1 min-w-0">
                <div className="w-8 h-8 bg-slate-700 rounded-lg flex items-center justify-center shrink-0">
                {pkg.icon ? (
                    <img src={pkg.icon} className="w-6 h-6" alt="" />
                ) : (
                    <Package size={16} className="text-slate-400" />
                )}
                </div>
                <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 flex-wrap">
                <span className="font-medium text-white truncate">{pkg.name}</span>
                <div className="flex items-center gap-1 text-xs text-slate-400 shrink-0">
                {sourceIcon(pkg.source)}
                <span className="uppercase">{pkg.source}</span>
                </div>
                {activeTab === 'installed' && pkg.updateAvailable && (
                    <span className="text-xs text-yellow-400 shrink-0">Update available</span>
                )}
                </div>
                <div className="text-xs text-slate-400">{pkg.version}</div>
                <div className="text-xs text-slate-500 mt-0.5 truncate">{pkg.description}</div>
                </div>
                </div>
                <div className="ml-4 shrink-0">
                {activeTab === 'available' && (
                    <button
                    onClick={() => installPackage(pkg)}
                    disabled={isActionActive}
                    className="px-3 py-1 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-50"
                    >
                    {isActionActive ? <Loader2 size={12} className="animate-spin" /> : 'Install'}
                    </button>
                )}
                {activeTab === 'installed' && (
                    <button
                    onClick={() => removePackage(pkg)}
                    disabled={isActionActive}
                    className="px-3 py-1 bg-red-600/20 hover:bg-red-500/40 text-red-400 rounded-lg text-sm disabled:opacity-50"
                    >
                    {isActionActive ? <Loader2 size={12} className="animate-spin" /> : 'Remove'}
                    </button>
                )}
                {activeTab === 'updates' && (
                    <button
                    onClick={() => updatePackage(pkg)}
                    disabled={isActionActive}
                    className="px-3 py-1 bg-green-600 hover:bg-green-500 rounded-lg text-sm disabled:opacity-50"
                    >
                    {isActionActive ? <Loader2 size={12} className="animate-spin" /> : 'Update'}
                    </button>
                )}
                </div>
                </div>
            );
        }
    };

    // Sprawdzenie, czy któreś źródło nadal się ładuje
    const anyLoading = Object.values(sources).some(s => s.loading);

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
        {/* Nagłówek */}
        <div className="p-6 border-b border-white/5">
        <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
        <Package size={28} className="text-blue-400" />
        <h1 className="text-2xl font-bold">Blue Software</h1>
        </div>
        <button
        onClick={loadAllPackages}
        disabled={anyLoading}
        className="p-2 hover:bg-white/10 rounded-full transition-colors disabled:opacity-50"
        title="Refresh"
        >
        <RefreshCw size={18} className={anyLoading ? 'animate-spin' : ''} />
        </button>
        </div>

        {/* Karty */}
        <div className="flex gap-2 border-b border-white/10 mb-4">
        {(['available', 'installed', 'updates'] as const).map(tab => (
            <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px ${
                activeTab === tab
                ? 'border-blue-500 text-blue-400'
                : 'border-transparent text-slate-400 hover:text-white'
            }`}
            >
            {tab === 'available' && `Available (${displayPackages.length})`}
            {tab === 'installed' && `Installed (${installedPackages.length})`}
            {tab === 'updates' && `Updates (${updatesAvailable.length})`}
            </button>
        ))}
        </div>

        {/* Pasek wyszukiwania i przełącznik widoku */}
        <div className="flex items-center gap-4">
        <div className="relative flex-1">
        <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
        <input
        type="text"
        placeholder="Search applications..."
        value={searchQuery}
        onChange={(e) => setSearchQuery(e.target.value)}
        className="w-full bg-slate-800 border border-white/10 rounded-lg py-2 pl-9 pr-4 text-white focus:outline-none focus:border-blue-500"
        />
        </div>
        <div className="flex gap-1 bg-slate-800 rounded-lg p-1">
        <button
        onClick={() => setViewMode('grid')}
        className={`p-1.5 rounded ${viewMode === 'grid' ? 'bg-blue-600' : 'hover:bg-white/10'}`}
        title="Grid view"
        >
        <LayoutGrid size={16} />
        </button>
        <button
        onClick={() => setViewMode('list')}
        className={`p-1.5 rounded ${viewMode === 'list' ? 'bg-blue-600' : 'hover:bg-white/10'}`}
        title="List view"
        >
        <List size={16} />
        </button>
        </div>
        </div>
        </div>

        {/* Główna zawartość */}
        <div className="flex-1 overflow-y-auto p-6">
        {globalError && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 mb-4 flex items-center gap-2 text-red-400">
            <AlertCircle size={16} />
            <span>{globalError}</span>
            <button onClick={() => setGlobalError(null)} className="ml-auto text-xs">Dismiss</button>
            </div>
        )}

        {anyLoading && displayPackages.length === 0 ? (
            <div className="flex items-center justify-center h-64">
            <Loader2 size={32} className="animate-spin text-blue-400" />
            </div>
        ) : displayPackages.length === 0 ? (
            <div className="text-center py-12 text-slate-500">
            <Package size={48} className="mx-auto mb-3 opacity-50" />
            <p>No applications found</p>
            </div>
        ) : (
            <div className={viewMode === 'grid' ? 'grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4' : 'space-y-1'}>
            {displayPackages.map(renderPackage)}
            </div>
        )}
        </div>
        </div>
    );
};

export default BlueSoftwareApp;
