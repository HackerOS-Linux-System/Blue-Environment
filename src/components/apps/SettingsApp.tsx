import React, { useState, useEffect, useCallback } from 'react';
import {
    Monitor, Wifi, Bluetooth, Volume2, Image as ImageIcon, Info, User,
    Palette, Check, RefreshCw, Lock, Unlock, Loader2, Sun, Moon,
    Battery, Cpu, HardDrive, Eye, EyeOff, Download, Upload, Plus,
    Trash2, Edit, Save, X, Grid, Layers, Droplet, Zap, Wind
} from 'lucide-react';
import { AppProps, UserConfig, ThemeDefinition, PowerProfile } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';

// Domyślne motywy wbudowane
const BUILTIN_THEMES: Record<string, ThemeDefinition> = {
    'blue-default': {
        id: 'blue-default',
        name: 'Blue Glass',
        type: 'builtin',
        colors: { primary: '#0f172a', secondary: '#1e293b', text: '#f1f5f9', accent: '#2563eb' },
    },
    'cyberpunk': {
        id: 'cyberpunk',
        name: 'Cyberpunk',
        type: 'builtin',
        colors: { primary: '#09090b', secondary: '#18181b', text: '#e4e4e7', accent: '#eab308' },
    },
    'dracula': {
        id: 'dracula',
        name: 'Dracula',
        type: 'builtin',
        colors: { primary: '#282a36', secondary: '#44475a', text: '#f8f8f2', accent: '#bd93f9' },
    },
    'light-glass': {
        id: 'light-glass',
        name: 'Light Glass',
        type: 'builtin',
        colors: { primary: '#e2e8f0', secondary: '#ffffff', text: '#0f172a', accent: '#0ea5e9' },
    },
};

const TabButton = ({ id, icon: Icon, label, isActive, onClick }: any) => (
    <button
    onClick={onClick}
    className={`w-full flex items-center gap-3 p-2.5 rounded-lg text-sm font-medium transition-all duration-200 ${
        isActive
        ? 'bg-blue-600 text-white shadow-lg shadow-blue-500/20 translate-x-1'
        : 'text-slate-400 hover:bg-white/5 hover:text-white'
    }`}
    >
    <Icon size={18} /> {label}
    </button>
);

const SettingsApp: React.FC<AppProps> = () => {
    const [config, setConfig] = useState<UserConfig | null>(null);
    const [wallpapers, setWallpapers] = useState<string[]>([]);
    const [wallpaperPreviews, setWallpaperPreviews] = useState<Map<string, string>>(new Map());
    const [activeTab, setActiveTab] = useState('display');
    const [customThemes, setCustomThemes] = useState<ThemeDefinition[]>([]);
    const [editingTheme, setEditingTheme] = useState<ThemeDefinition | null>(null);
    const [newThemeName, setNewThemeName] = useState('');
    const [newThemeCss, setNewThemeCss] = useState('');
    const [showThemeEditor, setShowThemeEditor] = useState(false);
    const [brightness, setBrightness] = useState(80);
    const [displayScale, setDisplayScale] = useState(1);
    const [refreshRate, setRefreshRate] = useState(60);
    const [resolution, setResolution] = useState('1920x1080');
    const [powerProfile, setPowerProfile] = useState('balanced');
    const [batteryStatus, setBatteryStatus] = useState({ percentage: 85, charging: false });
    const [powerProfiles, setPowerProfiles] = useState<PowerProfile[]>([]);

    // Network State
    const [networks, setNetworks] = useState<any[]>([]);
    const [btDevices, setBtDevices] = useState<any[]>([]);
    const [scanning, setScanning] = useState(false);
    const [connectingTo, setConnectingTo] = useState<string | null>(null);
    const [wifiEnabled, setWifiEnabled] = useState(true);
    const [btEnabled, setBtEnabled] = useState(true);

    useEffect(() => {
        SystemBridge.loadConfig().then(setConfig);
        loadWallpapers();
        loadCustomThemes();
        loadPowerProfiles();
        refreshBattery();

        const interval = setInterval(refreshBattery, 30000);
        return () => clearInterval(interval);
    }, []);

    const loadWallpapers = async () => {
        const wps = await SystemBridge.getWallpapers();
        setWallpapers(wps);
        // Wczytaj podglądy (pierwsze 6)
        const previews = new Map();
        for (let i = 0; i < Math.min(wps.length, 6); i++) {
            const preview = await SystemBridge.getWallpaperPreview(wps[i]);
            if (preview) previews.set(wps[i], preview);
        }
        setWallpaperPreviews(previews);
    };

    const loadCustomThemes = async () => {
        const themes = await SystemBridge.getCustomThemes();
        setCustomThemes(themes);
    };

    const loadPowerProfiles = async () => {
        const profiles = await SystemBridge.getPowerProfiles();
        setPowerProfiles(profiles);
        const active = profiles.find(p => p.active);
        if (active) setPowerProfile(active.name);
    };

        const refreshBattery = async () => {
            const stats = await SystemBridge.getSystemStats();
            setBatteryStatus({ percentage: stats.battery, charging: stats.isCharging });
        };

        const handleSave = (newConfig: Partial<UserConfig>) => {
            if (!config) return;
            const updated = { ...config, ...newConfig };
            setConfig(updated);
            SystemBridge.saveConfig(updated);
            // Jeśli zmieniamy motyw, aplikujemy go od razu
            if (newConfig.themeName) {
                applyTheme(newConfig.themeName);
            }
        };

        const applyTheme = (themeName: string) => {
            document.documentElement.setAttribute('data-theme', themeName);
            // Jeśli to motyw własny, wczytaj jego CSS
            const custom = customThemes.find(t => t.id === themeName);
            if (custom?.css) {
                const style = document.getElementById('custom-theme-style') || document.createElement('style');
                style.id = 'custom-theme-style';
                style.innerHTML = custom.css;
                document.head.appendChild(style);
            } else {
                const style = document.getElementById('custom-theme-style');
                if (style) style.remove();
            }
        };

        const scanNetworks = async () => {
            setScanning(true);
            const nets = await SystemBridge.getWifiNetworks();
            setNetworks(nets);
            setScanning(false);
        };

        const handleConnectWifi = async (ssid: string) => {
            setConnectingTo(ssid);
            try {
                await SystemBridge.connectWifi(ssid, "password");
                await scanNetworks();
            } catch (e) {
                alert("Connection failed");
            } finally {
                setConnectingTo(null);
            }
        };

        const scanBluetooth = async () => {
            setScanning(true);
            const devs = await SystemBridge.getBluetoothDevices();
            setBtDevices(devs);
            setScanning(false);
        };

        const handleToggleBt = async (mac: string) => {
            await SystemBridge.toggleBluetoothDevice(mac);
            scanBluetooth();
        };

        const saveCustomTheme = async () => {
            if (!newThemeName.trim()) return;
            const newTheme: ThemeDefinition = {
                id: `custom-${Date.now()}`,
                name: newThemeName,
                type: 'custom',
                css: newThemeCss,
                colors: {
                    primary: '#1a1a1a',
                    secondary: '#2a2a2a',
                    text: '#ffffff',
                    accent: '#ff6600',
                }
            };
            await SystemBridge.saveCustomTheme(newTheme);
            setCustomThemes([...customThemes, newTheme]);
            setShowThemeEditor(false);
            setNewThemeName('');
            setNewThemeCss('');
        };

        const deleteCustomTheme = async (id: string) => {
            await SystemBridge.deleteCustomTheme(id);
            setCustomThemes(customThemes.filter(t => t.id !== id));
            if (config?.themeName === id) {
                handleSave({ themeName: 'blue-default' });
            }
        };

        if (!config) return <div className="h-full flex items-center justify-center text-slate-400"><RefreshCw className="animate-spin mr-2" /> Loading system config...</div>;

        const renderContent = () => {
            switch(activeTab) {
                case 'display':
                    return (
                        <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                        <h2 className="text-2xl font-bold text-white">Ekran i wygląd</h2>

                        {/* Wallpaper Section */}
                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-4 flex items-center gap-2">
                        <ImageIcon size={16} className="text-blue-400" /> Tapeta
                        </label>
                        <div className="grid grid-cols-3 gap-4 max-h-80 overflow-y-auto custom-scrollbar p-1">
                        {wallpapers.map((wp, idx) => (
                            <div
                            key={idx}
                            className={`relative aspect-video rounded-xl overflow-hidden cursor-pointer border-2 transition-all hover:scale-105 ${
                                config.wallpaper === wp ? 'border-blue-500 ring-2 ring-blue-500/20' : 'border-transparent'
                            }`}
                            onClick={() => handleSave({ wallpaper: wp })}
                            >
                            {wallpaperPreviews.has(wp) ? (
                                <img src={wallpaperPreviews.get(wp)} className="w-full h-full object-cover" alt={`Tapeta ${idx}`} />
                            ) : (
                                <div className="w-full h-full bg-slate-700 flex items-center justify-center text-slate-500">
                                <ImageIcon size={24} />
                                </div>
                            )}
                            </div>
                        ))}
                        </div>
                        </div>

                        {/* Jasność */}
                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-2">Jasność</label>
                        <input
                        type="range"
                        min="0"
                        max="100"
                        value={brightness}
                        onChange={(e) => setBrightness(parseInt(e.target.value))}
                        onMouseUp={() => SystemBridge.setBrightness(brightness)}
                        className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
                        />
                        <div className="flex justify-between text-xs text-slate-500 mt-1">
                        <span>0%</span>
                        <span>{brightness}%</span>
                        <span>100%</span>
                        </div>
                        </div>

                        {/* Skalowanie wyświetlacza */}
                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-2">Skalowanie</label>
                        <select
                        value={displayScale}
                        onChange={(e) => handleSave({ displayScale: parseFloat(e.target.value) })}
                        className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white"
                        >
                        <option value="1">100%</option>
                        <option value="1.25">125%</option>
                        <option value="1.5">150%</option>
                        <option value="1.75">175%</option>
                        <option value="2">200%</option>
                        </select>
                        </div>

                        {/* Rozdzielczość i odświeżanie */}
                        <div className="grid grid-cols-2 gap-4">
                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-2">Rozdzielczość</label>
                        <select
                        value={resolution}
                        onChange={(e) => setResolution(e.target.value)}
                        className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white"
                        >
                        <option>1920x1080</option>
                        <option>2560x1440</option>
                        <option>3840x2160</option>
                        <option>1366x768</option>
                        </select>
                        </div>
                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-2">Częstotliwość</label>
                        <select
                        value={refreshRate}
                        onChange={(e) => setRefreshRate(parseInt(e.target.value))}
                        className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white"
                        >
                        <option>60</option>
                        <option>75</option>
                        <option>120</option>
                        <option>144</option>
                        </select>
                        </div>
                        </div>
                        </div>
                    );

                    case 'personalization':
                        return (
                            <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                            <h2 className="text-2xl font-bold text-white">Personalizacja</h2>

                            {/* Motywy wbudowane */}
                            <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                            <label className="block text-sm font-medium text-slate-400 mb-4 flex items-center gap-2">
                            <Palette size={16} className="text-purple-400" /> Motywy wbudowane
                            </label>
                            <div className="grid grid-cols-2 gap-4">
                            {Object.values(BUILTIN_THEMES).map(theme => (
                                <button
                                key={theme.id}
                                onClick={() => handleSave({ themeName: theme.id })}
                                className={`flex items-center gap-4 p-4 rounded-xl border transition-all ${
                                    config.themeName === theme.id ? 'bg-blue-600/20 border-blue-500' : 'bg-slate-900 border-white/5 hover:bg-slate-700'
                                }`}
                                >
                                <div
                                className="w-12 h-12 rounded-lg shadow-lg"
                                style={{ background: theme.colors?.primary }}
                                />
                                <div className="text-left">
                                <div className="font-bold text-white">{theme.name}</div>
                                <div className="text-xs text-slate-400">Akcent: {theme.colors?.accent}</div>
                                </div>
                                {config.themeName === theme.id && <Check size={20} className="ml-auto text-blue-400" />}
                                </button>
                            ))}
                            </div>
                            </div>

                            {/* Motywy własne */}
                            <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                            <div className="flex items-center justify-between mb-4">
                            <label className="text-sm font-medium text-slate-400 flex items-center gap-2">
                            <Droplet size={16} className="text-pink-400" /> Motywy własne
                            </label>
                            <button
                            onClick={() => setShowThemeEditor(true)}
                            className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm flex items-center gap-2"
                            >
                            <Plus size={14} /> Dodaj motyw
                            </button>
                            </div>

                            {customThemes.length === 0 ? (
                                <div className="text-center py-8 text-slate-500 text-sm">
                                Brak własnych motywów. Kliknij "Dodaj motyw", aby utworzyć nowy.
                                </div>
                            ) : (
                                <div className="space-y-2">
                                {customThemes.map(theme => (
                                    <div
                                    key={theme.id}
                                    className={`flex items-center justify-between p-3 rounded-xl border ${
                                        config.themeName === theme.id ? 'bg-blue-600/20 border-blue-500' : 'bg-slate-900 border-white/5'
                                    }`}
                                    >
                                    <div className="flex items-center gap-3">
                                    <div
                                    className="w-8 h-8 rounded"
                                    style={{ background: theme.colors?.primary }}
                                    />
                                    <div>
                                    <div className="font-medium text-white">{theme.name}</div>
                                    <div className="text-xs text-slate-400">Motyw własny</div>
                                    </div>
                                    </div>
                                    <div className="flex gap-1">
                                    <button
                                    onClick={() => handleSave({ themeName: theme.id })}
                                    className={`p-2 rounded-lg ${
                                        config.themeName === theme.id ? 'text-blue-400' : 'text-slate-400 hover:text-white hover:bg-white/5'
                                    }`}
                                    title="Zastosuj"
                                    >
                                    <Check size={16} />
                                    </button>
                                    <button
                                    onClick={() => deleteCustomTheme(theme.id)}
                                    className="p-2 rounded-lg text-slate-400 hover:text-red-400 hover:bg-white/5"
                                    title="Usuń"
                                    >
                                    <Trash2 size={16} />
                                    </button>
                                    </div>
                                    </div>
                                ))}
                                </div>
                            )}
                            </div>

                            {/* Edytor motywu */}
                            {showThemeEditor && (
                                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                                <div className="bg-slate-800 rounded-2xl p-6 w-96 border border-white/10">
                                <h3 className="text-lg font-bold text-white mb-4">Nowy motyw CSS</h3>
                                <input
                                type="text"
                                placeholder="Nazwa motywu"
                                value={newThemeName}
                                onChange={(e) => setNewThemeName(e.target.value)}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white mb-4"
                                />
                                <textarea
                                placeholder="Wpisz kod CSS (np. :root { --bg-primary: #...; })"
                                value={newThemeCss}
                                onChange={(e) => setNewThemeCss(e.target.value)}
                                rows={8}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white font-mono text-sm"
                                />
                                <div className="flex justify-end gap-2 mt-4">
                                <button
                                onClick={() => setShowThemeEditor(false)}
                                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg"
                                >
                                Anuluj
                                </button>
                                <button
                                onClick={saveCustomTheme}
                                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg flex items-center gap-2"
                                >
                                <Save size={14} /> Zapisz
                                </button>
                                </div>
                                </div>
                                </div>
                            )}
                            </div>
                        );

                        case 'wifi':
                            return (
                                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                                <div className="flex items-center justify-between">
                                <h2 className="text-2xl font-bold text-white">Wi-Fi</h2>
                                <button
                                onClick={scanNetworks}
                                className={`p-2 bg-slate-800 rounded-full hover:bg-white/10 ${scanning ? 'animate-spin' : ''}`}
                                disabled={scanning}
                                >
                                <RefreshCw size={18} />
                                </button>
                                </div>

                                {/* Włącznik Wi-Fi */}
                                <div className="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
                                <span className="text-white">Wi-Fi</span>
                                <button
                                onClick={async () => {
                                    const newState = !wifiEnabled;
                                    setWifiEnabled(newState);
                                    await SystemBridge.toggleWifi(newState);
                                    if (newState) scanNetworks();
                                }}
                                className={`w-12 h-6 rounded-full transition-colors ${
                                    wifiEnabled ? 'bg-blue-600' : 'bg-slate-600'
                                }`}
                                >
                                <div className={`w-4 h-4 rounded-full bg-white transform transition-transform ${
                                    wifiEnabled ? 'translate-x-7' : 'translate-x-1'
                                }`} />
                                </button>
                                </div>

                                {/* Lista sieci */}
                                <div className="bg-slate-800 border border-white/5 rounded-2xl overflow-hidden">
                                {networks.length === 0 && !scanning && (
                                    <div className="p-8 text-center text-slate-500">Brak sieci</div>
                                )}
                                {networks.map((net, i) => (
                                    <div key={i} className={`flex items-center justify-between p-4 border-b border-white/5 last:border-0 ${
                                        net.in_use ? 'bg-blue-600/10' : 'hover:bg-white/5'
                                    }`}>
                                    <div className="flex items-center gap-3">
                                    <Wifi size={20} className={net.signal > 60 ? "text-green-400" : "text-yellow-400"} />
                                    <div>
                                    <div className="font-medium text-white flex items-center gap-2">
                                    {net.ssid}
                                    {net.in_use && <span className="text-xs bg-green-500/20 text-green-400 px-2 rounded-full">Połączono</span>}
                                    </div>
                                    <div className="text-xs text-slate-400 flex items-center gap-1">
                                    {net.secure ? <Lock size={10} /> : <Unlock size={10} />}
                                    {net.secure ? "Zabezpieczona" : "Otwarta"} • {net.signal}% • {net.frequency}
                                    </div>
                                    </div>
                                    </div>
                                    {net.in_use ? (
                                        <button
                                        onClick={async () => {
                                            await SystemBridge.disconnectWifi();
                                            scanNetworks();
                                        }}
                                        className="px-4 py-2 bg-red-500/20 text-red-400 rounded-lg text-sm hover:bg-red-500/40"
                                        >
                                        Rozłącz
                                        </button>
                                    ) : (
                                        <button
                                        onClick={() => handleConnectWifi(net.ssid)}
                                        disabled={connectingTo !== null}
                                        className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm disabled:opacity-50"
                                        >
                                        {connectingTo === net.ssid ? 'Łączenie...' : 'Połącz'}
                                        </button>
                                    )}
                                    </div>
                                ))}
                                </div>
                                </div>
                            );

                            case 'bluetooth':
                                return (
                                    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                                    <div className="flex items-center justify-between">
                                    <h2 className="text-2xl font-bold text-white">Bluetooth</h2>
                                    <button
                                    onClick={scanBluetooth}
                                    className={`p-2 bg-slate-800 rounded-full hover:bg-white/10 ${scanning ? 'animate-spin' : ''}`}
                                    >
                                    <RefreshCw size={18} />
                                    </button>
                                    </div>

                                    {/* Włącznik Bluetooth */}
                                    <div className="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
                                    <span className="text-white">Bluetooth</span>
                                    <button
                                    onClick={() => setBtEnabled(!btEnabled)}
                                    className={`w-12 h-6 rounded-full transition-colors ${
                                        btEnabled ? 'bg-blue-600' : 'bg-slate-600'
                                    }`}
                                    >
                                    <div className={`w-4 h-4 rounded-full bg-white transform transition-transform ${
                                        btEnabled ? 'translate-x-7' : 'translate-x-1'
                                    }`} />
                                    </button>
                                    </div>

                                    {/* Lista urządzeń */}
                                    <div className="bg-slate-800 border border-white/5 rounded-2xl overflow-hidden">
                                    {btDevices.length === 0 && !scanning && (
                                        <div className="p-8 text-center text-slate-500">Brak urządzeń</div>
                                    )}
                                    {btDevices.map((dev, i) => (
                                        <div key={i} className="flex items-center justify-between p-4 border-b border-white/5 last:border-0 hover:bg-white/5">
                                        <div className="flex items-center gap-3">
                                        <Bluetooth size={20} className={dev.connected ? "text-blue-400" : "text-slate-500"} />
                                        <div>
                                        <div className="font-medium text-white">{dev.name}</div>
                                        <div className="text-xs text-slate-400">
                                        {dev.device_type} • {dev.connected ? 'Połączono' : 'Rozłączono'}
                                        {dev.battery && ` • Bateria: ${dev.battery}%`}
                                        </div>
                                        </div>
                                        </div>
                                        <button
                                        onClick={() => handleToggleBt(dev.mac)}
                                        className={`px-4 py-2 rounded-lg text-sm ${
                                            dev.connected
                                            ? 'bg-red-500/20 text-red-400 hover:bg-red-500/40'
                                            : 'bg-blue-600 hover:bg-blue-500 text-white'
                                        }`}
                                        >
                                        {dev.connected ? 'Rozłącz' : 'Połącz'}
                                        </button>
                                        </div>
                                    ))}
                                    </div>
                                    </div>
                                );

                                case 'power':
                                    return (
                                        <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                                        <h2 className="text-2xl font-bold text-white">Zasilanie</h2>

                                        {/* Status baterii */}
                                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                                        <div className="flex items-center gap-4">
                                        <div className="p-4 bg-blue-600/20 rounded-full">
                                        <Battery size={32} className={batteryStatus.percentage < 20 ? 'text-red-400' : 'text-green-400'} />
                                        </div>
                                        <div>
                                        <div className="text-3xl font-bold text-white">{batteryStatus.percentage}%</div>
                                        <div className="text-slate-400">
                                        {batteryStatus.charging ? 'Ładowanie' : 'Na baterii'}
                                        </div>
                                        </div>
                                        </div>
                                        </div>

                                        {/* Profile zasilania */}
                                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                                        <h3 className="text-lg font-semibold text-white mb-4">Profile zasilania</h3>
                                        <div className="space-y-2">
                                        {powerProfiles.map(profile => (
                                            <button
                                            key={profile.name}
                                            onClick={async () => {
                                                setPowerProfile(profile.name);
                                                await SystemBridge.setPowerProfile(profile.name);
                                            }}
                                            className={`w-full flex items-center justify-between p-4 rounded-xl border transition-all ${
                                                powerProfile === profile.name
                                                ? 'bg-blue-600/20 border-blue-500'
                                                : 'bg-slate-900 border-white/5 hover:bg-slate-700'
                                            }`}
                                            >
                                            <div className="flex items-center gap-3">
                                            {profile.icon === 'Battery' && <Battery size={20} />}
                                            {profile.icon === 'Wind' && <Wind size={20} />}
                                            {profile.icon === 'Zap' && <Zap size={20} />}
                                            <div className="text-left">
                                            <div className="font-medium text-white">
                                            {profile.name === 'power-saver' && 'Oszczędzanie energii'}
                                            {profile.name === 'balanced' && 'Zrównoważony'}
                                            {profile.name === 'performance' && 'Wydajność'}
                                            </div>
                                            <div className="text-xs text-slate-400">{profile.description}</div>
                                            </div>
                                            </div>
                                            {powerProfile === profile.name && <Check size={20} className="text-blue-400" />}
                                            </button>
                                        ))}
                                        </div>
                                        </div>
                                        </div>
                                    );

                                    case 'about':
                                        return (
                                            <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                                            <h2 className="text-2xl font-bold text-white">O systemie</h2>
                                            <div className="bg-gradient-to-br from-blue-900/50 to-slate-900 p-8 rounded-3xl border border-white/5 flex items-center gap-6">
                                            <div className="w-24 h-24 bg-blue-600 rounded-full flex items-center justify-center shadow-2xl">
                                            <span className="text-4xl font-bold text-white">B</span>
                                            </div>
                                            <div>
                                            <h3 className="text-2xl font-bold text-white">Blue Environment</h3>
                                            <p className="text-blue-200">Wersja 1.2.0 (Stable)</p>
                                            <p className="text-slate-400 text-sm mt-2">System operacyjny: HackerOS Linux</p>
                                            <p className="text-slate-400 text-xs mt-4">© 2026 HackerOS Team</p>
                                            </div>
                                            </div>
                                            </div>
                                        );

                                    default:
                                        return null;
            }
        };

        return (
            <div className="flex h-full bg-slate-900 text-white">
            {/* Lewy pasek nawigacji */}
            <div className="w-64 bg-slate-800/50 border-r border-white/5 p-4 flex flex-col gap-1">
            <h2 className="text-xl font-bold mb-6 px-2 flex items-center gap-2">
            <div className="w-6 h-6 bg-blue-600 rounded flex items-center justify-center text-xs">B</div>
            Ustawienia
            </h2>
            <TabButton
            id="display"
            icon={Monitor}
            label="Ekran"
            isActive={activeTab === 'display'}
            onClick={() => setActiveTab('display')}
            />
            <TabButton
            id="personalization"
            icon={Palette}
            label="Personalizacja"
            isActive={activeTab === 'personalization'}
            onClick={() => setActiveTab('personalization')}
            />
            <TabButton
            id="wifi"
            icon={Wifi}
            label="Wi-Fi"
            isActive={activeTab === 'wifi'}
            onClick={() => { setActiveTab('wifi'); scanNetworks(); }}
            />
            <TabButton
            id="bluetooth"
            icon={Bluetooth}
            label="Bluetooth"
            isActive={activeTab === 'bluetooth'}
            onClick={() => { setActiveTab('bluetooth'); scanBluetooth(); }}
            />
            <TabButton
            id="power"
            icon={Battery}
            label="Zasilanie"
            isActive={activeTab === 'power'}
            onClick={() => setActiveTab('power')}
            />
            <TabButton
            id="about"
            icon={Info}
            label="O systemie"
            isActive={activeTab === 'about'}
            onClick={() => setActiveTab('about')}
            />
            </div>

            {/* Główna zawartość */}
            <div className="flex-1 p-8 overflow-y-auto">
            {renderContent()}
            </div>
            </div>
        );
};

export default SettingsApp;
