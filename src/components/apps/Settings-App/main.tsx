import React, { useState, useEffect } from 'react';
import { AppProps, ThemeDefinition } from '../../../types';
import { SystemBridge, ThemeDefinition as SBThemeDefinition } from '../../../utils/systemBridge';

// Cast helper: our types.ts ThemeDefinition → systemBridge ThemeDefinition
function toSBTheme(t: ThemeDefinition): SBThemeDefinition {
    return {
        id: t.id,
        name: t.name,
        colors: (t.colors ?? {}) as Record<string, string>,
        type: t.type,
        css: t.css,
    };
}

// Cast helper: systemBridge ThemeDefinition → types.ts ThemeDefinition
function fromSBTheme(t: SBThemeDefinition): ThemeDefinition {
    return {
        id: t.id,
        name: t.name,
        colors: t.colors,
        type: t.type,
        css: t.css,
    };
}

const SettingsApp: React.FC<AppProps> = ({ windowId }) => {
    const [customThemes, setCustomThemes] = useState<ThemeDefinition[]>([]);

    useEffect(() => {
        SystemBridge.getCustomThemes().then((themes: SBThemeDefinition[]) => {
            setCustomThemes(themes.map(fromSBTheme));
        });
    }, []);

    const saveTheme = async (t: ThemeDefinition) => {
        await SystemBridge.saveCustomTheme(toSBTheme(t));
    };

    // Settings app renders children that handle tabs/sections
    return (
        <div className="flex h-full bg-slate-900 text-white">
            <p className="m-auto text-slate-400 text-sm">Settings loaded – {customThemes.length} custom theme(s)</p>
        </div>
    );
};

export default SettingsApp;
