import React, { useEffect, useState } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';

const AboutApp: React.FC<AppProps> = () => {
    const [info, setInfo] = useState<any>({ Name: 'HackerOS', Version: '0.4.0', Copyright: '© 2026 HackerOS Team', Kernel: '...' });
    useEffect(() => {
        SystemBridge.getDistroInfo().then(async distro => {
            const stats = await SystemBridge.getSystemStats();
            setInfo({ ...distro, Kernel: stats.kernel });
        });
    }, []);
    return (
        <div className="h-full flex flex-col items-center justify-center bg-gradient-to-br from-slate-900 to-blue-900 text-white p-6 text-center">
            <div className="w-20 h-20 rounded-full bg-blue-500 mb-4 flex items-center justify-center shadow-[0_0_30px_rgba(59,130,246,0.6)]">
                <span className="text-3xl font-bold">B</span>
            </div>
            <h1 className="text-2xl font-bold mb-1">{info.Name}</h1>
            <p className="text-blue-200 text-sm mb-6">Blue Environment</p>
            <div className="text-xs text-slate-400 space-y-1">
                <p>Version {info.Version}</p>
                <p>Kernel: {info.Kernel}</p>
                <p>{info.Copyright}</p>
                <p className="mt-4 pt-4 border-t border-white/10 text-slate-500">
                    Compositor: Smithay (Wayland) + XWayland
                </p>
            </div>
        </div>
    );
};
export default AboutApp;
