import React, { useState, useEffect } from 'react';
import { SystemBridge } from '../utils/systemBridge';
import { useDialog } from '../contexts/DialogContext';
import { Monitor, RefreshCw, Plus, Trash2, User, Users, Printer, Check, X } from 'lucide-react';

// ── Monitors ─────────────────────────────────────────────────────────────

interface MonitorInfo {
    name: string; resolution: string; rate: string; scale: string;
    primary: boolean; connected: boolean; x: number; y: number;
}

export function MonitorsSection() {
    const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
    const [loading, setLoading] = useState(true);

    const load = async () => {
        setLoading(true);
        try {
            const result = await SystemBridge.executeCommand(
                "xrandr --query 2>/dev/null | grep -E 'connected|\\*' || wlr-randr 2>/dev/null || echo 'UNAVAILABLE'"
            );
            const out = typeof result === 'string' ? result : result?.stdout || '';
            if (out.includes('UNAVAILABLE') || !out.trim()) {
                setMonitors([{ name: 'Primary Monitor', resolution: '1920x1080', rate: '60.0', scale: '1.0', primary: true, connected: true, x: 0, y: 0 }]);
            } else {
                // Parse xrandr output
                const mons: MonitorInfo[] = [];
                const lines = out.split('\n');
                let current: Partial<MonitorInfo> | null = null;
                for (const line of lines) {
                    if (line.match(/^[A-Z].*connected/)) {
                        if (current?.name) mons.push(current as MonitorInfo);
                        const parts = line.split(' ');
                        current = {
                            name: parts[0], connected: true,
                            primary: line.includes('primary'),
                            resolution: '—', rate: '—', scale: '1.0', x: 0, y: 0,
                        };
                        const geom = line.match(/(\d+)x(\d+)\+(\d+)\+(\d+)/);
                        if (geom) {
                            current.resolution = `${geom[1]}x${geom[2]}`;
                            current.x = parseInt(geom[3]); current.y = parseInt(geom[4]);
                        }
                    } else if (current && line.match(/^\s+\d+x\d+.*\*/)) {
                        const m = line.match(/(\d+\.\d+)\*/);
                        if (m) current.rate = m[1];
                    }
                }
                if (current?.name) mons.push(current as MonitorInfo);
                setMonitors(mons.length > 0 ? mons : [{ name: 'Primary', resolution: '1920x1080', rate: '60.0', scale: '1.0', primary: true, connected: true, x: 0, y: 0 }]);
            }
        } catch { setMonitors([]); }
        setLoading(false);
    };

    useEffect(() => { load(); }, []);

    const applyScale = async (mon: MonitorInfo, scale: string) => {
        await SystemBridge.executeCommand(`xrandr --output "${mon.name}" --scale ${scale}x${scale}`).catch(() => {});
        load();
    };

    const applyRotation = async (mon: MonitorInfo, rotation: string) => {
        await SystemBridge.executeCommand(`xrandr --output "${mon.name}" --rotate ${rotation}`).catch(() => {});
    };

    if (loading) return <div className="flex items-center justify-center py-12"><RefreshCw size={20} className="animate-spin text-blue-400" /></div>;

    return (
        <div className="space-y-4">
            {/* Visual layout */}
            <div className="bg-slate-800 rounded-xl p-4 border border-white/5 relative overflow-hidden" style={{ minHeight: 120 }}>
                <div className="flex items-center gap-2 flex-wrap">
                    {monitors.map((mon, i) => (
                        <div key={mon.name} className={`border-2 rounded-lg flex items-center justify-center ${mon.primary ? 'border-blue-500' : 'border-white/20'}`}
                            style={{ width: 120, height: 80, background: 'rgba(255,255,255,0.05)' }}>
                            <div className="text-center">
                                <Monitor size={20} className="mx-auto text-blue-400 mb-1" />
                                <div className="text-xs text-white truncate px-1">{mon.name}</div>
                                <div className="text-[10px] text-slate-400">{mon.resolution}</div>
                            </div>
                        </div>
                    ))}
                </div>
                <button onClick={load} className="absolute top-2 right-2 p-1 hover:bg-white/10 rounded text-slate-500"><RefreshCw size={12} /></button>
            </div>

            {monitors.map(mon => (
                <div key={mon.name} className="bg-slate-800 rounded-xl p-5 border border-white/5 space-y-4">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <Monitor size={20} className="text-blue-400" />
                            <div>
                                <div className="font-semibold text-white">{mon.name}</div>
                                <div className="text-xs text-slate-500">{mon.connected ? 'Connected' : 'Disconnected'}</div>
                            </div>
                        </div>
                        {mon.primary && <span className="text-xs bg-blue-600/20 text-blue-300 px-2 py-0.5 rounded-full border border-blue-500/20">Primary</span>}
                    </div>
                    <div className="grid grid-cols-2 gap-3 text-sm">
                        <div className="bg-slate-900/50 rounded-lg p-3">
                            <div className="text-slate-400 text-xs mb-1">Resolution</div>
                            <div className="font-medium">{mon.resolution}</div>
                        </div>
                        <div className="bg-slate-900/50 rounded-lg p-3">
                            <div className="text-slate-400 text-xs mb-1">Refresh Rate</div>
                            <div className="font-medium">{mon.rate} Hz</div>
                        </div>
                    </div>
                    <div className="flex items-center gap-3">
                        <div className="flex-1">
                            <label className="text-xs text-slate-400 block mb-1">Scale</label>
                            <select defaultValue={mon.scale} onChange={e => applyScale(mon, e.target.value)}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
                                {['0.5', '0.75', '1.0', '1.25', '1.5', '1.75', '2.0', '2.5', '3.0'].map(s =>
                                    <option key={s} value={s}>{(parseFloat(s) * 100).toFixed(0)}%</option>
                                )}
                            </select>
                        </div>
                        <div className="flex-1">
                            <label className="text-xs text-slate-400 block mb-1">Rotation</label>
                            <select defaultValue="normal" onChange={e => applyRotation(mon, e.target.value)}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
                                <option value="normal">Normal (0°)</option>
                                <option value="right">Right (90°)</option>
                                <option value="inverted">Inverted (180°)</option>
                                <option value="left">Left (270°)</option>
                            </select>
                        </div>
                    </div>
                </div>
            ))}
        </div>
    );
}

// ── Printers ─────────────────────────────────────────────────────────────

interface PrinterInfo { name: string; status: string; isDefault: boolean; location?: string; }

export function PrintersSection() {
    const dialog = useDialog();
    const [printers, setPrinters] = useState<PrinterInfo[]>([]);
    const [loading, setLoading] = useState(true);

    const load = async () => {
        setLoading(true);
        try {
            const result = await SystemBridge.executeCommand('lpstat -p 2>/dev/null || echo "No CUPS"');
            const out = typeof result === 'string' ? result : result?.stdout || '';
            if (out.includes('No CUPS') || !out.trim()) { setPrinters([]); setLoading(false); return; }
            const defResult = await SystemBridge.executeCommand('lpstat -d 2>/dev/null | cut -d: -f2');
            const defOut = typeof defResult === 'string' ? defResult : defResult?.stdout || '';
            const defaultPrinter = defOut.trim();
            const ps: PrinterInfo[] = [];
            for (const line of out.split('\n')) {
                const m = line.match(/^printer\s+(\S+)\s+is\s+(.+)/);
                if (m) ps.push({ name: m[1], status: m[2].trim(), isDefault: m[1] === defaultPrinter });
            }
            setPrinters(ps);
        } catch { setPrinters([]); }
        setLoading(false);
    };

    useEffect(() => { load(); }, []);

    const setDefault = async (name: string) => {
        await SystemBridge.executeCommand(`lpoptions -d "${name}"`).catch(() => {});
        load();
    };

    const removePrinter = async (name: string) => {
        const ok = await dialog.confirm({
            title: 'Remove printer',
            message: `Remove printer "${name}"? You can add it again later.`,
            confirmLabel: 'Remove',
            danger: true,
        });
        if (!ok) return;
        await SystemBridge.executeCommand(`lpadmin -x "${name}"`).catch(() => {});
        load();
    };

    if (loading) return <div className="flex items-center justify-center py-12"><RefreshCw size={20} className="animate-spin text-blue-400" /></div>;

    return (
        <div className="space-y-4">
            <div className="flex items-center justify-between">
                <p className="text-sm text-slate-400">Manage printers via CUPS</p>
                <button onClick={() => SystemBridge.launchApp('system-config-printer &').catch(() => {})}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm transition-colors">
                    <Plus size={13} /> Add Printer
                </button>
            </div>
            {printers.length === 0 ? (
                <div className="bg-slate-800 rounded-xl p-8 text-center border border-white/5">
                    <Printer size={32} className="mx-auto text-slate-600 mb-3" />
                    <p className="text-slate-400">No printers configured</p>
                    <p className="text-xs text-slate-600 mt-1">Install CUPS and add a printer</p>
                </div>
            ) : (
                printers.map(p => (
                    <div key={p.name} className="bg-slate-800 rounded-xl p-4 border border-white/5 flex items-center gap-4">
                        <div className="w-10 h-10 bg-white/5 rounded-xl flex items-center justify-center shrink-0">
                            <Printer size={18} className="text-slate-400" />
                        </div>
                        <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                                <span className="font-medium text-white">{p.name}</span>
                                {p.isDefault && <span className="text-xs bg-green-600/20 text-green-300 px-1.5 py-0.5 rounded border border-green-500/20">Default</span>}
                            </div>
                            <div className="text-xs text-slate-500 capitalize">{p.status}</div>
                        </div>
                        <div className="flex gap-1">
                            {!p.isDefault && (
                                <button onClick={() => setDefault(p.name)} className="px-2 py-1 text-xs bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors">
                                    Set Default
                                </button>
                            )}
                            <button onClick={() => removePrinter(p.name)} className="p-1.5 hover:text-red-400 text-slate-500 transition-colors">
                                <Trash2 size={13} />
                            </button>
                        </div>
                    </div>
                ))
            )}
        </div>
    );
}

// ── Users & Groups ────────────────────────────────────────────────────────

interface UserInfo { username: string; uid: string; shell: string; home: string; groups: string[]; }

export function UsersSection() {
    const [users, setUsers] = useState<UserInfo[]>([]);
    const [loading, setLoading] = useState(true);
    const [currentUser] = useState(SystemBridge.getUsername());

    const load = async () => {
        setLoading(true);
        try {
            const result = await SystemBridge.executeCommand(
                "awk -F: '$3 >= 1000 && $3 < 65534 && $7 !~ /nologin|false|sync|halt|shutdown/ && $6 != \"/var/empty\" && $6 != \"/nonexistent\" {print $1\":\"$3\":\"$6\":\"$7}' /proc/1/root/etc/passwd 2>/dev/null || " +
                "awk -F: '$3 >= 1000 && $3 < 65534 && $7 !~ /nologin|false|sync|halt|shutdown/ && $6 != \"/var/empty\" && $6 != \"/nonexistent\" {print $1\":\"$3\":\"$6\":\"$7}' /etc/passwd"
            );
            const out = typeof result === 'string' ? result : result?.stdout || '';
            const us: UserInfo[] = [];
            for (const line of out.split('\n').filter(Boolean)) {
                const parts = line.split(':');
                if (parts.length < 4) continue;
                const [username, uid, home, shell] = parts;
                // Get groups
                const grpResult = await SystemBridge.executeCommand(`groups ${username} 2>/dev/null`);
                const grpOut = typeof grpResult === 'string' ? grpResult : grpResult?.stdout || '';
                const groups = grpOut.split(':').slice(-1)[0]?.trim().split(' ') || [];
                us.push({ username, uid, home, shell, groups });
            }
            setUsers(us.length > 0 ? us : [{ username: currentUser, uid: '1000', home: `/home/${currentUser}`, shell: '/bin/bash', groups: ['sudo', 'audio', 'video'] }]);
        } catch { setUsers([]); }
        setLoading(false);
    };

    useEffect(() => { load(); }, []);

    const changePassword = async (username: string) => {
        if (username === currentUser) {
            await SystemBridge.launchApp('bash -c "xterm -e passwd" &').catch(() => {});
        } else {
            await SystemBridge.launchApp(`bash -c "xterm -e 'sudo passwd ${username}'" &`).catch(() => {});
        }
    };

    if (loading) return <div className="flex items-center justify-center py-12"><RefreshCw size={20} className="animate-spin text-blue-400" /></div>;

    return (
        <div className="space-y-4">
            <div className="flex items-center justify-between">
                <p className="text-sm text-slate-400">System users (UID 1000+)</p>
                <button onClick={() => SystemBridge.launchApp('xterm -e "sudo adduser" &').catch(() => {})}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm transition-colors">
                    <Plus size={13} /> Add User
                </button>
            </div>
            {users.map(u => (
                <div key={u.username} className="bg-slate-800 rounded-xl p-4 border border-white/5">
                    <div className="flex items-center gap-4">
                        <div className="w-12 h-12 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-xl font-bold shrink-0">
                            {u.username.charAt(0).toUpperCase()}
                        </div>
                        <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                                <span className="font-medium text-white">{u.username}</span>
                                {u.username === currentUser && (
                                    <span className="text-xs bg-blue-600/20 text-blue-300 px-1.5 py-0.5 rounded border border-blue-500/20">You</span>
                                )}
                                {u.groups.includes('sudo') || u.groups.includes('wheel') ? (
                                    <span className="text-xs bg-orange-600/20 text-orange-300 px-1.5 py-0.5 rounded border border-orange-500/20">Admin</span>
                                ) : null}
                            </div>
                            <div className="text-xs text-slate-500 mt-0.5">UID: {u.uid} · {u.home} · {u.shell.split('/').pop()}</div>
                            <div className="flex flex-wrap gap-1 mt-1.5">
                                {u.groups.slice(0, 6).map(g => (
                                    <span key={g} className="text-[10px] bg-white/5 text-slate-400 px-1.5 py-0.5 rounded">{g}</span>
                                ))}
                                {u.groups.length > 6 && <span className="text-[10px] text-slate-600">+{u.groups.length - 6} more</span>}
                            </div>
                        </div>
                        <div className="flex gap-1">
                            <button onClick={() => changePassword(u.username)}
                                className="px-2 py-1 text-xs bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors whitespace-nowrap">
                                Change Password
                            </button>
                        </div>
                    </div>
                </div>
            ))}
        </div>
    );
}
