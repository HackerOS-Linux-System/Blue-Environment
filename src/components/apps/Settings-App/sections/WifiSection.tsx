import React, { useState } from 'react';
import { RefreshCw, Lock, Unlock, Loader2, Wifi } from 'lucide-react';
import { SystemBridge } from '../../../../utils/systemBridge';

interface WifiNetwork { ssid: string; signal: number; secure: boolean; in_use: boolean; }

const WifiSection: React.FC = () => {
    const [networks, setNetworks] = useState<WifiNetwork[]>([]);
    const [scanning, setScanning] = useState(false);
    const [enabled, setEnabled] = useState(true);
    const [connecting, setConnecting] = useState<string | null>(null);

    const scan = async () => {
        setScanning(true);
        const nets = await SystemBridge.getWifiNetworks();
        setNetworks(nets);
        setScanning(false);
    };
    const connect = async (ssid: string) => {
        setConnecting(ssid);
        try { await SystemBridge.connectWifi(ssid, ''); await scan(); }
        catch { alert('Connection failed'); }
        finally { setConnecting(null); }
    };
    const toggle = async () => {
        const n = !enabled; setEnabled(n);
        await SystemBridge.toggleWifi(n);
        if (n) scan();
    };

    return (
        <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
            <div className="flex items-center justify-between">
                <h2 className="text-2xl font-bold text-white">Wi-Fi</h2>
                <button onClick={scan} disabled={scanning} className="p-2 bg-slate-800 rounded-full hover:bg-white/10">
                    <RefreshCw size={18} className={scanning ? 'animate-spin' : ''} />
                </button>
            </div>
            <div className="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
                <span className="text-white">Wi-Fi</span>
                <button onClick={toggle}
                    className={`w-12 h-6 rounded-full transition-colors relative ${enabled ? 'bg-blue-600' : 'bg-slate-600'}`}>
                    <div className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-transform ${enabled ? 'translate-x-7' : 'translate-x-1'}`} />
                </button>
            </div>
            <div className="bg-slate-800 border border-white/5 rounded-2xl overflow-hidden">
                {networks.length === 0 && !scanning && (
                    <div className="p-8 text-center text-slate-500 cursor-pointer hover:text-white" onClick={scan}>
                        No networks found — click to scan
                    </div>
                )}
                {scanning && <div className="p-4 text-center flex items-center justify-center gap-2 text-slate-500"><Loader2 size={16} className="animate-spin" /> Scanning...</div>}
                {networks.map((net, i) => (
                    <div key={i} className={`flex items-center justify-between p-4 border-b border-white/5 last:border-0 ${net.in_use ? 'bg-blue-600/10' : 'hover:bg-white/5'}`}>
                        <div className="flex items-center gap-3">
                            <Wifi size={20} className={net.signal > 60 ? 'text-green-400' : 'text-yellow-400'} />
                            <div>
                                <div className="font-medium text-white flex items-center gap-2">
                                    {net.ssid}
                                    {net.in_use && <span className="text-xs bg-green-500/20 text-green-400 px-2 rounded-full">Connected</span>}
                                </div>
                                <div className="text-xs text-slate-400 flex items-center gap-1">
                                    {net.secure ? <Lock size={10}/> : <Unlock size={10}/>}
                                    {net.secure ? 'Secured' : 'Open'} · {net.signal}%
                                </div>
                            </div>
                        </div>
                        {net.in_use
                            ? <button onClick={() => SystemBridge.disconnectWifi().then(scan)}
                                className="px-4 py-2 bg-red-500/20 text-red-400 rounded-lg text-sm hover:bg-red-500/40 transition-colors">Disconnect</button>
                            : <button onClick={() => connect(net.ssid)} disabled={connecting !== null}
                                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm disabled:opacity-50 transition-colors">
                                {connecting === net.ssid ? 'Connecting...' : 'Connect'}
                              </button>
                        }
                    </div>
                ))}
            </div>
        </div>
    );
};
export default WifiSection;
