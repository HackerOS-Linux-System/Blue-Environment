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

        // Uzupełnij brakujące pola
        if (loaded.desktopPath === undefined) {
            loaded.desktopPath = await SystemBridge.getDefaultDesktopPath();
        }
        if (loaded.panelEnabled === undefined) loaded.panelEnabled = DEFAULT_CONFIG.panelEnabled;
        if (loaded.panelPosition === undefined) loaded.panelPosition = DEFAULT_CONFIG.panelPosition;
        if (loaded.panelSize === undefined) loaded.panelSize = DEFAULT_CONFIG.panelSize;
        if (loaded.panelOpacity === undefined) loaded.panelOpacity = DEFAULT_CONFIG.panelOpacity;
        if (loaded.language === undefined) loaded.language = DEFAULT_CONFIG.language;
        if (loaded.nightLightEnabled === undefined) loaded.nightLightEnabled = DEFAULT_CONFIG.nightLightEnabled;
        if (loaded.nightLightTemperature === undefined) loaded.nightLightTemperature = DEFAULT_CONFIG.nightLightTemperature;
        if (loaded.nightLightSchedule === undefined) loaded.nightLightSchedule = DEFAULT_CONFIG.nightLightSchedule;
        if (loaded.nightLightStartHour === undefined) loaded.nightLightStartHour = DEFAULT_CONFIG.nightLightStartHour;
        if (loaded.nightLightEndHour === undefined) loaded.nightLightEndHour = DEFAULT_CONFIG.nightLightEndHour;
        if (loaded.appsEnabled === undefined) loaded.appsEnabled = DEFAULT_CONFIG.appsEnabled;
        if (loaded.accounts === undefined) loaded.accounts = {};

        // Napraw ścieżkę tapety – jeśli nie ma prefiksu file://, dodaj go
        if (loaded.wallpaper && !loaded.wallpaper.startsWith('file://') && !loaded.wallpaper.startsWith('data:')) {
            loaded.wallpaper = `file://${loaded.wallpaper}`;
        }

        this.config = loaded;
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
            const style = document.getElementById('custom-theme-style') || document.createElement('style');
            style.id = 'custom-theme-style';
            style.innerHTML = customTheme.css;
            document.head.appendChild(style);
        } else {
            const style = document.getElementById('custom-theme-style');
            if (style) style.remove();
        }
    }
}

export const configStore = new ConfigStore();
