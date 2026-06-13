import React from 'react';
import type { UserConfig } from '../../../../types';

interface Props { config: UserConfig; onSave: (p: Partial<UserConfig>) => Promise<void>; }

const APP_LIST = [
    ['Blue AI','blueAI'], ['Blue Code','blueCode'], ['Blue Software','blueSoftware'],
    ['Mail','mail'], ['Calculator','calculator'], ['Notepad','notepad'],
    ['System Monitor','systemMonitor'], ['Explorer','explorer'], ['Terminal','terminal'],
    ['Blue Web','blueWeb'], ['Camera','camera'],
] as const;

const AppsSection: React.FC<Props> = ({ config, onSave }) => (
    <div className="space-y-6 animate-in fade-in slide-in-from-bottom-2 duration-300">
        <h2 className="text-2xl font-bold text-white">Applications</h2>
        <div className="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
            {APP_LIST.map(([name, key]) => (
                <div key={key} className="flex items-center justify-between py-2 border-b border-white/5 last:border-0">
                    <span className="text-white">{name}</span>
                    <input type="checkbox"
                        checked={(config.appsEnabled as any)?.[key] ?? true}
                        onChange={e => onSave({ appsEnabled: { ...config.appsEnabled, [key]: e.target.checked } })}
                        className="w-4 h-4 accent-blue-500" />
                </div>
            ))}
        </div>
    </div>
);
export default AppsSection;
