import React, { useState, useEffect, useCallback } from 'react';
import {
    Image as ImageIcon, Palette, Wifi, Bluetooth, BatteryCharging, PanelTop,
    Globe, Moon, LayoutGrid, Monitor, Printer, Users, UserCircle, Info, Search,
} from 'lucide-react';

import { AppProps } from '../../../types';
import { SystemBridge, ThemeDefinition as SBThemeDefinition, UserConfig } from '../../../utils/systemBridge';
import { configStore } from '../../../utils/configStore';

import TabButton from './TabButton';
import type { SettingsTab } from './types';
import { getAvailableModes, getCurrentResolution, getCurrentRefreshRate } from './display_helpers';

import DisplaySection from './sections/DisplaySection';
import PersonalizationSection from './sections/PersonalizationSection';
import WifiSection from './sections/WifiSection';
import BluetoothSection from './sections/BluetoothSection';
import PowerSection from './sections/PowerSection';
import PanelSection from './sections/PanelSection';
import LanguageSection from './sections/LanguageSection';
import NightLightSection from './sections/NightLightSection';
import AppsSection from './sections/AppsSection';
import AccountsSection from './sections/AccountsSection';
import AboutSection from './sections/AboutSection';
import { MonitorsSection, PrintersSection, UsersSection } from '../../SettingsSections';

interface TabEntry {
    id: SettingsTab;
    label: string;
    icon: typeof Monitor;
    group: 'Appearance' | 'Network' | 'System' | 'Hardware' | 'Account';
}

const TABS: TabEntry[] = [
    { id: 'display',         label: 'Display',         icon: ImageIcon,    group: 'Appearance' },
    { id: 'personalization', label: 'Personalization',  icon: Palette,      group: 'Appearance' },
    { id: 'nightLight',      label: 'Night Light',      icon: Moon,         group: 'Appearance' },
    { id: 'panel',           label: 'Panel',            icon: PanelTop,     group: 'Appearance' },
    { id: 'language',        label: 'Language',         icon: Globe,        group: 'Appearance' },
    { id: 'wifi',            label: 'Wi-Fi',             icon: Wifi,         group: 'Network' },
    { id: 'bluetooth',       label: 'Bluetooth',        icon: Bluetooth,    group: 'Network' },
    { id: 'monitors',        label: 'Monitors',         icon: Monitor,      group: 'Hardware' },
    { id: 'printers',        label: 'Printers',         icon: Printer,      group: 'Hardware' },
    { id: 'power',           label: 'Power',            icon: BatteryCharging, group: 'Hardware' },
    { id: 'apps',            label: 'Applications',     icon: LayoutGrid,   group: 'System' },
    { id: 'users',           label: 'Users',            icon: Users,        group: 'System' },
    { id: 'accounts',        label: 'Accounts',         icon: UserCircle,   group: 'Account' },
    { id: 'about',           label: 'About',            icon: Info,         group: 'Account' },
];

const GROUP_ORDER: TabEntry['group'][] = ['Appearance', 'Network', 'Hardware', 'System', 'Account'];

const SettingsApp: React.FC<AppProps> = () => {
    const [activeTab, setActiveTab] = useState<SettingsTab>('display');
    const [query, setQuery] = useState('');

    const [config, setConfig] = useState<UserConfig | null>(null);
    const [customThemeCount, setCustomThemeCount] = useState(0);

    // Display-related state shared across the Display tab
    const [wallpapers, setWallpapers] = useState<string[]>([]);
    const [wallpaperPreviews, setWallpaperPreviews] = useState<Map<string, string>>(new Map());
    const [brightness, setBrightness] = useState(80);
    const [resolution, setResolution] = useState('1920x1080');
    const [refreshRate, setRefreshRate] = useState(60);
    const [modes, setModes] = useState<{ resolution: string; rates: number[] }[]>([
        { resolution: '1920x1080', rates: [60] },
    ]);

    // ── Load config (shared store also used by the desktop/taskbar) ───────
    useEffect(() => {
        configStore.init().then(setConfig);
        const unsub = configStore.subscribe(setConfig);
        return unsub;
    }, []);

    useEffect(() => {
        SystemBridge.getCustomThemes().then((t: SBThemeDefinition[]) => setCustomThemeCount(t.length));
    }, []);

    const onSave = useCallback(async (patch: Partial<UserConfig>) => {
        await configStore.save(patch);
    }, []);

    // ── Wallpapers ──────────────────────────────────────────────────────
    const loadWallpapers = useCallback(async () => {
        const list = await SystemBridge.getWallpapers();
        setWallpapers(list);
        const previews = new Map<string, string>();
        await Promise.all(list.map(async wp => {
            const data = await SystemBridge.getWallpaperPreview(wp);
            if (data) previews.set(wp, data);
        }));
        setWallpaperPreviews(previews);
    }, []);

    useEffect(() => { loadWallpapers(); }, [loadWallpapers]);

    // ── Display modes (resolution / refresh rate) ──────────────────────
    useEffect(() => {
        (async () => {
            const [avail, curRes, curRate] = await Promise.all([
                getAvailableModes(),
                getCurrentResolution(),
                getCurrentRefreshRate(),
            ]);
            setModes(avail);
            setResolution(curRes);
            setRefreshRate(curRate);
        })();
    }, []);

    const resolutionList = modes.map(m => m.resolution);
    const rateList = modes.find(m => m.resolution === resolution)?.rates ?? [60];

    if (!config) {
        return (
            <div className="flex h-full bg-slate-900 text-white items-center justify-center">
                <p className="text-slate-400 text-sm">Loading settings…</p>
            </div>
        );
    }

    const filteredTabs = query.trim()
        ? TABS.filter(t => t.label.toLowerCase().includes(query.trim().toLowerCase()))
        : TABS;

    return (
        <div className="flex h-full bg-slate-900 text-white overflow-hidden">
            {/* Sidebar */}
            <div className="w-56 shrink-0 bg-slate-950/60 border-r border-white/5 flex flex-col">
                <div className="p-3 pb-2">
                    <div className="relative">
                        <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
                        <input
                            value={query}
                            onChange={e => setQuery(e.target.value)}
                            placeholder="Find a setting…"
                            className="w-full bg-slate-800 border border-white/10 rounded-lg pl-7 pr-2 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/50"
                        />
                    </div>
                </div>
                <nav className="flex-1 overflow-y-auto px-2 pb-3 space-y-3">
                    {GROUP_ORDER.map(group => {
                        const groupTabs = filteredTabs.filter(t => t.group === group);
                        if (groupTabs.length === 0) return null;
                        return (
                            <div key={group}>
                                <div className="px-2.5 mb-1 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                                    {group}
                                </div>
                                <div className="space-y-0.5">
                                    {groupTabs.map(tab => (
                                        <TabButton
                                            key={tab.id}
                                            icon={tab.icon}
                                            label={tab.label}
                                            isActive={activeTab === tab.id}
                                            onClick={() => setActiveTab(tab.id)}
                                        />
                                    ))}
                                </div>
                            </div>
                        );
                    })}
                    {filteredTabs.length === 0 && (
                        <p className="text-xs text-slate-500 px-2.5 pt-2">No settings match "{query}".</p>
                    )}
                </nav>
                <div className="px-3 py-2 border-t border-white/5 text-[10px] text-slate-600">
                    {customThemeCount} custom theme{customThemeCount === 1 ? '' : 's'} installed
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-6">
                {activeTab === 'display' && (
                    <DisplaySection
                        config={config} onSave={onSave}
                        wallpapers={wallpapers} wallpaperPreviews={wallpaperPreviews}
                        onReloadWallpapers={loadWallpapers}
                        brightness={brightness} onBrightnessChange={setBrightness}
                        resolution={resolution} onResolutionChange={setResolution}
                        refreshRate={refreshRate} onRefreshRateChange={setRefreshRate}
                        resolutionList={resolutionList} rateList={rateList}
                    />
                )}
                {activeTab === 'personalization' && <PersonalizationSection />}
                {activeTab === 'nightLight' && <NightLightSection config={config} onSave={onSave} />}
                {activeTab === 'panel' && <PanelSection config={config} onSave={onSave} />}
                {activeTab === 'language' && <LanguageSection />}
                {activeTab === 'wifi' && <WifiSection />}
                {activeTab === 'bluetooth' && <BluetoothSection />}
                {activeTab === 'monitors' && <MonitorsSection />}
                {activeTab === 'printers' && <PrintersSection />}
                {activeTab === 'power' && <PowerSection />}
                {activeTab === 'apps' && <AppsSection config={config} onSave={onSave} />}
                {activeTab === 'users' && <UsersSection />}
                {activeTab === 'accounts' && <AccountsSection config={config} onSave={onSave} />}
                {activeTab === 'about' && <AboutSection />}
            </div>
        </div>
    );
};

export default SettingsApp;
