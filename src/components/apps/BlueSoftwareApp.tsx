import React, { useState, useEffect } from 'react';
import { AppProps } from '../../types';
import { useLanguage } from '../../contexts/LanguageContext';
import { Package, Download, Trash2, RefreshCw } from 'lucide-react';

interface SoftwareItem {
    id: string;
    name: string;
    description: string;
    version: string;
    installed: boolean;
}

const BlueSoftwareApp: React.FC<AppProps> = () => {
    const { t } = useLanguage();
    const [software, setSoftware] = useState<SoftwareItem[]>([
        { id: 'firefox', name: 'Firefox', description: 'Web browser', version: '120.0', installed: true },
        { id: 'libreoffice', name: 'LibreOffice', description: 'Office suite', version: '7.5', installed: false },
        { id: 'vlc', name: 'VLC', description: 'Media player', version: '3.0', installed: true },
        { id: 'gimp', name: 'GIMP', description: 'Image editor', version: '2.10', installed: false },
    ]);
    const [activeTab, setActiveTab] = useState<'installed' | 'available'>('installed');

    const handleInstall = async (item: SoftwareItem) => {
        // Mock installation
        setSoftware(prev => prev.map(p => p.id === item.id ? { ...p, installed: true } : p));
    };

    const handleUninstall = async (item: SoftwareItem) => {
        // Mock uninstallation
        setSoftware(prev => prev.map(p => p.id === item.id ? { ...p, installed: false } : p));
    };

    const filtered = software.filter(item => activeTab === 'installed' ? item.installed : !item.installed);

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
        <div className="h-12 bg-slate-800 border-b border-white/5 flex items-center px-4 gap-2">
        <button
        onClick={() => setActiveTab('installed')}
        className={`px-3 py-1 rounded-md ${activeTab === 'installed' ? 'bg-blue-600' : 'hover:bg-white/10'}`}
        >
        {t('software.installed')}
        </button>
        <button
        onClick={() => setActiveTab('available')}
        className={`px-3 py-1 rounded-md ${activeTab === 'available' ? 'bg-blue-600' : 'hover:bg-white/10'}`}
        >
        {t('software.available')}
        </button>
        <div className="flex-1" />
        <button className="p-2 hover:bg-white/10 rounded">
        <RefreshCw size={18} />
        </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {filtered.map(item => (
            <div key={item.id} className="bg-slate-800 rounded-xl p-4 flex items-center justify-between">
            <div>
            <div className="font-semibold">{item.name}</div>
            <div className="text-sm text-slate-400">{item.description}</div>
            <div className="text-xs text-slate-500">Version {item.version}</div>
            </div>
            {item.installed ? (
                <button
                onClick={() => handleUninstall(item)}
                className="px-4 py-2 bg-red-500/20 text-red-400 rounded-lg hover:bg-red-500/40"
                >
                {t('software.uninstall')}
                </button>
            ) : (
                <button
                onClick={() => handleInstall(item)}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg"
                >
                {t('software.install')}
                </button>
            )}
            </div>
        ))}
        </div>
        </div>
    );
};

export default BlueSoftwareApp;
