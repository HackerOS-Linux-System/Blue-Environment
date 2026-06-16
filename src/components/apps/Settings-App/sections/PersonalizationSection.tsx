import React, { useState, useEffect } from 'react';
import { ThemeDefinition } from '../../../../types';
import { SystemBridge, ThemeDefinition as SBThemeDefinition } from '../../../../utils/systemBridge';

function toSBTheme(t: ThemeDefinition): SBThemeDefinition {
    return {
        id: t.id,
        name: t.name,
        colors: (t.colors ?? {}) as Record<string, string>,
        type: t.type,
        css: t.css,
    };
}

function fromSBTheme(t: SBThemeDefinition): ThemeDefinition {
    return {
        id: t.id,
        name: t.name,
        colors: t.colors,
        type: t.type,
        css: t.css,
    };
}

const PersonalizationSection: React.FC = () => {
    const [themes, setThemes] = useState<ThemeDefinition[]>([]);

    useEffect(() => {
        SystemBridge.getCustomThemes().then((ts: SBThemeDefinition[]) => setThemes(ts.map(fromSBTheme)));
    }, []);

    const handleSave = async (t: ThemeDefinition) => {
        await SystemBridge.saveCustomTheme(toSBTheme(t));
        setThemes(prev => {
            const idx = prev.findIndex(x => x.id === t.id);
            if (idx >= 0) { const n = [...prev]; n[idx] = t; return n; }
            return [...prev, t];
        });
    };

    const handleDelete = async (id: string) => {
        await SystemBridge.deleteCustomTheme(id);
        setThemes(prev => prev.filter(t => t.id !== id));
    };

    return (
        <div className="p-4 space-y-4">
            <h2 className="text-lg font-semibold text-white">Personalization</h2>
            <div className="space-y-2">
                {themes.map(t => (
                    <div key={t.id} className="flex items-center justify-between bg-slate-800 rounded-xl px-4 py-3 border border-white/5">
                        <span className="text-sm text-white">{t.name}</span>
                        <div className="flex gap-2">
                            <button
                                onClick={() => handleSave(t)}
                                className="px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded-lg"
                            >
                                Apply
                            </button>
                            <button
                                onClick={() => handleDelete(t.id)}
                                className="px-3 py-1 text-xs bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-lg"
                            >
                                Delete
                            </button>
                        </div>
                    </div>
                ))}
                {themes.length === 0 && (
                    <p className="text-slate-500 text-sm">No custom themes yet.</p>
                )}
            </div>
        </div>
    );
};

export default PersonalizationSection;
