import React, { useState, useEffect, useCallback, useRef } from "react";
import { AppProps, PackageInfo } from "../../../types";
import { useLanguage } from "../../../contexts/LanguageContext";
import { SystemBridge } from "../../../utils/systemBridge";
import {
    Package, Download, Trash2, RefreshCw, Search, Zap,
    HardDrive, Loader2, LayoutGrid, List, AlertCircle, Check,
    Terminal, X, ArrowUpCircle,
} from "lucide-react";

interface InstallLog { pkgId: string; lines: string[]; done: boolean; success: boolean; }

const BlueSoftwareApp: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [activeTab, setActiveTab] = useState<"available" | "installed" | "updates">("available");
    const [searchQuery, setSearchQuery] = useState("");
    const [viewMode, setViewMode] = useState<"grid" | "list">("grid");
    const [packages, setPackages] = useState<PackageInfo[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [activeAction, setActiveAction] = useState<string | null>(null);
    const [installLog, setInstallLog] = useState<InstallLog | null>(null);
    const isMounted = useRef(true);

    const loadPackages = useCallback(async () => {
        setLoading(true); setError(null);
        try {
            const [apt, flatpak, snap, appimage] = await Promise.allSettled([
                SystemBridge.getAptPackages(),
                                                                            SystemBridge.getFlatpakPackages(),
                                                                            SystemBridge.getSnapPackages(),
                                                                            SystemBridge.getAppImagePackages(),
            ]);
            const all: PackageInfo[] = [];
            for (const r of [apt, flatpak, snap, appimage]) {
                if (r.status === "fulfilled") all.push(...r.value);
            }
            if (isMounted.current) setPackages(all.length > 0 ? all : MOCK_PACKAGES);
        } catch (e: any) {
            if (isMounted.current) { setPackages(MOCK_PACKAGES); setError(null); }
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
        if (activeTab === "available")  return matchSearch && !p.installed;
        if (activeTab === "installed")  return matchSearch && p.installed;
        if (activeTab === "updates")    return matchSearch && p.installed && p.updateAvailable;
        return matchSearch;
    });

    const performAction = async (pkg: PackageInfo, action: "install" | "remove" | "update") => {
        setActiveAction(pkg.id);
        setInstallLog({ pkgId: pkg.id, lines: [`${action === "install" ? "Installing" : action === "remove" ? "Removing" : "Updating"} ${pkg.name}...`], done: false, success: false });

        try {
            let ok = false;
            const addLog = (line: string) => setInstallLog(prev => prev ? { ...prev, lines: [...prev.lines, line] } : null);

            addLog(`Source: ${pkg.source}`);

            if (action === "install") {
                if (pkg.source === "apt")      ok = await SystemBridge.installAptPackage(pkg.id);
                else if (pkg.source === "flatpak") ok = await SystemBridge.installFlatpakPackage(pkg.id);
                else if (pkg.source === "snap")    ok = await SystemBridge.installSnapPackage(pkg.id);
                else                               ok = await SystemBridge.installAppImage(pkg.id);
            } else if (action === "remove") {
                if (pkg.source === "apt")      ok = await SystemBridge.removeAptPackage(pkg.id);
                else if (pkg.source === "flatpak") ok = await SystemBridge.removeFlatpakPackage(pkg.id);
                else if (pkg.source === "snap")    ok = await SystemBridge.removeSnapPackage(pkg.id);
                else                               ok = await SystemBridge.removeAppImage(pkg.id);
            } else {
                if (pkg.source === "apt")      ok = await SystemBridge.updateAptPackage(pkg.id);
                else if (pkg.source === "flatpak") ok = await SystemBridge.updateFlatpakPackage(pkg.id);
                else                               ok = await SystemBridge.updateSnapPackage(pkg.id);
            }

            addLog(ok ? `✓ Done successfully` : `✗ Operation failed`);
            setInstallLog(prev => prev ? { ...prev, done: true, success: ok } : null);

            if (ok || !SystemBridge.isTauri()) await loadPackages();
        } catch (e: any) {
            setError(e.message);
            setInstallLog(prev => prev ? { ...prev, done: true, success: false, lines: [...(prev?.lines ?? []), `Error: ${e.message}`] } : null);
        } finally {
            setActiveAction(null);
        }
    };

    const sourceIcon = (source: string) => {
        if (source === "apt")      return <HardDrive size={12} className="text-blue-400" />;
        if (source === "flatpak")  return <Package   size={12} className="text-green-400" />;
        if (source === "snap")     return <Zap       size={12} className="text-yellow-400" />;
        return <Download size={12} className="text-purple-400" />;
    };

    const sourceBadge = (source: string) => {
        const colors: Record<string, string> = {
            apt: "bg-blue-500/15 text-blue-300 border-blue-500/20",
            flatpak: "bg-green-500/15 text-green-300 border-green-500/20",
            snap: "bg-yellow-500/15 text-yellow-300 border-yellow-500/20",
            appimage: "bg-purple-500/15 text-purple-300 border-purple-500/20",
        };
        return (
            <span className={`text-[10px] px-1.5 py-0.5 rounded border font-mono ${colors[source] ?? "bg-white/5 text-slate-400"}`}>
            {source}
            </span>
        );
    };

    const installed = packages.filter(p => p.installed).length;
    const updates   = packages.filter(p => p.installed && p.updateAvailable).length;

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
        <RefreshCw size={16} className={loading ? "animate-spin" : ""} />
        </button>
        </div>

        {/* Tabs */}
        <div className="flex gap-0 border-b border-white/10 mb-3">
        {([
            ["available", `Available (${packages.filter(p => !p.installed).length})`],
          ["installed", `Installed (${installed})`],
          ["updates",   `Updates (${updates})`],
        ] as const).map(([tab, label]) => (
            <button key={tab} onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px
                ${activeTab === tab ? "border-blue-500 text-blue-400" : "border-transparent text-slate-400 hover:text-white"}`}>
                {label}
                {tab === "updates" && updates > 0 && (
                    <span className="ml-1.5 px-1.5 py-0.5 rounded-full text-[10px] bg-orange-500/20 text-orange-300">{updates}</span>
                )}
                </button>
        ))}
        </div>

        {/* Search + view */}
        <div className="flex items-center gap-3">
        <div className="relative flex-1">
        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
        <input type="text" placeholder="Search packages…" value={searchQuery}
        onChange={e => setSearchQuery(e.target.value)}
        className="w-full bg-slate-800 border border-white/10 rounded-lg py-2 pl-9 pr-4
        text-sm text-white focus:outline-none focus:border-blue-500/50" />
        </div>
        <div className="flex gap-1 bg-slate-800 rounded-lg p-1">
        <button onClick={() => setViewMode("grid")}
        className={`p-1.5 rounded ${viewMode === "grid" ? "bg-blue-600 text-white" : "text-slate-400 hover:bg-white/10"}`}>
        <LayoutGrid size={15} />
        </button>
        <button onClick={() => setViewMode("list")}
        className={`p-1.5 rounded ${viewMode === "list" ? "bg-blue-600 text-white" : "text-slate-400 hover:bg-white/10"}`}>
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
            {activeTab === "updates" && <p className="text-xs mt-1 text-slate-600">Everything is up to date</p>}
            </div>
        ) : viewMode === "grid" ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
            {filtered.map(pkg => {
                const busy = activeAction === pkg.id;
                return (
                    <div key={`${pkg.source}-${pkg.id}`}
                    className="bg-slate-800/60 rounded-xl p-4 border border-white/5 hover:border-white/10 transition-all flex flex-col gap-3">
                    <div className="flex items-start gap-3">
                    <div className="w-11 h-11 bg-slate-700 rounded-xl flex items-center justify-center shrink-0">
                    {pkg.icon
                        ? <img src={pkg.icon} alt="" className="w-8 h-8 object-contain" onError={e => (e.target as any).style.display="none"} />
                        : <Package size={22} className="text-slate-400" />
                    }
                    </div>
                    <div className="flex-1 min-w-0">
                    <h3 className="font-semibold text-white text-sm truncate">{pkg.name}</h3>
                    <div className="flex items-center gap-1.5 mt-0.5">
                    {sourceIcon(pkg.source)}
                    {sourceBadge(pkg.source)}
                    <span className="text-[10px] text-slate-500">{pkg.version}</span>
                    </div>
                    </div>
                    {pkg.updateAvailable && (
                        <span title="Update available"><ArrowUpCircle size={16} className="text-orange-400 shrink-0" /></span>
                    )}
                    </div>
                    <p className="text-xs text-slate-400 line-clamp-2 flex-1">{pkg.description}</p>
                    <div className="flex items-center justify-between">
                    {pkg.size && <span className="text-[10px] text-slate-600">{pkg.size}</span>}
                    <div className="ml-auto">
                    {activeTab === "available" && (
                        <button onClick={() => performAction(pkg, "install")} disabled={busy}
                        className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs disabled:opacity-50 transition-colors">
                        {busy ? <Loader2 size={12} className="animate-spin" /> : <Download size={12} />}
                        Install
                        </button>
                    )}
                    {activeTab === "installed" && (
                        <button onClick={() => performAction(pkg, "remove")} disabled={busy}
                        className="flex items-center gap-1 px-3 py-1.5 bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg text-xs disabled:opacity-50 transition-colors">
                        {busy ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />}
                        Remove
                        </button>
                    )}
                    {activeTab === "updates" && (
                        <button onClick={() => performAction(pkg, "update")} disabled={busy}
                        className="flex items-center gap-1 px-3 py-1.5 bg-green-600 hover:bg-green-500 rounded-lg text-xs disabled:opacity-50 transition-colors">
                        {busy ? <Loader2 size={12} className="animate-spin" /> : <RefreshCw size={12} />}
                        Update
                        </button>
                    )}
                    </div>
                    </div>
                    </div>
                );
            })}
            </div>
        ) : (
            /* List view */
            <div className="space-y-1">
            {filtered.map(pkg => {
                const busy = activeAction === pkg.id;
                return (
                    <div key={`${pkg.source}-${pkg.id}`}
                    className="flex items-center gap-3 p-3 bg-slate-800/40 hover:bg-slate-800/70 rounded-xl border border-white/5 transition-colors">
                    <div className="w-9 h-9 bg-slate-700 rounded-lg flex items-center justify-center shrink-0">
                    {pkg.icon
                        ? <img src={pkg.icon} alt="" className="w-7 h-7 object-contain" onError={e => (e.target as any).style.display="none"} />
                        : <Package size={18} className="text-slate-400" />
                    }
                    </div>
                    <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                    <span className="font-medium text-white text-sm">{pkg.name}</span>
                    {sourceBadge(pkg.source)}
                    <span className="text-[10px] text-slate-500">{pkg.version}</span>
                    {pkg.updateAvailable && <span className="text-[10px] text-orange-400">● update</span>}
                    </div>
                    <div className="text-xs text-slate-500 truncate">{pkg.description}</div>
                    </div>
                    <div className="shrink-0">
                    {activeTab === "available" && (
                        <button onClick={() => performAction(pkg, "install")} disabled={busy}
                        className="px-3 py-1 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs disabled:opacity-50 transition-colors flex items-center gap-1">
                        {busy ? <Loader2 size={11} className="animate-spin" /> : null} Install
                        </button>
                    )}
                    {activeTab === "installed" && (
                        <button onClick={() => performAction(pkg, "remove")} disabled={busy}
                        className="px-3 py-1 bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg text-xs disabled:opacity-50 transition-colors">
                        Remove
                        </button>
                    )}
                    {activeTab === "updates" && (
                        <button onClick={() => performAction(pkg, "update")} disabled={busy}
                        className="px-3 py-1 bg-green-600 hover:bg-green-500 rounded-lg text-xs disabled:opacity-50 transition-colors flex items-center gap-1">
                        {busy ? <Loader2 size={11} className="animate-spin" /> : null} Update
                        </button>
                    )}
                    </div>
                    </div>
                );
            })}
            </div>
        )}
        </div>

        {/* Install log terminal */}
        {installLog && (
            <div className="border-t border-white/5 bg-slate-950 shrink-0" style={{ maxHeight: 160 }}>
            <div className="flex items-center justify-between px-3 py-1.5 border-b border-white/5">
            <span className="text-xs text-slate-400 flex items-center gap-1.5">
            <Terminal size={12} />
            {installLog.done
                ? installLog.success
                ? <span className="text-green-400 flex items-center gap-1"><Check size={11} /> Done</span>
                : <span className="text-red-400 flex items-center gap-1"><AlertCircle size={11} /> Failed</span>
                : <span className="flex items-center gap-1"><Loader2 size={11} className="animate-spin" /> Working…</span>
            }
            </span>
            {installLog.done && (
                <button onClick={() => setInstallLog(null)} className="text-slate-500 hover:text-white"><X size={12} /></button>
            )}
            </div>
            <div className="overflow-y-auto p-3 font-mono text-xs text-slate-300 space-y-0.5" style={{ maxHeight: 110 }}>
            {installLog.lines.map((line, i) => (
                <div key={i} className={line.startsWith("✓") ? "text-green-400" : line.startsWith("✗") ? "text-red-400" : ""}>{line}</div>
            ))}
            </div>
            </div>
        )}
        </div>
    );
};

const MOCK_PACKAGES: PackageInfo[] = [
    { id: "firefox",    name: "Firefox",    description: "Fast, private web browser",      version: "125.0", source: "apt",      installed: false },
{ id: "vlc",        name: "VLC",        description: "Versatile media player",          version: "3.0.20",source: "apt",      installed: true, updateAvailable: true },
{ id: "gimp",       name: "GIMP",       description: "GNU Image Manipulation Program",  version: "2.10.36",source:"apt",      installed: false },
{ id: "libreoffice",name: "LibreOffice",description: "Open source office suite",        version: "7.6.0", source: "apt",      installed: true },
{ id: "code",       name: "VS Code",    description: "Code editor by Microsoft",        version: "1.89.0",source: "flatpak",  installed: false },
{ id: "discord",    name: "Discord",    description: "Chat for communities",             version: "0.0.45",source: "flatpak",  installed: false },
{ id: "spotify",    name: "Spotify",    description: "Music streaming",                  version: "1.2.35",source: "snap",     installed: false },
{ id: "obsidian",   name: "Obsidian",   description: "Knowledge base & notes",           version: "1.4.16",source: "appimage", installed: false },
{ id: "inkscape",   name: "Inkscape",   description: "Vector graphics editor",           version: "1.3.0", source: "flatpak",  installed: true, updateAvailable: false },
{ id: "blender",    name: "Blender",    description: "3D creation suite",                version: "4.1.0", source: "flatpak",  installed: false },
{ id: "kdenlive",   name: "Kdenlive",   description: "Video editor",                     version: "24.02", source: "flatpak",  installed: false },
{ id: "audacity",   name: "Audacity",   description: "Audio recording and editing",      version: "3.5.0", source: "flatpak",  installed: false },
];

export default BlueSoftwareApp;
