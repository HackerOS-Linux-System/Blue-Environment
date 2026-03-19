import { UserConfig } from '../types';
import { SystemBridge } from './systemBridge';

const DEFAULT_CONFIG: UserConfig = {
    wallpaper: "file:///usr/share/wallpapers/default.png",
    theme: 'dark',
    themeName: 'blue-default',
    accentColor: 'blue',
    displayScale: 1,
};

class ConfigStore extends EventTarget {
    private config: UserConfig = DEFAULT_CONFIG;
    private initialized = false;

    async init(): Promise<UserConfig> {
        if (this.initialized) return this.config;
        const loaded = await SystemBridge.loadConfig();
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
    }
}

export const configStore = new ConfigStore();
