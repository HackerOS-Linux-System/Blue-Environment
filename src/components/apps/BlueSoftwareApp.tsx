import React, { useState, useEffect, useCallback, useRef } from 'react';
import { AppProps, PackageInfo } from '../../types';
import { useLanguage } from '../../contexts/LanguageContext';
import { SystemBridge } from '../../utils/systemBridge';
import { Package, Download, Trash2, RefreshCw, Search, Zap, HardDrive, Loader2, LayoutGrid, List, AlertCircle } from 'lucide-react';

const MOCK_PACKAGES: PackageInfo[] = [
    { id: 'firefox', name: 'Firefox', description: 'Fast, private web browser', version: '120.0', source: 'apt', installed: false },
    { id: 'vlc', name: 'VLC', description: 'Versatile media player', version: '3.0.20', source: 'apt', installed: true, updateAvailable: true },
    { id: 'gimp', name: 'GIMP', description: 'GNU Image Manipulation Program', version: '2.10.34', source: 'apt', installed: false },
    { id: 'libreoffice', name: 'LibreOffice', description: 'Office suite', version: '7.5.8', source: 'apt', installed: true },
    { id: 'code', name: 'VS Code', description: 'Code editor by Microsoft', version: '1.85.0', source: 'flatpak', installed: false },
    { id: 'discord', name: 'Discord', description: 'Chat for communities', version: '0.0.45', source: 'flatpak', installed: false },
    { id: 'spotify', name: 'Spotify', description: 'Music streaming service', version: '1.2.35', source: 'snap', installed: false },
    { id: 'obsidian', name: 'Obsidian', description: 'Knowledge base tool', version: '1.4.16', source: 'appimage', installed: false },
];

const BlueSoftwareApp: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [activeTab, setActiveTab] = useState<'available' | 'installed' | 'updates'>('available');
    const [searchQuery, setSearchQuery] = useState('');
    const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
    const [packages, setPackages] = useState<PackageInfo[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [activeAction, setActiveAction] = useState<string | null>(null);
    const isMounted = useRef(true);

    const loadPackages = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const [apt, flatpak, snap, appimage] = await Promise.allSettled([
                SystemBridge.getAptPackages(),
                SystemBridge.getFlatpakPackages(),
                SystemBridge.getSnapPackages(),
                SystemBridge.getAppImagePackages(),
            ]);
            const all: PackageInfo[] = [];
            for (const r of [apt, flatpak, snap, appimage]) {
                if (r.status === 'fulfilled') all.push(...r.value);
            }
            if (isMounted.current) setPackages(all.length > 0 ? all : MOCK_PACKAGES);
        } catch (e: any) {
            if (isMounted.current) { setPackages(MOCK_PACKAGES); }
        } finally {
            if (isMounted.current) setLoading(false);
        }
    }, []);

    useEffect(() => {
        loadPackages();
        return () => { isMounted.current = false; };
    }, [loadPackages]);

    const filtered = packages.filter(p => {
        const q = searchQuery.toLowerCase();
        const matchSearch = !q || p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q);
        if (activeTab === 'available') return matchSearch && !p.installed;
        if (activeTab === 'installed') return matchSearch && p.installed;
        if (activeTab === 'updates') return matchSearch && p.installed && p.updateAvailable;
        return matchSearch;
    });

    const performAction = async (pkg: PackageInfo, action: 'install' | 'remove' | 'update') => {
        setActiveAction(pkg.id);
        try {
            let ok = false;
            if (action === 'install') {
                if (pkg.source === 'apt') ok = await SystemBridge.installAptPackage(pkg.id);
                else if (pkg.source === 'flatpak') ok = await SystemBridge.installFlatpakPackage(pkg.id);
                else if (pkg.source === 'snap') ok = await SystemBridge.installSnapPackage(pkg.id);
                else ok = await SystemBridge.installAppImage(pkg.id);
            } else if (action === 'remove') {
                if (pkg.source === 'apt') ok = await SystemBridge.removeAptPackage(pkg.id);
                else if (pkg.source === 'flatpak') ok = await SystemBridge.removeFlatpakPackage(pkg.id);
                else if (pkg.source === 'snap') ok = await SystemBridge.removeSnapPackage(pkg.id);
                else ok = await SystemBridge.removeAppImage(pkg.id);
            } else {
                if (pkg.source === 'apt') ok = await SystemBridge.updateAptPackage(pkg.id);
                else if (pkg.source === 'flatpak') ok = await SystemBridge.updateFlatpakPackage(pkg.id);
                else ok = await SystemBridge.updateSnapPackage(pkg.id);
            }
            if (ok || !SystemBridge.isTauri()) await loadPackages();
        } catch (e: any) { setError(e.message); }
        finally { setActiveAction(null); }
    };

    const sourceIcon = (source: string) => {
        if (source === 'apt') return <HardDrive size={12} className="text-blue-400" />;
        if (source === 'flatpak') return <Package size={12} className="text-green-400" />;
        if (source === 'snap') return <Zap size={12} className="text-yellow-400" />;
        return <Download size={12} className="text-purple-400" />;
    };

    const installed = packages.filter(p => p.installed).length;
    const updates = packages.filter(p => p.installed && p.updateAvailable).length;

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
            <div className="p-5 border-b border-white/5">
                <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center gap-2"><Package size={28} className="text-blue-400" /><h1 className="text-2xl font-bold">Blue Software</h1></div>
                    <button onClick={loadPackages} disabled={loading} className="p-2 hover:bg-white/10 rounded-full transition-colors disabled:opacity-50">
                        <RefreshCw size={18} className={loading ? 'animate-spin' : ''} />
                    </button>
                </div>
                <div className="flex gap-2 border-b border-white/10 mb-4">
                    {([['available', `Available (${packages.filter(p => !p.installed).length})`], ['installed', `Installed (${installed})`], ['updates', `Updates (${updates})`]] as const).map(([tab, label]) => (
                        <button key={tab} onClick={() => setActiveTab(tab)}
                            className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px ${activeTab === tab ? 'border-blue-500 text-blue-400' : 'border-transparent text-slate-400 hover:text-white'}`}>
                            {label}
                        </button>
                    ))}
                </div>
                <div className="flex items-center gap-4">
                    <div className="relative flex-1">
                        <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
                        <input type="text" placeholder="Search packages..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)}
                            className="w-full bg-slate-800 border border-white/10 rounded-lg py-2 pl-9 pr-4 text-white focus:outline-none focus:border-blue-500" />
                    </div>
                    <div className="flex gap-1 bg-slate-800 rounded-lg p-1">
                        <button onClick={() => setViewMode('grid')} className={`p-1.5 rounded ${viewMode === 'grid' ? 'bg-blue-600' : 'hover:bg-white/10'}`}><LayoutGrid size={16} /></button>
                        <button onClick={() => setViewMode('list')} className={`p-1.5 rounded ${viewMode === 'list' ? 'bg-blue-600' : 'hover:bg-white/10'}`}><List size={16} /></button>
                    </div>
                </div>
            </div>
            <div className="flex-1 overflow-y-auto p-5">
                {error && (
                    <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 mb-4 flex items-center gap-2 text-red-400">
                        <AlertCircle size={16} /><span>{error}</span><button onClick={() => setError(null)} className="ml-auto text-xs">Dismiss</button>
                    </div>
                )}
                {loading && filtered.length === 0 ? (
                    <div className="flex items-center justify-center h-64"><Loader2 size={32} className="animate-spin text-blue-400" /></div>
                ) : filtered.length === 0 ? (
                    <div className="text-center py-12 text-slate-500"><Package size={48} className="mx-auto mb-3 opacity-50" /><p>No packages found</p></div>
                ) : viewMode === 'grid' ? (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                        {filtered.map(pkg => {
                            const isActive = activeAction === pkg.id;
                            return (
                                <div key={pkg.id} className="bg-slate-800/50 rounded-xl p-4 border border-white/5 hover:border-blue-500/30 transition-all">
                                    <div className="flex items-start gap-3 mb-3">
                                        <div className="w-12 h-12 bg-slate-700 rounded-xl flex items-center justify-center shrink-0"><Package size={24} className="text-slate-400" /></div>
                                        <div>
                                            <h3 className="font-semibold text-white">{pkg.name}</h3>
                                            <div className="flex items-center gap-1 text-xs text-slate-400">{sourceIcon(pkg.source)}<span className="uppercase">{pkg.source}</span><span className="mx-1">•</span><span>{pkg.version}</span></div>
                                        </div>
                                        {pkg.updateAvailable && <div className="ml-auto bg-yellow-500/20 text-yellow-400 px-2 py-0.5 rounded-full text-xs">Update</div>}
                                    </div>
                                    <p className="text-sm text-slate-300 line-clamp-2 mb-3">{pkg.description}</p>
                                    <div className="flex justify-end">
                                        {activeTab === 'available' && <button onClick={() => performAction(pkg, 'install')} disabled={isActive} className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm flex items-center gap-1 disabled:opacity-50 transition-colors">{isActive ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />} Install</button>}
                                        {activeTab === 'installed' && <button onClick={() => performAction(pkg, 'remove')} disabled={isActive} className="px-3 py-1.5 bg-red-600/20 hover:bg-red-500/40 text-red-400 rounded-lg text-sm flex items-center gap-1 disabled:opacity-50 transition-colors">{isActive ? <Loader2 size={14} className="animate-spin" /> : <Trash2 size={14} />} Remove</button>}
                                        {activeTab === 'updates' && <button onClick={() => performAction(pkg, 'update')} disabled={isActive} className="px-3 py-1.5 bg-green-600 hover:bg-green-500 rounded-lg text-sm flex items-center gap-1 disabled:opacity-50 transition-colors">{isActive ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />} Update</button>}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                ) : (
                    <div className="space-y-1">
                        {filtered.map(pkg => {
                            const isActive = activeAction === pkg.id;
                            return (
                                <div key={pkg.id} className="flex items-center p-3 border-b border-white/5 hover:bg-white/5">
                                    <div className="w-8 h-8 bg-slate-700 rounded-lg flex items-center justify-center mr-3 shrink-0"><Package size={16} className="text-slate-400" /></div>
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2 flex-wrap"><span className="font-medium text-white">{pkg.name}</span><div className="flex items-center gap-1 text-xs text-slate-400">{sourceIcon(pkg.source)}<span>{pkg.source}</span></div>{pkg.updateAvailable && <span className="text-xs text-yellow-400">Update available</span>}</div>
                                        <div className="text-xs text-slate-500 truncate">{pkg.description}</div>
                                    </div>
                                    <div className="ml-4 shrink-0">
                                        {activeTab === 'available' && <button onClick={() => performAction(pkg, 'install')} disabled={isActive} className="px-3 py-1 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-50 transition-colors">{isActive ? <Loader2 size={12} className="animate-spin" /> : 'Install'}</button>}
                                        {activeTab === 'installed' && <button onClick={() => performAction(pkg, 'remove')} disabled={isActive} className="px-3 py-1 bg-red-600/20 hover:bg-red-500/40 text-red-400 rounded-lg text-sm disabled:opacity-50 transition-colors">{isActive ? <Loader2 size={12} className="animate-spin" /> : 'Remove'}</button>}
                                        {activeTab === 'updates' && <button onClick={() => performAction(pkg, 'update')} disabled={isActive} className="px-3 py-1 bg-green-600 hover:bg-green-500 rounded-lg text-sm disabled:opacity-50 transition-colors">{isActive ? <Loader2 size={12} className="animate-spin" /> : 'Update'}</button>}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>
        </div>
    );
};
export default BlueSoftwareApp;
