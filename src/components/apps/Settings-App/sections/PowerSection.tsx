import React, { useState, useEffect } from 'react';
import { Battery, Zap, Wind, Check } from 'lucide-react';
import { SystemBridge } from '../../../../utils/systemBridge';
import type { PowerProfile } from '../../../../types';

const PowerSection: React.FC = () => {
    const [profile, setProfile] = useState('balanced');
    const [profiles, setProfiles] = useState<PowerProfile[]>([]);
    const [battery, setBattery] = useState({ percentage: 100, charging: false });

    useEffect(() => {
        SystemBridge.getPowerProfiles().then(setProfiles);
        const refresh = () => SystemBridge.getSystemStats().then((s: any) =>
            setBattery({ percentage: s.battery ?? 100, charging: s.isCharging ?? false }));
        refresh();
        const id = setInterval(refresh, 30000);
        return () => clearInterval(id);
    }, []);

    const IconFor = (icon: string) => {
        if (icon === 'Zap') return Zap;
        if (icon === 'Wind') return Wind;
        return Battery;
    };

    return (
        <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
            <h2 className="text-2xl font-bold text-white">Power</h2>
            <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                <div className="flex items-center gap-4">
                    <div className="p-4 bg-blue-600/20 rounded-full">
                        <Battery size={32} className={battery.percentage < 20 ? 'text-red-400' : 'text-green-400'} />
                    </div>
                    <div>
                        <div className="text-3xl font-bold text-white">{battery.percentage}%</div>
                        <div className="text-slate-400">{battery.charging ? 'Charging' : 'On battery'}</div>
                    </div>
                </div>
            </div>
            <div className="bg-slate-800 p-6 rounded-2xl border border-white/5">
                <h3 className="text-lg font-semibold text-white mb-4">Power Profiles</h3>
                <div className="space-y-2">
                    {profiles.map((p: PowerProfile) => {
                        const Icon = IconFor(p.icon ?? 'Battery');
                        return (
                            <button key={p.name}
                                onClick={async () => { setProfile(p.name); await SystemBridge.setPowerProfile(p.name); }}
                                className={`w-full flex items-center justify-between p-4 rounded-xl border transition-all
                                    ${profile === p.name ? 'bg-blue-600/20 border-blue-500' : 'bg-slate-900 border-white/5 hover:bg-slate-700'}`}>
                                <div className="flex items-center gap-3">
                                    <Icon size={20} />
                                    <div className="text-left">
                                        <div className="font-medium text-white">{p.name}</div>
                                        <div className="text-xs text-slate-400">{p.description}</div>
                                    </div>
                                </div>
                                {profile === p.name && <Check size={20} className="text-blue-400" />}
                            </button>
                        );
                    })}
                </div>
            </div>
        </div>
    );
};
export default PowerSection;
