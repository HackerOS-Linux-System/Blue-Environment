import { UserConfig } from '../types';
import { SystemBridge } from './systemBridge';

const DEFAULT_CONFIG: UserConfig = {
    wallpaper: "file:///usr/share/wallpapers/default.png",
    theme: 'dark',
    themeName: 'blue-default',
    accentColor: 'blue',
    displayScale: 1,
    desktopPath: 'HOME/Desktop',
    panelEnabled: true,
    panelPosition: 'bottom',
    panelSize: 40,
    panelOpacity: 0.9,
    language: 'en',
    nightLightEnabled: false,
    nightLightTemperature: 4000,
    nightLightSchedule: 'manual',
    nightLightStartHour: 20,
    nightLightEndHour: 6,
    appsEnabled: {
        blueAI: true,
        blueCode: true,
        blueSoftware: true,
        mail: true,
    },
    accounts: {},
};

class ConfigStore extends EventTarget {
    private config: UserConfig = DEFAULT_CONFIG;
    private initialized = false;

    async init(): Promise<UserConfig> {
        if (this.initialized) return this.config;
        const loaded = await SystemBridge.loadConfig();

        // Fill in any missing fields from defaults
        const merged: UserConfig = { ...DEFAULT_CONFIG, ...loaded };

        if (!merged.desktopPath) {
            merged.desktopPath = await SystemBridge.getDefaultDesktopPath();
        }
        // Normalize wallpaper path
        if (
            merged.wallpaper &&
            !merged.wallpaper.startsWith('file://') &&
            !merged.wallpaper.startsWith('data:')
        ) {
            merged.wallpaper = `file://${merged.wallpaper}`;
        }

        this.config = merged;
        this.initialized = true;
        this.applyTheme(this.config.themeName);
        return this.config;
    }

    get(): UserConfig {
        return this.config;
    }

    async update(patch: Partial<UserConfig>): Promise<void> {
        this.config = { ...this.config, ...patch };
        if (patch.themeName) this.applyTheme(patch.themeName);
        await SystemBridge.saveConfig(this.config);
        this.dispatchEvent(new CustomEvent<UserConfig>('change', { detail: this.config }));
    }

    subscribe(cb: (cfg: UserConfig) => void): () => void {
        const handler = (e: Event) => cb((e as CustomEvent<UserConfig>).detail);
        this.addEventListener('change', handler);
        return () => this.removeEventListener('change', handler);
    }

    private applyTheme(themeName: string): void {
        document.documentElement.setAttribute('data-theme', themeName);
        const customTheme = this.config.customThemes?.find(t => t.id === themeName);
        if (customTheme?.css) {
            let style = document.getElementById('custom-theme-style');
            if (!style) {
                style = document.createElement('style');
                style.id = 'custom-theme-style';
                document.head.appendChild(style);
            }
            style.innerHTML = customTheme.css;
        } else {
            const style = document.getElementById('custom-theme-style');
            if (style) style.remove();
        }
    }
}

export const configStore = new ConfigStore();
