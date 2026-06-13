import React, { useState, useEffect } from 'react';
import {
    Monitor, Wifi, Bluetooth, Image as ImageIcon, Info, User,
    Printer, Users, Tv2, HardDrive, Keyboard, MousePointer,
    Palette, Check, RefreshCw, Lock, Unlock, Loader2, Sun, Moon,
    Battery, Cpu, Eye, EyeOff, Download, Upload, Plus,
    Trash2, Edit, Save, X, Grid, Layers, Droplet, Zap, Wind,
    PanelBottom, Globe, Package, GitBranch,
} from 'lucide-react';
import { AppProps, UserConfig, ThemeDefinition, PowerProfile } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import { useLanguage } from '../../contexts/LanguageContext';
import { configStore } from '../../utils/configStore';
import { MonitorsSection, PrintersSection, UsersSection } from '../SettingsSections';
import AboutApp from './AboutApp';

const BUILTIN_THEMES: Record<string, ThemeDefinition> = {
    'blue-default': { id: 'blue-default', name: 'Blue Glass', type: 'builtin', colors: { primary: '#0f172a', secondary: '#1e293b', text: '#f1f5f9', accent: '#2563eb' } },
    'cyberpunk': { id: 'cyberpunk', name: 'Cyberpunk', type: 'builtin', colors: { primary: '#09090b', secondary: '#18181b', text: '#e4e4e7', accent: '#eab308' } },
    'dracula': { id: 'dracula', name: 'Dracula', type: 'builtin', colors: { primary: '#282a36', secondary: '#44475a', text: '#f8f8f2', accent: '#bd93f9' } },
    'light-glass': { id: 'light-glass', name: 'Light Glass', type: 'builtin', colors: { primary: '#e2e8f0', secondary: '#ffffff', text: '#0f172a', accent: '#0ea5e9' } },
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
    const { t, language, setLanguage } = useLanguage();
    const [config, setConfig] = useState<UserConfig | null>(null);
    const [wallpapers, setWallpapers] = useState<string[]>([]);
    const [wallpaperPreviews, setWallpaperPreviews] = useState<Map<string, string>>(new Map());
    const [activeTab, setActiveTab] = useState('display');
    const [customThemes, setCustomThemes] = useState<ThemeDefinition[]>([]);
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
    const [networks, setNetworks] = useState<any[]>([]);
    const [btDevices, setBtDevices] = useState<any[]>([]);
    const [scanning, setScanning] = useState(false);
    const [connectingTo, setConnectingTo] = useState<string | null>(null);
    const [wifiEnabled, setWifiEnabled] = useState(true);
    const [btEnabled, setBtEnabled] = useState(true);
    const [showGoogleLogin, setShowGoogleLogin] = useState(false);

    useEffect(() => {
        configStore.init().then(cfg => {
            setConfig(cfg);
            setBrightness(80);
            setDisplayScale(cfg.displayScale || 1);
        });
        loadWallpapers();
        SystemBridge.getCustomThemes().then(setCustomThemes);
        loadPowerProfiles();
        refreshBattery();

        const interval = setInterval(refreshBattery, 30000);

        // Listen for tab changes from context menu
        const handler = (e: CustomEvent) => setActiveTab(e.detail);
        window.addEventListener('blue:settings-tab', handler as EventListener);

        return () => {
            clearInterval(interval);
            window.removeEventListener('blue:settings-tab', handler as EventListener);
        };
    }, []);

    const loadWallpapers = async () => {
        const wps = await SystemBridge.getWallpapers();
        setWallpapers(wps);
        const previews = new Map<string, string>();
        for (let i = 0; i < Math.min(wps.length, 12); i++) {
            const preview = await SystemBridge.getWallpaperPreview(wps[i]);
            if (preview) previews.set(wps[i], preview);
        }
        setWallpaperPreviews(new Map(previews));
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

    // THE KEY FIX: use configStore.update so wallpaper change propagates to desktop
    const handleSave = async (newConfig: Partial<UserConfig>) => {
        if (!config) return;
        await configStore.update(newConfig);
        const updated = { ...config, ...newConfig };
        setConfig(updated);
    };

    const applyTheme = (themeName: string) => {
        document.documentElement.setAttribute('data-theme', themeName);
        const custom = customThemes.find(t => t.id === themeName);
        if (custom?.css) {
            let style = document.getElementById('custom-theme-style');
            if (!style) { style = document.createElement('style'); style.id = 'custom-theme-style'; document.head.appendChild(style); }
            style.innerHTML = custom.css;
        } else {
            document.getElementById('custom-theme-style')?.remove();
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
        try { await SystemBridge.connectWifi(ssid, ''); await scanNetworks(); }
        catch { alert('Connection failed'); }
        finally { setConnectingTo(null); }
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
            colors: { primary: '#1a1a1a', secondary: '#2a2a2a', text: '#ffffff', accent: '#ff6600' }
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
        if (config?.themeName === id) handleSave({ themeName: 'blue-default' });
    };

    const handleAddGoogleAccount = async () => {
        const result = await SystemBridge.googleSignIn();
        if (result) {
            handleSave({ accounts: { ...config?.accounts, google: result.user } });
            setShowGoogleLogin(false);
        }
    };

    if (!config) return <div className="h-full flex items-center justify-center text-slate-400"><RefreshCw className="animate-spin mr-2" /> {t('settings.loading')}...</div>;

    const renderContent = () => {
        switch (activeTab) {
            case 'display': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.display')}</h2>

                    {/* Wallpaper */}
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-4 flex items-center gap-2">
                            <ImageIcon size={16} className="text-blue-400" /> {t('settings.wallpaper')}
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
                                        <img src={wallpaperPreviews.get(wp)} className="w-full h-full object-cover" alt={`Wallpaper ${idx}`} />
                                    ) : (
                                        <div className="w-full h-full bg-slate-700 flex items-center justify-center text-slate-500">
                                            <ImageIcon size={24} />
                                        </div>
                                    )}
                                    {config.wallpaper === wp && (
                                        <div className="absolute inset-0 flex items-center justify-center bg-blue-500/20">
                                            <Check size={24} className="text-white drop-shadow" />
                                        </div>
                                    )}
                                </div>
                            ))}
                        </div>
                        <button onClick={loadWallpapers} className="mt-3 text-xs text-slate-500 hover:text-white flex items-center gap-1 transition-colors">
                            <RefreshCw size={12} /> Refresh wallpapers
                        </button>
                    </div>

                    {/* Brightness */}
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-2">{t('settings.brightness')}</label>
                        <input type="range" min="0" max="100" value={brightness}
                            onChange={e => setBrightness(parseInt(e.target.value))}
                            onMouseUp={() => SystemBridge.setBrightness(brightness)}
                            className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
                        <div className="flex justify-between text-xs text-slate-500 mt-1"><span>0%</span><span>{brightness}%</span><span>100%</span></div>
                    </div>

                    {/* Display scale */}
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-2">{t('settings.display_scale')}</label>
                        <select value={config.displayScale} onChange={e => handleSave({ displayScale: parseFloat(e.target.value) })}
                            className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                            <option value="1">100%</option>
                            <option value="1.25">125%</option>
                            <option value="1.5">150%</option>
                            <option value="1.75">175%</option>
                            <option value="2">200%</option>
                        </select>
                    </div>

                    {/* Resolution & Refresh */}
                    <div className="grid grid-cols-2 gap-4">
                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                            <label className="block text-sm font-medium text-slate-400 mb-2">{t('settings.resolution')}</label>
                            <select value={resolution} onChange={e => setResolution(e.target.value)}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                                <option>1920x1080</option>
                                <option>2560x1440</option>
                                <option>3840x2160</option>
                                <option>1366x768</option>
                            </select>
                        </div>
                        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                            <label className="block text-sm font-medium text-slate-400 mb-2">{t('settings.refresh_rate')}</label>
                            <select value={refreshRate} onChange={e => setRefreshRate(parseInt(e.target.value))}
                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                                <option>60</option>
                                <option>75</option>
                                <option>120</option>
                                <option>144</option>
                            </select>
                        </div>
                    </div>
                </div>
            );

            case 'personalization': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.personalization')}</h2>

                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-4 flex items-center gap-2">
                            <Palette size={16} className="text-purple-400" /> {t('settings.builtin_themes')}
                        </label>
                        <div className="grid grid-cols-2 gap-4">
                            {Object.values(BUILTIN_THEMES).map(theme => (
                                <button key={theme.id} onClick={() => { handleSave({ themeName: theme.id }); applyTheme(theme.id); }}
                                    className={`flex items-center gap-4 p-4 rounded-xl border transition-all ${
                                        config.themeName === theme.id ? 'bg-blue-600/20 border-blue-500' : 'bg-slate-900 border-white/5 hover:bg-slate-700'
                                    }`}>
                                    <div className="w-12 h-12 rounded-lg shadow-lg" style={{ background: theme.colors?.primary }} />
                                    <div className="text-left">
                                        <div className="font-bold text-white">{theme.name}</div>
                                        <div className="text-xs text-slate-400">{t('settings.accent')}: {theme.colors?.accent}</div>
                                    </div>
                                    {config.themeName === theme.id && <Check size={20} className="ml-auto text-blue-400" />}
                                </button>
                            ))}
                        </div>
                    </div>

                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <div className="flex items-center justify-between mb-4">
                            <label className="text-sm font-medium text-slate-400 flex items-center gap-2">
                                <Droplet size={16} className="text-pink-400" /> {t('settings.custom_themes')}
                            </label>
                            <button onClick={() => setShowThemeEditor(true)}
                                className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm flex items-center gap-2 transition-colors">
                                <Plus size={14} /> {t('settings.add_theme')}
                            </button>
                        </div>
                        {customThemes.length === 0 ? (
                            <div className="text-center py-8 text-slate-500 text-sm">{t('settings.no_custom_themes')}</div>
                        ) : (
                            <div className="space-y-2">
                                {customThemes.map(theme => (
                                    <div key={theme.id} className={`flex items-center justify-between p-3 rounded-xl border ${
                                        config.themeName === theme.id ? 'bg-blue-600/20 border-blue-500' : 'bg-slate-900 border-white/5'
                                    }`}>
                                        <div className="flex items-center gap-3">
                                            <div className="w-8 h-8 rounded" style={{ background: theme.colors?.primary }} />
                                            <div>
                                                <div className="font-medium text-white">{theme.name}</div>
                                                <div className="text-xs text-slate-400">{t('settings.custom_theme')}</div>
                                            </div>
                                        </div>
                                        <div className="flex gap-1">
                                            <button onClick={() => { handleSave({ themeName: theme.id }); applyTheme(theme.id); }}
                                                className={`p-2 rounded-lg ${config.themeName === theme.id ? 'text-blue-400' : 'text-slate-400 hover:text-white hover:bg-white/5'}`}>
                                                <Check size={16} />
                                            </button>
                                            <button onClick={() => deleteCustomTheme(theme.id)} className="p-2 rounded-lg text-slate-400 hover:text-red-400 hover:bg-white/5">
                                                <Trash2 size={16} />
                                            </button>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>

                    {showThemeEditor && (
                        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                            <div className="bg-slate-800 rounded-2xl p-6 w-96 border border-white/10">
                                <h3 className="text-lg font-bold text-white mb-4">{t('settings.new_css_theme')}</h3>
                                <input type="text" placeholder={t('settings.theme_name')} value={newThemeName}
                                    onChange={e => setNewThemeName(e.target.value)}
                                    className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white mb-4 focus:outline-none" />
                                <textarea placeholder={`:root {\n  --bg-primary: #1a1a1a;\n  --accent: #ff6600;\n}`}
                                    value={newThemeCss} onChange={e => setNewThemeCss(e.target.value)}
                                    rows={8} className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white font-mono text-sm focus:outline-none" />
                                <div className="flex justify-end gap-2 mt-4">
                                    <button onClick={() => setShowThemeEditor(false)} className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors">{t('settings.cancel')}</button>
                                    <button onClick={saveCustomTheme} className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg flex items-center gap-2 transition-colors">
                                        <Save size={14} /> {t('settings.save')}
                                    </button>
                                </div>
                            </div>
                        </div>
                    )}
                </div>
            );

            case 'wifi': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <div className="flex items-center justify-between">
                        <h2 className="text-2xl font-bold text-white">{t('settings.wifi')}</h2>
                        <button onClick={scanNetworks} className={`p-2 bg-slate-800 rounded-full hover:bg-white/10 ${scanning ? 'animate-spin' : ''}`} disabled={scanning}>
                            <RefreshCw size={18} />
                        </button>
                    </div>
                    <div className="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
                        <span className="text-white">{t('settings.wifi')}</span>
                        <button onClick={async () => { const n = !wifiEnabled; setWifiEnabled(n); await SystemBridge.toggleWifi(n); if (n) scanNetworks(); }}
                            className={`w-12 h-6 rounded-full transition-colors relative ${wifiEnabled ? 'bg-blue-600' : 'bg-slate-600'}`}>
                            <div className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-transform ${wifiEnabled ? 'translate-x-7' : 'translate-x-1'}`} />
                        </button>
                    </div>
                    <div className="bg-slate-800 border border-white/5 rounded-2xl overflow-hidden">
                        {networks.length === 0 && !scanning && (
                            <div className="p-8 text-center text-slate-500">
                                <Wifi size={32} className="mx-auto mb-2 opacity-30" />
                                <p className="cursor-pointer hover:text-white" onClick={scanNetworks}>{t('settings.no_networks')} — click to scan</p>
                            </div>
                        )}
                        {scanning && <div className="p-4 text-center text-slate-500 flex items-center justify-center gap-2"><Loader2 size={16} className="animate-spin" /> Scanning...</div>}
                        {networks.map((net, i) => (
                            <div key={i} className={`flex items-center justify-between p-4 border-b border-white/5 last:border-0 ${net.in_use ? 'bg-blue-600/10' : 'hover:bg-white/5'}`}>
                                <div className="flex items-center gap-3">
                                    <Wifi size={20} className={net.signal > 60 ? "text-green-400" : "text-yellow-400"} />
                                    <div>
                                        <div className="font-medium text-white flex items-center gap-2">
                                            {net.ssid}
                                            {net.in_use && <span className="text-xs bg-green-500/20 text-green-400 px-2 rounded-full">{t('settings.connected')}</span>}
                                        </div>
                                        <div className="text-xs text-slate-400 flex items-center gap-1">
                                            {net.secure ? <Lock size={10} /> : <Unlock size={10} />}
                                            {net.secure ? t('settings.secured') : t('settings.open')} • {net.signal}% • {net.frequency}
                                        </div>
                                    </div>
                                </div>
                                {net.in_use ? (
                                    <button onClick={async () => { await SystemBridge.disconnectWifi(); scanNetworks(); }}
                                        className="px-4 py-2 bg-red-500/20 text-red-400 rounded-lg text-sm hover:bg-red-500/40 transition-colors">
                                        {t('settings.disconnect')}
                                    </button>
                                ) : (
                                    <button onClick={() => handleConnectWifi(net.ssid)} disabled={connectingTo !== null}
                                        className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm disabled:opacity-50 transition-colors">
                                        {connectingTo === net.ssid ? t('settings.connecting') : t('settings.connect')}
                                    </button>
                                )}
                            </div>
                        ))}
                    </div>
                </div>
            );

            case 'bluetooth': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <div className="flex items-center justify-between">
                        <h2 className="text-2xl font-bold text-white">{t('settings.bluetooth')}</h2>
                        <button onClick={scanBluetooth} className={`p-2 bg-slate-800 rounded-full hover:bg-white/10 ${scanning ? 'animate-spin' : ''}`}>
                            <RefreshCw size={18} />
                        </button>
                    </div>
                    <div className="bg-slate-800 p-4 rounded-xl flex items-center justify-between">
                        <span className="text-white">{t('settings.bluetooth')}</span>
                        <button onClick={() => setBtEnabled(!btEnabled)}
                            className={`w-12 h-6 rounded-full transition-colors relative ${btEnabled ? 'bg-blue-600' : 'bg-slate-600'}`}>
                            <div className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-transform ${btEnabled ? 'translate-x-7' : 'translate-x-1'}`} />
                        </button>
                    </div>
                    <div className="bg-slate-800 border border-white/5 rounded-2xl overflow-hidden">
                        {btDevices.length === 0 && !scanning && (
                            <div className="p-8 text-center text-slate-500">
                                <Bluetooth size={32} className="mx-auto mb-2 opacity-30" />
                                <p className="cursor-pointer hover:text-white" onClick={scanBluetooth}>{t('settings.no_devices')} — click to scan</p>
                            </div>
                        )}
                        {btDevices.map((dev, i) => (
                            <div key={i} className="flex items-center justify-between p-4 border-b border-white/5 last:border-0 hover:bg-white/5">
                                <div className="flex items-center gap-3">
                                    <Bluetooth size={20} className={dev.connected ? "text-blue-400" : "text-slate-500"} />
                                    <div>
                                        <div className="font-medium text-white">{dev.name}</div>
                                        <div className="text-xs text-slate-400">
                                            {dev.device_type} • {dev.connected ? t('settings.connected') : t('settings.disconnected')}
                                            {dev.battery != null && ` • ${t('settings.battery')}: ${dev.battery}%`}
                                        </div>
                                    </div>
                                </div>
                                <button onClick={() => handleToggleBt(dev.mac)}
                                    className={`px-4 py-2 rounded-lg text-sm transition-colors ${
                                        dev.connected ? 'bg-red-500/20 text-red-400 hover:bg-red-500/40' : 'bg-blue-600 hover:bg-blue-500 text-white'
                                    }`}>
                                    {dev.connected ? t('settings.disconnect') : t('settings.connect')}
                                </button>
                            </div>
                        ))}
                    </div>
                </div>
            );

            case 'power': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.power')}</h2>
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <div className="flex items-center gap-4">
                            <div className="p-4 bg-blue-600/20 rounded-full">
                                <Battery size={32} className={batteryStatus.percentage < 20 ? 'text-red-400' : 'text-green-400'} />
                            </div>
                            <div>
                                <div className="text-3xl font-bold text-white">{batteryStatus.percentage}%</div>
                                <div className="text-slate-400">{batteryStatus.charging ? t('settings.charging') : t('settings.on_battery')}</div>
                            </div>
                        </div>
                    </div>
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <h3 className="text-lg font-semibold text-white mb-4">{t('settings.power_profiles')}</h3>
                        <div className="space-y-2">
                            {powerProfiles.map(profile => (
                                <button key={profile.name} onClick={async () => { setPowerProfile(profile.name); await SystemBridge.setPowerProfile(profile.name); }}
                                    className={`w-full flex items-center justify-between p-4 rounded-xl border transition-all ${
                                        powerProfile === profile.name ? 'bg-blue-600/20 border-blue-500' : 'bg-slate-900 border-white/5 hover:bg-slate-700'
                                    }`}>
                                    <div className="flex items-center gap-3">
                                        {profile.icon === 'Battery' && <Battery size={20} />}
                                        {profile.icon === 'Wind' && <Wind size={20} />}
                                        {profile.icon === 'Zap' && <Zap size={20} />}
                                        <div className="text-left">
                                            <div className="font-medium text-white">
                                                {profile.name === 'power-saver' && t('settings.power_saver')}
                                                {profile.name === 'balanced' && t('settings.balanced')}
                                                {profile.name === 'performance' && t('settings.performance')}
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

            case 'panel': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.panel')}</h2>
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <div className="flex items-center justify-between mb-4">
                            <label className="text-sm font-medium text-slate-400">{t('settings.enable_panel')}</label>
                            <input type="checkbox" checked={config?.panelEnabled ?? true}
                                onChange={async e => { const v = e.target.checked; handleSave({ panelEnabled: v }); await SystemBridge.setPanelEnabled(v); }}
                                className="w-4 h-4 accent-blue-500" />
                        </div>
                        {config?.panelEnabled && (
                            <>
                                <div className="mb-4">
                                    <label className="block text-sm font-medium text-slate-400 mb-1">{t('settings.panel_position')}</label>
                                    <select value={config?.panelPosition ?? 'bottom'} onChange={e => handleSave({ panelPosition: e.target.value as any })}
                                        className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                                        <option value="top">{t('settings.top')}</option>
                                        <option value="bottom">{t('settings.bottom')}</option>
                                        <option value="left">{t('settings.left')}</option>
                                        <option value="right">{t('settings.right')}</option>
                                    </select>
                                </div>
                                <div className="mb-4">
                                    <label className="block text-sm font-medium text-slate-400 mb-1">{t('settings.panel_size')} (px)</label>
                                    <input type="number" value={config?.panelSize ?? 40} onChange={e => handleSave({ panelSize: parseInt(e.target.value) })}
                                        className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none" />
                                </div>
                                <div>
                                    <label className="block text-sm font-medium text-slate-400 mb-1">{t('settings.panel_opacity')}</label>
                                    <input type="range" min="0" max="100" value={config?.panelOpacity ? config.panelOpacity * 100 : 90}
                                        onChange={e => handleSave({ panelOpacity: parseInt(e.target.value) / 100 })}
                                        className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
                                    <div className="text-right text-xs text-slate-500">{Math.round((config?.panelOpacity ?? 0.9) * 100)}%</div>
                                </div>
                            </>
                        )}
                    </div>
                </div>
            );

            case 'language': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.language')}</h2>
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <label className="block text-sm font-medium text-slate-400 mb-2">{t('settings.select_language')}</label>
                        <select value={language} onChange={e => setLanguage(e.target.value as 'en' | 'pl')}
                            className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                            <option value="en">English</option>
                            <option value="pl">Polski</option>
                        </select>
                        <p className="text-xs text-slate-500 mt-2">{t('settings.language_restart_hint')}</p>
                    </div>
                </div>
            );

            case 'nightLight': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.night_light')}</h2>
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <div className="flex items-center justify-between mb-4">
                            <label className="text-sm font-medium text-slate-400">{t('settings.night_light')}</label>
                            <input type="checkbox" checked={config?.nightLightEnabled ?? false}
                                onChange={async e => { const v = e.target.checked; handleSave({ nightLightEnabled: v }); await SystemBridge.setNightLightEnabled(v); }}
                                className="w-4 h-4 accent-blue-500" />
                        </div>
                        {config?.nightLightEnabled && (
                            <>
                                <div className="mb-4">
                                    <label className="block text-sm font-medium text-slate-400 mb-1">{t('settings.night_light_temperature')} (K)</label>
                                    <input type="range" min="1000" max="10000" step="100" value={config?.nightLightTemperature ?? 4000}
                                        onChange={e => { const v = parseInt(e.target.value); handleSave({ nightLightTemperature: v }); SystemBridge.setNightLightTemperature(v); }}
                                        className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-orange-400" />
                                    <div className="flex justify-between text-xs text-slate-500 mt-1">
                                        <span>🕯️ 1000K</span>
                                        <span>{config?.nightLightTemperature ?? 4000}K</span>
                                        <span>☀️ 10000K</span>
                                    </div>
                                </div>
                                <div className="mb-4">
                                    <label className="block text-sm font-medium text-slate-400 mb-1">{t('settings.night_light_schedule')}</label>
                                    <select value={config?.nightLightSchedule ?? 'manual'} onChange={e => handleSave({ nightLightSchedule: e.target.value as any })}
                                        className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
                                        <option value="manual">{t('settings.manual')}</option>
                                        <option value="sunset">{t('settings.sunset')}</option>
                                    </select>
                                </div>
                                {config?.nightLightSchedule === 'manual' && (
                                    <div className="grid grid-cols-2 gap-4">
                                        <div>
                                            <label className="block text-sm font-medium text-slate-400 mb-1">{t('settings.start_hour')}</label>
                                            <input type="time" value={`${String(config?.nightLightStartHour ?? 20).padStart(2,'0')}:00`}
                                                onChange={e => { const [h] = e.target.value.split(':'); handleSave({ nightLightStartHour: parseInt(h) }); }}
                                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none" />
                                        </div>
                                        <div>
                                            <label className="block text-sm font-medium text-slate-400 mb-1">{t('settings.end_hour')}</label>
                                            <input type="time" value={`${String(config?.nightLightEndHour ?? 6).padStart(2,'0')}:00`}
                                                onChange={e => { const [h] = e.target.value.split(':'); handleSave({ nightLightEndHour: parseInt(h) }); }}
                                                className="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none" />
                                        </div>
                                    </div>
                                )}
                            </>
                        )}
                    </div>
                </div>
            );

            case 'apps': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.apps')}</h2>
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
                        {([['Blue AI', 'blueAI'], ['Blue Code', 'blueCode'], ['Blue Software', 'blueSoftware'], ['Mail', 'mail']] as const).map(([name, key]) => (
                            <div key={key} className="flex items-center justify-between py-2 border-b border-white/5 last:border-0">
                                <span className="text-white">{name}</span>
                                <input type="checkbox" checked={config?.appsEnabled?.[key] ?? true}
                                    onChange={e => handleSave({ appsEnabled: { ...config?.appsEnabled, [key]: e.target.checked } })}
                                    className="w-4 h-4 accent-blue-500" />
                            </div>
                        ))}
                    </div>
                </div>
            );

            case 'accounts': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">{t('settings.accounts')}</h2>
                    <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                        <div className="space-y-3">
                            <button onClick={() => setShowGoogleLogin(true)}
                                className="w-full flex items-center justify-center gap-2 bg-white text-slate-900 p-3 rounded-lg hover:bg-slate-100 transition-colors">
                                <img src="https://www.google.com/favicon.ico" className="w-5 h-5" alt="Google" /> Sign in with Google
                            </button>
                            <button onClick={() => alert('Apple Sign-In coming soon')}
                                className="w-full flex items-center justify-center gap-2 bg-black text-white p-3 rounded-lg hover:bg-slate-800 transition-colors border border-white/10">
                                 Sign in with Apple
                            </button>
                        </div>
                        {(config?.accounts?.google || config?.accounts?.apple) && (
                            <div className="mt-6 border-t border-white/10 pt-4">
                                <h3 className="text-sm font-medium text-slate-400 mb-3">Connected accounts</h3>
                                {config?.accounts?.google && (
                                    <div className="flex items-center justify-between py-2">
                                        <div className="flex items-center gap-2 text-sm text-slate-300">
                                            <img src="https://www.google.com/favicon.ico" className="w-4 h-4" alt="" />
                                            {config.accounts.google.email}
                                        </div>
                                        <button onClick={() => handleSave({ accounts: { ...config.accounts, google: undefined } })} className="text-xs text-red-400 hover:underline">Remove</button>
                                    </div>
                                )}
                            </div>
                        )}
                    </div>
                    {showGoogleLogin && (
                        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                            <div className="bg-slate-800 rounded-2xl p-6 w-96 border border-white/10">
                                <h3 className="text-lg font-bold text-white mb-4">Sign in with Google</h3>
                                <p className="text-sm text-slate-300 mb-4">You will be redirected to Google for authentication.</p>
                                <div className="flex justify-end gap-2">
                                    <button onClick={() => setShowGoogleLogin(false)} className="px-4 py-2 bg-slate-700 rounded-lg hover:bg-slate-600 transition-colors">Cancel</button>
                                    <button onClick={handleAddGoogleAccount} className="px-4 py-2 bg-blue-600 rounded-lg hover:bg-blue-500 transition-colors">Continue</button>
                                </div>
                            </div>
                        </div>
                    )}
                </div>
            );

            case 'monitors': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">Monitors</h2>
                    <MonitorsSection />
                </div>
            );
            case 'printers': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">Printers</h2>
                    <PrintersSection />
                </div>
            );
            case 'users': return (
                <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
                    <h2 className="text-2xl font-bold text-white">Users & Groups</h2>
                    <UsersSection />
                </div>
            );
            case 'about': return (
                <div className="animate-in fade-in slide-in-from-bottom-2 duration-300 -m-8 h-[calc(100%+4rem)]">
                    <AboutApp windowId="settings-about" />
                </div>
            );

            default: return null;
        }
    };

    return (
        <div className="flex h-full bg-slate-900 text-white">
            <div className="w-64 bg-slate-800/50 border-r border-white/5 p-4 flex flex-col gap-1">
                <h2 className="text-xl font-bold mb-6 px-2 flex items-center gap-2">
                    <div className="w-6 h-6 bg-blue-600 rounded flex items-center justify-center text-xs font-bold">B</div>
                    {t('settings.title')}
                </h2>
                <TabButton id="display" icon={Monitor} label={t('settings.display')} isActive={activeTab === 'display'} onClick={() => setActiveTab('display')} />
                <TabButton id="personalization" icon={Palette} label={t('settings.personalization')} isActive={activeTab === 'personalization'} onClick={() => setActiveTab('personalization')} />
                <TabButton id="wifi" icon={Wifi} label={t('settings.wifi')} isActive={activeTab === 'wifi'} onClick={() => { setActiveTab('wifi'); scanNetworks(); }} />
                <TabButton id="bluetooth" icon={Bluetooth} label={t('settings.bluetooth')} isActive={activeTab === 'bluetooth'} onClick={() => { setActiveTab('bluetooth'); scanBluetooth(); }} />
                <TabButton id="power" icon={Battery} label={t('settings.power')} isActive={activeTab === 'power'} onClick={() => setActiveTab('power')} />
                <TabButton id="panel" icon={PanelBottom} label={t('settings.panel')} isActive={activeTab === 'panel'} onClick={() => setActiveTab('panel')} />
                <TabButton id="language" icon={Globe} label={t('settings.language')} isActive={activeTab === 'language'} onClick={() => setActiveTab('language')} />
                <TabButton id="nightLight" icon={Moon} label={t('settings.night_light')} isActive={activeTab === 'nightLight'} onClick={() => setActiveTab('nightLight')} />
                <TabButton id="apps" icon={Package} label={t('settings.apps')} isActive={activeTab === 'apps'} onClick={() => setActiveTab('apps')} />
                <TabButton id="monitors" icon={Tv2} label="Monitors" isActive={activeTab === 'monitors'} onClick={() => setActiveTab('monitors')} />
                <TabButton id="printers" icon={Printer} label="Printers" isActive={activeTab === 'printers'} onClick={() => setActiveTab('printers')} />
                <TabButton id="users" icon={Users} label="Users" isActive={activeTab === 'users'} onClick={() => setActiveTab('users')} />
                <TabButton id="accounts" icon={User} label={t('settings.accounts')} isActive={activeTab === 'accounts'} onClick={() => setActiveTab('accounts')} />
                <TabButton id="about" icon={Info} label={t('settings.about')} isActive={activeTab === 'about'} onClick={() => setActiveTab('about')} />
            </div>
            <div className="flex-1 p-8 overflow-y-auto">
                {renderContent()}
            </div>
        </div>
    );
};

export default SettingsApp;
