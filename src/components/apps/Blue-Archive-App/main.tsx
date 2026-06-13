import React, { useState, useRef } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import {
    Archive, FolderOpen, File, Folder, RefreshCw,
    Plus, Download, Search, X, Check, AlertTriangle
} from 'lucide-react';

interface ArchiveEntry { name: string; path: string; size: number; compressedSize: number; isDir: boolean; }
interface ArchiveInfo { name: string; format: string; totalSize: number; entries: ArchiveEntry[]; }

function fmtSize(b: number) { if(!b)return'0 B';if(b<1024)return`${b} B`;if(b<1048576)return`${(b/1024).toFixed(1)} KB`;if(b<1073741824)return`${(b/1048576).toFixed(1)} MB`;return`${(b/1073741824).toFixed(2)} GB`; }
function ratio(orig: number, comp: number) { if(!orig||!comp)return'—';return`${(100-(comp/orig)*100).toFixed(1)}%`; }

const BlueArchiveApp: React.FC<AppProps> = () => {
    const [archive, setArchive] = useState<ArchiveInfo|null>(null);
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string|null>(null);
    const [status, setStatus] = useState('');
    const [query, setQuery] = useState('');
    const fileInputRef = useRef<HTMLInputElement>(null);

    const openArchive = async (file: File) => {
        setLoading(true); setError(null);
        const ext = file.name.split('.').pop()?.toLowerCase()||'';
        let format = ext.toUpperCase();
        if(file.name.endsWith('.tar.gz')||file.name.endsWith('.tgz')) format='TAR.GZ';
        else if(file.name.endsWith('.tar.bz2')) format='TAR.BZ2';
        else if(file.name.endsWith('.tar.xz')) format='TAR.XZ';
        else if(file.name.endsWith('.tar.zst')) format='TAR.ZST';

        setArchive({ name: file.name, format, totalSize: file.size, entries: [] });
        setStatus('Archive opened — extraction requires Tauri backend');
        setLoading(false);
    };

    const handleFileOpen = (e: React.ChangeEvent<HTMLInputElement>) => {
        const f = e.target.files?.[0]; if(f) openArchive(f); e.target.value='';
    };

    const extractAll = async () => {
        if(!archive) return;
        setStatus('To extract: use terminal — tar -xzf archivename.tar.gz');
        setTimeout(()=>setStatus(''), 4000);
    };

    const filtered = (archive?.entries||[]).filter(e=>!query||e.name.toLowerCase().includes(query.toLowerCase()));
    const selAll=()=>setSelected(new Set(filtered.map(e=>e.path)));
    const selNone=()=>setSelected(new Set());
    const toggle=(p:string)=>setSelected(prev=>{const n=new Set(prev);n.has(p)?n.delete(p):n.add(p);return n;});

    const FORMATS=['ZIP','TAR.GZ','TAR.XZ','TAR.BZ2','7-Zip','RAR','ZST'];

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white overflow-hidden">
            <input ref={fileInputRef} type="file" className="hidden" onChange={handleFileOpen} accept=".zip,.tar,.gz,.tgz,.bz2,.xz,.7z,.rar,.zst"/>
            <div className="shrink-0 flex items-center gap-1 px-3 py-2 bg-slate-800 border-b border-white/5">
                <button onClick={()=>fileInputRef.current?.click()} className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm"><FolderOpen size={14}/> Open</button>
                {archive&&<button onClick={extractAll} className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm"><Download size={14}/> Extract All</button>}
                <div className="flex-1"/>
                {archive&&(
                    <div className="relative">
                        <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-slate-500"/>
                        <input value={query} onChange={e=>setQuery(e.target.value)} placeholder="Search..." className="bg-slate-800 border border-white/10 rounded-lg pl-7 pr-3 py-1.5 text-sm w-36 focus:outline-none"/>
                    </div>
                )}
            </div>

            {archive&&(
                <div className="shrink-0 flex items-center gap-4 px-4 py-2 bg-slate-800/50 border-b border-white/5 text-xs text-slate-400">
                    <div className="flex items-center gap-1.5"><Archive size={13} className="text-blue-400"/><span className="text-white font-medium">{archive.name}</span></div>
                    <span className="bg-blue-600/20 text-blue-300 px-2 py-0.5 rounded text-[10px] font-mono">{archive.format}</span>
                    <span>{fmtSize(archive.totalSize)}</span>
                    <button onClick={()=>{setArchive(null);selNone();}} className="ml-auto hover:text-white"><X size={13}/></button>
                </div>
            )}

            <div className="flex-1 overflow-hidden">
                {!archive?(
                    <div className="flex flex-col items-center justify-center h-full gap-6">
                        <div className="w-24 h-24 bg-slate-800 rounded-2xl flex items-center justify-center border border-white/5">
                            <Archive size={40} className="text-slate-600"/>
                        </div>
                        <div className="text-center">
                            <h3 className="text-lg font-semibold mb-1">Blue Archive</h3>
                            <p className="text-slate-400 text-sm">Open an archive to inspect its contents</p>
                            <p className="text-slate-500 text-xs mt-1">Supports: {FORMATS.join(', ')}</p>
                        </div>
                        <button onClick={()=>fileInputRef.current?.click()} className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">
                            <FolderOpen size={15}/> Open Archive
                        </button>
                        <div className="flex gap-2 flex-wrap justify-center">
                            {FORMATS.map(f=><span key={f} className="text-[10px] bg-white/5 text-slate-500 px-2 py-1 rounded font-mono">{f}</span>)}
                        </div>
                    </div>
                ):(
                    <div className="flex flex-col h-full">
                        <div className="shrink-0 flex items-center gap-2 px-4 py-1.5 bg-slate-800/30 border-b border-white/5 text-xs text-slate-500">
                            <input type="checkbox" checked={selected.size===filtered.length&&filtered.length>0} onChange={e=>e.target.checked?selAll():selNone()} className="accent-blue-500"/>
                            <span>{archive.entries.length} entries</span>
                        </div>
                        <div className="flex-1 overflow-y-auto">
                            {loading?(
                                <div className="flex items-center justify-center h-32"><RefreshCw size={20} className="animate-spin text-blue-400"/></div>
                            ):filtered.length===0?(
                                <div className="flex flex-col items-center justify-center h-32 text-slate-500 text-sm">Archive loaded — entries require Tauri backend</div>
                            ):(
                                <table className="w-full text-sm">
                                    <thead className="sticky top-0 bg-slate-800 z-10">
                                        <tr className="text-slate-500 text-xs">
                                            <th className="w-8 py-2 pl-4"><input type="checkbox" className="accent-blue-500"/></th>
                                            <th className="text-left py-2 font-medium">Name</th>
                                            <th className="text-right py-2 pr-3 font-medium w-28">Size</th>
                                            <th className="text-right py-2 pr-3 font-medium w-20">Ratio</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {filtered.map(entry=>(
                                            <tr key={entry.path} onClick={()=>toggle(entry.path)} className={`cursor-pointer border-b border-white/5 ${selected.has(entry.path)?'bg-blue-600/15':'hover:bg-white/5'}`}>
                                                <td className="pl-4 py-2"><input type="checkbox" checked={selected.has(entry.path)} onChange={()=>toggle(entry.path)} onClick={e=>e.stopPropagation()} className="accent-blue-500"/></td>
                                                <td className="py-2">
                                                    <div className="flex items-center gap-2">
                                                        {entry.isDir?<Folder size={14} className="text-blue-400"/>:<File size={14} className="text-slate-500"/>}
                                                        <span className="text-slate-200 text-xs truncate">{entry.name}</span>
                                                    </div>
                                                </td>
                                                <td className="py-2 pr-3 text-right text-xs text-slate-400">{fmtSize(entry.size)}</td>
                                                <td className="py-2 pr-3 text-right text-xs text-green-500">{ratio(entry.size,entry.compressedSize)}</td>
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            )}
                        </div>
                    </div>
                )}
            </div>

            <div className="shrink-0 px-4 py-1.5 bg-slate-800/50 border-t border-white/5 text-xs text-slate-500 flex items-center gap-2">
                {error&&<><AlertTriangle size={11} className="text-red-400"/><span className="text-red-400">{error}</span></>}
                {status&&<><RefreshCw size={11} className="animate-spin text-blue-400"/><span className="text-blue-300">{status}</span></>}
                {!error&&!status&&archive&&<span>Ready · {archive.format}</span>}
                {!error&&!status&&!archive&&<span>Blue Archive v0.6</span>}
            </div>
        </div>
    );
};
export default BlueArchiveApp;
