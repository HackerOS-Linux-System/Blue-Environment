import React, { useState, useEffect, useCallback } from 'react';
import {
    Wifi, WifiOff, Bluetooth, BluetoothOff, Volume2, VolumeX,
    Sun, Moon, BatteryCharging, Battery, ChevronRight,
    RefreshCw, Speaker, Loader2, Check
} from 'lucide-react';
import { SystemBridge } from '../utils/systemBridge';

interface ControlCenterProps {
    isOpen: boolean;
    onOpenSettings: (tab?: string) => void;
}

interface AudioSink {
    id: number;
    name: string;
    description: string;
    volume: number;
    muted: boolean;
    is_default: boolean;
}

const ControlCenter: React.FC<ControlCenterProps> = ({ isOpen, onOpenSettings }) => {
    const [volume, setVolume] = useState(60);
    const [brightness, setBrightness] = useState(80);
    const [battery, setBattery] = useState(85);
    const [isCharging, setIsCharging] = useState(false);
    const [wifiEnabled, setWifiEnabled] = useState(true);
    const [wifiSSID, setWifiSSID] = useState('...');
    const [btEnabled, setBtEnabled] = useState(true);
    const [darkMode, setDarkMode] = useState(true);
    const [muted, setMuted] = useState(false);
    const [sinks, setSinks] = useState<AudioSink[]>([]);
    const [showSinks, setShowSinks] = useState(false);
    const [loading, setLoading] = useState(false);

    const refresh = useCallback(async () => {
        if (!isOpen) return;
        const stats = await SystemBridge.getSystemStats();
        setVolume(stats.volume);
        setBrightness(stats.brightness >= 0 ? stats.brightness : 80);
        setBattery(stats.battery);
        setIsCharging(stats.isCharging);
        setWifiSSID(stats.wifiSSID);
        setWifiEnabled(stats.wifiSSID !== 'Disconnected' && stats.wifiSSID !== '');

        // Load audio sinks
        const audioSinks = await SystemBridge.getAudioSinks();
        setSinks(audioSinks);
        const defaultSink = (audioSinks as AudioSink[]).find((s: AudioSink) => s.is_default);
        if (defaultSink) {
            setVolume(defaultSink.volume);
            setMuted(defaultSink.muted);
        }
    }, [isOpen]);

    useEffect(() => {
        refresh();
    }, [refresh]);

    const handleVolume = async (val: number) => {
        setVolume(val);
        await SystemBridge.setVolume(val);
    };

    const handleBrightness = async (val: number) => {
        setBrightness(val);
        await SystemBridge.setBrightness(val);
    };

    const handleToggleMute = async () => {
        const defaultSink = sinks.find(s => s.is_default);
        if (defaultSink) {
            await SystemBridge.toggleSinkMute(defaultSink.name);
            setMuted(!muted);
        }
    };

    const handleToggleWifi = async () => {
        const next = !wifiEnabled;
        setWifiEnabled(next);
        await SystemBridge.toggleWifi(next);
        if (!next) setWifiSSID('Disconnected');
    };

    const handleSinkSelect = async (sink: AudioSink) => {
        await SystemBridge.setDefaultSink(sink.name);
        setSinks(prev => prev.map(s => ({ ...s, is_default: s.name === sink.name })));
        setShowSinks(false);
    };

    const batteryColor = battery < 20 ? 'text-red-400' : battery < 50 ? 'text-yellow-400' : 'text-green-400';
    const defaultSink = sinks.find(s => s.is_default);

    if (!isOpen) return null;

    return (
        <div className="absolute top-14 right-4 w-80 bg-slate-900/98 border border-white/10 rounded-2xl shadow-2xl p-3 z-50 animate-in fade-in slide-in-from-top-3 duration-150 backdrop-blur-xl">

            {/* ── Row 1: Wi-Fi + Bluetooth + Media ── */}
            <div className="grid grid-cols-2 gap-2 mb-2">

                {/* Wi-Fi */}
                <button
                    onClick={() => onOpenSettings('wifi')}
                    className={`p-3 rounded-xl flex items-center gap-2 transition-all text-left group relative ${wifiEnabled ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}`}
                >
                    <div
                        className="p-1.5 rounded-full bg-white/20 shrink-0"
                        onClick={e => { e.stopPropagation(); handleToggleWifi(); }}
                    >
                        {wifiEnabled ? <Wifi size={14} /> : <WifiOff size={14} />}
                    </div>
                    <div className="min-w-0 flex-1">
                        <div className="text-xs font-bold leading-none">Wi-Fi</div>
                        <div className="text-[10px] opacity-70 truncate mt-0.5">
                            {wifiEnabled ? wifiSSID : 'Off'}
                        </div>
                    </div>
                    <ChevronRight size={12} className="opacity-0 group-hover:opacity-60 transition-opacity shrink-0" />
                </button>

                {/* Bluetooth */}
                <button
                    onClick={() => onOpenSettings('bluetooth')}
                    className={`p-3 rounded-xl flex items-center gap-2 transition-all text-left group ${btEnabled ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}`}
                >
                    <div
                        className="p-1.5 rounded-full bg-white/20 shrink-0"
                        onClick={e => { e.stopPropagation(); setBtEnabled(!btEnabled); }}
                    >
                        {btEnabled ? <Bluetooth size={14} /> : <BluetoothOff size={14} />}
                    </div>
                    <div className="min-w-0 flex-1">
                        <div className="text-xs font-bold leading-none">Bluetooth</div>
                        <div className="text-[10px] opacity-70 mt-0.5">{btEnabled ? 'On' : 'Off'}</div>
                    </div>
                    <ChevronRight size={12} className="opacity-0 group-hover:opacity-60 transition-opacity shrink-0" />
                </button>
            </div>

            {/* ── Row 2: Dark mode + Battery ── */}
            <div className="grid grid-cols-2 gap-2 mb-2">
                <button
                    onClick={() => setDarkMode(!darkMode)}
                    className={`p-3 rounded-xl flex items-center gap-2 transition-all ${darkMode ? 'bg-slate-700 text-white' : 'bg-amber-400/20 text-amber-300'}`}
                >
                    {darkMode ? <Moon size={16} /> : <Sun size={16} />}
                    <span className="text-xs font-bold">{darkMode ? 'Dark' : 'Light'}</span>
                </button>

                <div className="bg-slate-800 rounded-xl p-3">
                    <div className="flex items-center justify-between mb-0.5">
                        <span className="text-[10px] text-slate-400 font-medium">Battery</span>
                        {isCharging && <BatteryCharging size={12} className="text-green-400" />}
                    </div>
                    <div className={`text-xl font-bold ${batteryColor}`}>{Math.round(battery)}%</div>
                    <div className="text-[10px] text-slate-500 mt-0.5">
                        {isCharging ? 'Charging' : 'On battery'}
                    </div>
                </div>
            </div>

            {/* ── Audio output selector ── */}
            <div className="bg-slate-800 rounded-xl p-3 mb-2">
                <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                        <Speaker size={13} className="text-slate-400" />
                        <span className="text-xs font-medium text-slate-300">Output</span>
                    </div>
                    <button
                        onClick={() => setShowSinks(!showSinks)}
                        className="text-[10px] text-slate-500 hover:text-white transition-colors flex items-center gap-1"
                    >
                        {defaultSink ? defaultSink.description.slice(0, 20) : 'Default'}
                        <ChevronRight size={10} className={`transition-transform ${showSinks ? 'rotate-90' : ''}`} />
                    </button>
                </div>

                {showSinks && sinks.length > 0 && (
                    <div className="space-y-1 mb-2 border-t border-white/5 pt-2">
                        {sinks.map(sink => (
                            <button
                                key={sink.id}
                                onClick={() => handleSinkSelect(sink)}
                                className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left text-xs transition-colors ${sink.is_default ? 'bg-blue-600/30 text-blue-300' : 'hover:bg-white/5 text-slate-400'}`}
                            >
                                {sink.is_default && <Check size={10} />}
                                <span className="truncate">{sink.description}</span>
                            </button>
                        ))}
                    </div>
                )}

                {/* Volume slider */}
                <div className="flex items-center gap-2">
                    <button onClick={handleToggleMute} className="text-slate-400 hover:text-white transition-colors shrink-0">
                        {muted ? <VolumeX size={16} /> : <Volume2 size={16} />}
                    </button>
                    <input
                        type="range" min="0" max="100" value={muted ? 0 : volume}
                        onChange={e => handleVolume(parseInt(e.target.value))}
                        className="flex-1 h-1.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
                    />
                    <span className="text-[10px] text-slate-400 w-7 text-right">{muted ? 0 : volume}%</span>
                </div>
            </div>

            {/* ── Brightness (only if available) ── */}
            {brightness >= 0 && (
                <div className="flex items-center gap-2 px-3 py-2 bg-slate-800 rounded-xl mb-2">
                    <Sun size={14} className="text-slate-400 shrink-0" />
                    <input
                        type="range" min="5" max="100" value={brightness}
                        onChange={e => handleBrightness(parseInt(e.target.value))}
                        className="flex-1 h-1.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-yellow-400"
                    />
                    <span className="text-[10px] text-slate-400 w-7 text-right">{brightness}%</span>
                </div>
            )}

            {/* ── Footer ── */}
            <div className="flex items-center justify-between pt-1">
                <button
                    onClick={() => onOpenSettings()}
                    className="text-xs text-slate-500 hover:text-white transition-colors"
                >
                    All settings →
                </button>
                <button
                    onClick={refresh}
                    className="p-1.5 rounded-full hover:bg-white/10 text-slate-500 hover:text-white transition-colors"
                >
                    <RefreshCw size={12} />
                </button>
            </div>
        </div>
    );
};

export default ControlCenter;
