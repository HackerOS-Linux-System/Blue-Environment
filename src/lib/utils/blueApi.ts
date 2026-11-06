import { SystemBridge } from './systemBridge';
import { notificationManager } from './notificationManager';
import type { PackageKind } from './blueStore';

export interface BlueAppInfo {
    id: string;
    name: string;
    version: string;
    kind: PackageKind;
    permissions: string[];
}

export interface BlueApi {
    app: BlueAppInfo;
    notify(title: string, message?: string): void;
    clipboard: {
        readText(): Promise<string>;
        writeText(text: string): Promise<void>;
    };
    storage: {
        get<T = unknown>(key: string): T | null;
        set(key: string, value: unknown): void;
        remove(key: string): void;
        keys(): string[];
    };
    system: {
        isTauri(): boolean;
        platform(): Promise<string>;
    };
    /**
     * Camera, microphone and screen capture. Each call needs its own permission
     * (`camera`, `microphone`, `screen_capture`) *and* the person's consent at
     * install time. Screen capture goes through the desktop's ScreenCast portal
     * (xdg-desktop-portal-wlr), so the system's own chooser decides what is shared.
     */
    media: {
        camera(constraints?: MediaTrackConstraints | boolean): Promise<MediaStream>;
        microphone(constraints?: MediaTrackConstraints | boolean): Promise<MediaStream>;
        screen(): Promise<MediaStream>;
    };
    /** Requests the host window close itself (same as the titlebar ✕). */
    close(): void;
}

const STORAGE_PREFIX = 'blue-app-storage:';

function requires(app: BlueAppInfo, permission: string) {
    if (!app.permissions.includes(permission)) {
        throw new Error(`"${app.name}" tried to use "${permission}" without declaring it in blue.hk's [permissions] list.`);
    }
}

/** Builds the `api` object for one running instance of a community app. */
export function createBlueApi(app: BlueAppInfo, onClose: () => void): BlueApi {
    const storageKeyPrefix = `${STORAGE_PREFIX}${app.kind}:${app.id}:`;

    return {
        app,
        notify(title: string, message = '') {
            requires(app, 'notifications');
            notificationManager.add({ title, message, appId: `community:${app.kind}:${app.id}`, icon: '' });
        },
        clipboard: {
            async readText() {
                requires(app, 'clipboard');
                return SystemBridge.readText();
            },
            async writeText(text: string) {
                requires(app, 'clipboard');
                await SystemBridge.copyText(text);
            },
        },
        storage: {
            get<T = unknown>(key: string): T | null {
                requires(app, 'storage');
                try {
                    const raw = localStorage.getItem(storageKeyPrefix + key);
                    return raw === null ? null : (JSON.parse(raw) as T);
                } catch {
                    return null;
                }
            },
            set(key: string, value: unknown) {
                requires(app, 'storage');
                try {
                    localStorage.setItem(storageKeyPrefix + key, JSON.stringify(value));
                } catch {
                    /* quota exceeded or value not serialisable — silently ignored, same as SystemBridge's own storage helpers */
                }
            },
            remove(key: string) {
                requires(app, 'storage');
                localStorage.removeItem(storageKeyPrefix + key);
            },
            keys(): string[] {
                requires(app, 'storage');
                const out: string[] = [];
                for (let i = 0; i < localStorage.length; i++) {
                    const k = localStorage.key(i);
                    if (k?.startsWith(storageKeyPrefix)) out.push(k.slice(storageKeyPrefix.length));
                }
                return out;
            },
        },
        system: {
            isTauri: () => SystemBridge.isTauri(),
            async platform() {
                requires(app, 'system');
                // Blue Environment only ever runs on Linux (HackerOS) today —
                // no need for an extra plugin/round-trip just for this.
                return SystemBridge.isTauri() ? 'linux' : 'web';
            },
        },
        media: {
            async camera(constraints: MediaTrackConstraints | boolean = true) {
                requires(app, 'camera');
                return navigator.mediaDevices.getUserMedia({ video: constraints });
            },
            async microphone(constraints: MediaTrackConstraints | boolean = true) {
                requires(app, 'microphone');
                return navigator.mediaDevices.getUserMedia({ audio: constraints });
            },
            async screen() {
                requires(app, 'screen_capture');
                return navigator.mediaDevices.getDisplayMedia({ video: true });
            },
        },
        close: onClose,
    };
}
