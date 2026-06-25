import React, { useEffect, useState } from 'react';
import { Package, RefreshCw, Search, LayoutGrid, List, AlertCircle, Loader2, X } from 'lucide-react';
import { AppProps } from '../../../types';
import { SoftwareTab, ViewMode } from './src/types';
import { usePackages } from './src/usePackages';
import PackageCard from './src/PackageCard';
import PackageRow from './src/PackageRow';
import InstallLogTerminal from './src/InstallLogTerminal';

const BlueSoftwareApp: React.FC<AppProps> = () => {
    const { packages, loading, error, setError, activeAction, installLog, setInstallLog, loadPackages, performAction } = usePackages();
    const [activeTab,    setActiveTab]    = useState<SoftwareTab>('available');
    const [searchQuery,  setSearchQuery]  = useState('');
    const [viewMode,     setViewMode]     = useState<ViewMode>('grid');

    useEffect(() => { loadPackages(); }, [loadPackages]);

    const filtered = packages.filter(p => {
        const q = searchQuery.toLowerCase();
        const matchSearch = !q || p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q);
        if (activeTab === 'available') return matchSearch && !p.installed;
        if (activeTab === 'installed') return matchSearch && p.installed;
        if (activeTab === 'updates')   return matchSearch && p.installed && p.update_available;
        return matchSearch;
    });

    const installed = packages.filter(p => p.installed).length;
    const updates   = packages.filter(p => p.installed && p.update_available).length;

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
            {/* Header */}
            <div className="p-4 border-b border-white/5 shrink-0">
                <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-2">
                        <Package size={26} className="text-blue-400" />
                        <h1 className="text-xl font-bold">Blue Software</h1>
                    </div>
                    <button onClick={loadPackages} disabled={loading}
                        className="p-2 hover:bg-white/10 rounded-full transition-colors disabled:opacity-50">
                        <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
                    </button>
                </div>

                {/* Tab bar */}
                <div className="flex gap-0 border-b border-white/10 mb-3">
                    {([
                        ['available', `Available (${packages.filter(p => !p.installed).length})`],
                        ['installed', `Installed (${installed})`],
                        ['updates',   `Updates (${updates})`],
                    ] as const).map(([tab, label]) => (
                        <button key={tab} onClick={() => setActiveTab(tab)}
                            className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px
                                ${activeTab === tab ? 'border-blue-500 text-blue-400' : 'border-transparent text-slate-400 hover:text-white'}`}>
                            {label}
                            {tab === 'updates' && updates > 0 && (
                                <span className="ml-1.5 px-1.5 py-0.5 rounded-full text-[10px] bg-orange-500/20 text-orange-300">{updates}</span>
                            )}
                        </button>
                    ))}
                </div>

                {/* Search + view toggle */}
                <div className="flex items-center gap-3">
                    <div className="relative flex-1">
                        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
                        <input type="text" placeholder="Search packages…" value={searchQuery}
                            onChange={e => setSearchQuery(e.target.value)}
                            className="w-full bg-slate-800 border border-white/10 rounded-lg py-2 pl-9 pr-4 text-sm text-white focus:outline-none focus:border-blue-500/50" />
                    </div>
                    <div className="flex gap-1 bg-slate-800 rounded-lg p-1">
                        <button onClick={() => setViewMode('grid')}
                            className={`p-1.5 rounded ${viewMode === 'grid' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/10'}`}>
                            <LayoutGrid size={15} />
                        </button>
                        <button onClick={() => setViewMode('list')}
                            className={`p-1.5 rounded ${viewMode === 'list' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:bg-white/10'}`}>
                            <List size={15} />
                        </button>
                    </div>
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-4">
                {error && (
                    <div className="bg-red-500/10 border border-red-500/30 rounded-xl p-3 mb-4 flex items-center gap-2 text-red-400 text-sm">
                        <AlertCircle size={15} /> {error}
                        <button onClick={() => setError(null)} className="ml-auto"><X size={14} /></button>
                    </div>
                )}

                {loading && filtered.length === 0 ? (
                    <div className="flex items-center justify-center h-48">
                        <Loader2 size={28} className="animate-spin text-blue-400" />
                    </div>
                ) : filtered.length === 0 ? (
                    <div className="text-center py-16 text-slate-500">
                        <Package size={40} className="mx-auto mb-3 opacity-40" />
                        <p className="text-sm">No packages found</p>
                        {activeTab === 'updates' && <p className="text-xs mt-1 text-slate-600">Everything is up to date</p>}
                        {activeTab === 'available' && !loading && (
                            <p className="text-xs mt-1 text-slate-600">
                                No packages returned — make sure apt/flatpak/snap is reachable and try refreshing.
                            </p>
                        )}
                    </div>
                ) : viewMode === 'grid' ? (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
                        {filtered.map(pkg => (
                            <PackageCard key={`${pkg.source}-${pkg.id}`}
                                pkg={pkg} tab={activeTab}
                                busy={activeAction === pkg.id}
                                onAction={performAction} />
                        ))}
                    </div>
                ) : (
                    <div className="space-y-1">
                        {filtered.map(pkg => (
                            <PackageRow key={`${pkg.source}-${pkg.id}`}
                                pkg={pkg} tab={activeTab}
                                busy={activeAction === pkg.id}
                                onAction={performAction} />
                        ))}
                    </div>
                )}
            </div>

            {installLog && (
                <InstallLogTerminal log={installLog} onClose={() => setInstallLog(null)} />
            )}
        </div>
    );
};

export default BlueSoftwareApp;
