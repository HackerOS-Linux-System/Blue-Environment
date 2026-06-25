import React from 'react';
import { Package, ArrowUpCircle, Download, Trash2, RefreshCw, Loader2, HardDrive } from 'lucide-react';
import { PackageInfo, SoftwareTab } from './types';

interface Props {
    pkg:        PackageInfo;
    tab:        SoftwareTab;
    busy:       boolean;
    onAction:   (pkg: PackageInfo, action: 'install' | 'remove' | 'update') => void;
}

export function sourceIcon(source: string) {
    if (source === 'dnf')     return <HardDrive size={12} className="text-blue-400" />;
    if (source === 'flatpak') return <Package   size={12} className="text-green-400" />;
    return <Download size={12} className="text-purple-400" />;
}

export function sourceBadge(source: string) {
    const colors: Record<string, string> = {
        dnf:      'bg-blue-500/15 text-blue-300 border-blue-500/20',
        flatpak:  'bg-green-500/15 text-green-300 border-green-500/20',
        appimage: 'bg-purple-500/15 text-purple-300 border-purple-500/20',
    };
    return (
        <span className={`text-[10px] px-1.5 py-0.5 rounded border font-mono ${colors[source] ?? 'bg-white/5 text-slate-400'}`}>
            {source}
        </span>
    );
}

const PackageCard: React.FC<Props> = ({ pkg, tab, busy, onAction }) => (
    <div className="bg-slate-800/60 rounded-xl p-4 border border-white/5 hover:border-white/10 transition-all flex flex-col gap-3">
        <div className="flex items-start gap-3">
            <div className="w-11 h-11 bg-slate-700 rounded-xl flex items-center justify-center shrink-0">
                {pkg.icon
                    ? <img src={pkg.icon} alt="" className="w-8 h-8 object-contain" onError={e => ((e.target as HTMLElement).style.display = 'none')} />
                    : <Package size={22} className="text-slate-400" />}
            </div>
            <div className="flex-1 min-w-0">
                <h3 className="font-semibold text-white text-sm truncate">{pkg.name}</h3>
                <div className="flex items-center gap-1.5 mt-0.5">
                    {sourceIcon(pkg.source)}
                    {sourceBadge(pkg.source)}
                    <span className="text-[10px] text-slate-500">{pkg.version}</span>
                </div>
            </div>
            {pkg.update_available && <ArrowUpCircle size={16} className="text-orange-400 shrink-0" />}
        </div>

        <p className="text-xs text-slate-400 line-clamp-2 flex-1">{pkg.description}</p>

        <div className="flex items-center justify-between">
            {pkg.size && <span className="text-[10px] text-slate-600">{pkg.size}</span>}
            <div className="ml-auto">
                {tab === 'available' && (
                    <button onClick={() => onAction(pkg, 'install')} disabled={busy}
                        className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs disabled:opacity-50 transition-colors">
                        {busy ? <Loader2 size={12} className="animate-spin" /> : <Download size={12} />} Install
                    </button>
                )}
                {tab === 'installed' && (
                    <button onClick={() => onAction(pkg, 'remove')} disabled={busy}
                        className="flex items-center gap-1 px-3 py-1.5 bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg text-xs disabled:opacity-50 transition-colors">
                        {busy ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />} Remove
                    </button>
                )}
                {tab === 'updates' && (
                    <button onClick={() => onAction(pkg, 'update')} disabled={busy}
                        className="flex items-center gap-1 px-3 py-1.5 bg-green-600 hover:bg-green-500 rounded-lg text-xs disabled:opacity-50 transition-colors">
                        {busy ? <Loader2 size={12} className="animate-spin" /> : <RefreshCw size={12} />} Update
                    </button>
                )}
            </div>
        </div>
    </div>
);

export default PackageCard;
