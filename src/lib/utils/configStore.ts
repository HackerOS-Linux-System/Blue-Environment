import { SystemBridge } from './systemBridge';
import type { UserConfig, AIConfig } from './systemBridge';

const DEFAULT_CONFIG: UserConfig = {
    // Empty until resolved — see `load()` below. Previously this was a
    // single hardcoded path (`file:///usr/share/Blue-Environment/
    // wallpapers/default.png`) used verbatim even on installs where it
    // didn't exist, since nothing ever checked. Real resolution now
    // happens once, dynamically, via `resolve_default_wallpaper` (see
    // that command's doc comment in display.rs for the exact fallback
    // order: the standard `/usr/share/wallpapers/default.png` first,
    // then whatever `get_wallpapers()` finds anywhere it already scans).
    wallpaper: '',
    theme: 'dark',
    themeName: 'blue-default',
    accentColor: 'blue',
    displayScale: 1,
    desktopPath: 'HOME/Desktop',
    panelEnabled: true,
    panelPosition: 'top',
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
        calculator: true,
        notepad: true,
        systemMonitor: true,
        explorer: true,
        terminal: true,
        blueWeb: true,
        about: true,
    },
    accounts: {},
    customBookmarks: [],
    weatherEnabled: true,
    weatherCity: '',
    weatherUnit: 'celsius',
    clipboardHoverPreviewEnabled: true,
    networkHoverInfoEnabled: true,
};

type Listener = (cfg: UserConfig) => void;

class ConfigStore {
    private config: UserConfig = { ...DEFAULT_CONFIG };
    private loaded = false;
    private listeners: Set<Listener> = new Set();

    async load(): Promise<UserConfig> {
        if (this.loaded) return this.config;
        try {
            const loaded = await SystemBridge.loadConfig();
            this.config = { ...DEFAULT_CONFIG, ...loaded } as UserConfig;
        } catch {
            this.config = { ...DEFAULT_CONFIG };
        }

        // First-ever run on this machine (nothing persisted a wallpaper
        // choice yet, and `DEFAULT_CONFIG.wallpaper` is deliberately
        // empty — see that field's doc comment): resolve a real one
        // dynamically instead of trusting a hardcoded path that might
        // not exist. Persisted immediately so this resolution only ever
        // has to run once per install, not on every single startup.
        if (!this.config.wallpaper) {
            const resolved = await SystemBridge.resolveDefaultWallpaper();
            if (resolved) {
                this.config.wallpaper = resolved;
                try { await SystemBridge.saveConfig(this.config); } catch { /* best effort — still usable this session even if persisting fails */ }
            }
        }

        this.loaded = true;
        return this.config;
    }

    async init(): Promise<UserConfig> {
        return this.load();
    }

    subscribe(listener: Listener): () => void {
        this.listeners.add(listener);
        // immediately emit current config if already loaded
        if (this.loaded) {
            listener(this.config);
        }
        return () => {
            this.listeners.delete(listener);
        };
    }

    private notify(): void {
        this.listeners.forEach(l => l(this.config));
    }

    get(): UserConfig {
        return this.config;
    }

    async save(patch: Partial<UserConfig>): Promise<void> {
        this.config = { ...this.config, ...patch };
        await SystemBridge.saveConfig(this.config);
        this.notify();
    }

    async setAIConfig(aiConfig: AIConfig): Promise<void> {
        await this.save({ aiConfig });
    }
}

export const configStore = new ConfigStore();
export type { UserConfig, AIConfig };
