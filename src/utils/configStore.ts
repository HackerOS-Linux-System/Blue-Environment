import { SystemBridge, UserConfig, AIConfig } from './systemBridge';

const DEFAULT_CONFIG: UserConfig = {
    wallpaper: 'file:///usr/share/Blue-Environment/wallpapers/default.png',
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
        calculator: true,
        notepad: true,
        systemMonitor: true,
        explorer: true,
        terminal: true,
        blueWeb: true,
        about: true,
    },
    accounts: {},
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
