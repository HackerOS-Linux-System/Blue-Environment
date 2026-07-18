export interface DesktopEntry {
    id: string;
    name: string;
    exec: string;
    icon?: string;
    comment?: string;
    categories?: string[];
    noDisplay?: boolean;
}

export interface UserConfig {
    wallpaper: string;
    theme: string;
    themeName: string;
    accentColor: string;
    displayScale: number;
    desktopPath: string;
    panelEnabled: boolean;
    panelPosition: string;
    panelSize: number;
    panelOpacity: number;
    language: string;
    nightLightEnabled: boolean;
    nightLightTemperature: number;
    nightLightSchedule: 'manual' | 'sunset';
    nightLightStartHour: number;
    nightLightEndHour: number;
    appsEnabled: Record<string, boolean>;
    accounts: Record<string, any>;
    aiConfig?: AIConfig;
    customThemes?: ThemeDefinition[];
}

export interface PowerProfile {
    name: string;
    active: boolean;
    icon: string;
    description: string;
}

export interface ThemeDefinition {
    id: string;
    name: string;
    colors: Record<string, string>;
    type?: 'builtin' | 'custom';
    css?: string;
}

export interface Notification {
    id: string;
    title: string;
    body?: string;
    message?: string;
    timestamp: number;
    read: boolean;
    app?: string;
    appId?: string;
}

export interface PackageInfo {
    id: string;
    name: string;
    description: string;
    version: string;
    source: 'dnf' | 'flatpak' | 'appimage';
    installed: boolean;
    // Matches the Rust struct field name as serialized over the wire
    // (snake_case, like every other backend struct in this codebase —
    // see FileEntry's is_dir/mime_type for the same convention). This
    // was previously named `updateAvailable` here, which never matched
    // anything the backend actually sent, so the Updates tab silently
    // always showed zero updates regardless of what apt/flatpak reported.
    update_available?: boolean;
    icon?: string;
    size?: string;
}

export interface AICallRequest {
    service: string;
    model: string;
    apiKey?: string;
    messages: AIMessage[];
}

export interface AIMessage {
    role: 'user' | 'assistant' | 'system';
    content: string;
}

export interface AIConfig {
    service: string;
    model: string;
    apiKey: string;
}

export interface ExternalWindow {
    id: string;
    pid: number;
    title: string;
    class: string;
    iconPath: string;
    isMinimized: boolean;
    desktop: number;
}

// ============================================================================
// Local file → webview-loadable URL
// ============================================================================
// Tauri's webview can't load raw `file://` URLs for security reasons (this is
// what silently broke the wallpaper and desktop icon thumbnails — the file
// exists on disk, the path is correct, but the webview just refuses the
// cross-origin file:// request). Local files have to go through the `asset:`
// custom protocol instead, which is enabled + scoped in tauri.conf.json's
// app.security.assetProtocol. This mirrors the convention already used by
// BlueImagesApp.
export function toAssetUrl(pathOrFileUrl: string): string {
    if (!pathOrFileUrl) return pathOrFileUrl;
    const rawPath = pathOrFileUrl.startsWith('file://') ? pathOrFileUrl.slice('file://'.length) : pathOrFileUrl;
    if (rawPath.startsWith('asset://')) return rawPath;
    return `asset://localhost/${rawPath}`;
}

// ============================================================================
// Tauri v2 environment detection
// ============================================================================

let isTauri = false;
let invoke: <T = any>(cmd: string, args?: any) => Promise<T>;

// Tauri v2 always injects window.__TAURI_INTERNALS__ regardless of withGlobalTauri.
// We detect it synchronously, but the actual @tauri-apps/api/core module must be
// imported via ESM `import()` — `require()` does not exist in a Vite/browser bundle
// and silently throws, which was breaking every single invoke() call.
const hasTauriInternals = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;

if (hasTauriInternals) {
    isTauri = true;
}

// Lazily-resolved real invoke, swapped in once the ESM import resolves.
let realInvoke: ((cmd: string, args?: any) => Promise<any>) | null = null;
let realInvokePromise: Promise<void> | null = null;

function ensureRealInvoke(): Promise<void> {
    if (!realInvokePromise) {
        realInvokePromise = import('@tauri-apps/api/core')
        .then((core) => {
            realInvoke = core.invoke;
        })
        .catch((e) => {
            console.error('Failed to load @tauri-apps/api/core:', e);
        });
    }
    return realInvokePromise;
}

if (isTauri) {
    // Kick off the import immediately so it's likely ready by the first call.
    ensureRealInvoke();
    invoke = async (cmd: string, args?: any) => {
        if (!realInvoke) {
            await ensureRealInvoke();
        }
        if (!realInvoke) {
            console.warn(`Tauri invoke not available, cmd: ${cmd}`, args);
            return null;
        }
        return realInvoke(cmd, args);
    };
} else {
    invoke = async <T = any>(cmd: string, args?: any): Promise<T> => {
        console.log(`[Mock] invoke: ${cmd}`, args);
        return null as T;
    };
}

// ============================================================================
// Mock helpers
// ============================================================================

let mockWifi = {
    connectedSSID: 'BlueNet 5G',
    networks: [
        { ssid: 'BlueNet 5G',  signal: 92, secure: true,  in_use: true,  bssid: 'AA:BB:CC:DD:EE:FF', frequency: '5 GHz' },
        { ssid: 'BlueNet 2.4', signal: 75, secure: true,  in_use: false, bssid: 'AA:BB:CC:DD:EE:FE', frequency: '2.4 GHz' },
        { ssid: 'Free WiFi',   signal: 40, secure: false, in_use: false, bssid: '11:22:33:44:55:66', frequency: '2.4 GHz' },
    ],
};

let mockBt = [
    { name: 'Sony WH-1000XM4',      mac: '00:11:22:33:44', device_type: 'audio-headphones', connected: true,  paired: true, trusted: true, battery: 72   },
{ name: 'Logitech MX Master 3', mac: 'AA:BB:CC:DD:EE', device_type: 'input-mouse',       connected: true,  paired: true, trusted: true, battery: null },
];

let mockVolume     = 65;
let mockBrightness = 80;

// ============================================================================
// Tauri v2 plugin helpers (lazy-loaded to avoid build errors when packages
// are not installed yet; fall back to invoke commands automatically)
// ============================================================================

async function pluginClipboardWriteText(text: string): Promise<void> {
    try {
        const mod = await import('@tauri-apps/plugin-clipboard-manager');
        await mod.writeText(text);
    } catch {
        await invoke('clipboard_copy', { text });
    }
}

async function pluginClipboardReadText(): Promise<string> {
    try {
        const mod = await import('@tauri-apps/plugin-clipboard-manager');
        return await mod.readText() ?? '';
    } catch {
        return (await invoke('clipboard_paste')) || '';
    }
}

async function pluginClipboardWriteImage(blob: Blob): Promise<void> {
    try {
        const mod = await import('@tauri-apps/plugin-clipboard-manager');
        // Tauri v2 plugin-clipboard-manager exposes writeImage
        if (typeof (mod as any).writeImage === 'function') {
            await (mod as any).writeImage(blob);
        } else {
            throw new Error('writeImage not available');
        }
    } catch {
        // Convert blob → dataUrl and send via invoke fallback
        const dataUrl: string = await new Promise((res, rej) => {
            const reader = new FileReader();
            reader.onload = () => res(reader.result as string);
            reader.onerror = rej;
            reader.readAsDataURL(blob);
        });
        await invoke('write_clipboard_image', { dataUrl });
    }
}

async function pluginDialogOpenDirectory(): Promise<string> {
    try {
        const mod = await import('@tauri-apps/plugin-dialog');
        const selected = await mod.open({ directory: true, multiple: false, title: 'Wybierz katalog' });
        return (selected as string | null) ?? '';
    } catch {
        return (await invoke('pick_directory')) ?? '';
    }
}

/**
 * Opens a Tauri-native file picker that stays inside the shell window.
 * `filters` follows Tauri's DialogFilter format:
 *   [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] }]
 *
 * Returns an array of selected file paths (empty if user cancelled).
 * Falls back to the backend `pick_file` command when the JS plugin isn't
 * available.
 */
async function pluginDialogOpenFile(
    filters: { name: string; extensions: string[] }[] = [],
    multiple = false,
    title = 'Wybierz plik',
): Promise<string[]> {
    try {
        const mod = await import('@tauri-apps/plugin-dialog');
        const selected = await mod.open({ filters, multiple, title });
        if (!selected) return [];
        if (Array.isArray(selected)) return selected as string[];
        return [selected as string];
    } catch {
        // Plugin not available — use the backend pick_file command instead.
        try {
            const path: string | null = await invoke('pick_file', {
                filters: filters.map(f => f.extensions).flat().join(','),
            });
            return path ? [path] : [];
        } catch {
            return [];
        }
    }
}

// ============================================================================
// Main SystemBridge object
// ============================================================================

export const SystemBridge = {
    // --- Environment ---
    isTauri: (): boolean => isTauri,

    /** Low-level Tauri invoke — use only when a higher-level SystemBridge method doesn't exist. */
    invoke: async <T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
        return invoke<T>(cmd, args);
    },

    // --- Session ---
    getSessionType: async (): Promise<string> => {
        if (isTauri) return await invoke('get_session_type');
        return 'wayland:mock';
    },

    getUsername: (): string => {
        try { return (window as any).__TAURI_ENV_USERNAME__ || 'user'; } catch { return 'user'; }
    },

    getHostname: async (): Promise<string> => {
        // Cached on window after the first real lookup so repeated calls
        // (many components call this) don't all hit the backend.
        const cached = (window as any).__TAURI_ENV_HOSTNAME__;
        if (cached) return cached;
        if (isTauri) {
            try {
                const h = await invoke('get_hostname');
                (window as any).__TAURI_ENV_HOSTNAME__ = h;
                return h;
            } catch {}
        }
        return 'localhost';
    },

    /**
     * Populates window.__TAURI_ENV_USERNAME__ / __TAURI_ENV_HOSTNAME__ /
     * __TAURI_HOME__ from the real backend. Call this once at shell
     * startup (see App.tsx) — getUsername() above (and any other code
     * reading these globals directly) stays synchronous and just reads
     * whatever this populated, which is why this used to silently show
     * "user" / "localhost" everywhere: nothing was ever calling it.
     */
    initEnvironment: async (): Promise<void> => {
        if (!isTauri) return;
        try {
            const [username, hostname, home] = await Promise.all([
                invoke('get_username').catch(() => 'user'),
                invoke('get_hostname').catch(() => 'localhost'),
                invoke('get_home_path').catch(() => null),
            ]);
            (window as any).__TAURI_ENV_USERNAME__ = username;
            (window as any).__TAURI_ENV_HOSTNAME__ = hostname;
            if (home) (window as any).__TAURI_HOME__ = home;
        } catch {}
    },

    // --- Apps ---
    getSystemApps: async (forceRefresh = false): Promise<DesktopEntry[]> => {
        if (isTauri) return await invoke('get_system_apps', { forceRefresh }) ?? [];
        return [];
    },

    getRecentApps: async (): Promise<string[]> => {
        if (isTauri) return await invoke('get_recent_apps') ?? [];
        try { return JSON.parse(localStorage.getItem('blue_recent_apps') || '[]'); } catch { return []; }
    },

    recordAppLaunch: async (appId: string): Promise<void> => {
        if (isTauri) { await invoke('record_app_launch', { appId }); return; }
        const existing: string[] = JSON.parse(localStorage.getItem('blue_recent_apps') || '[]');
        const updated = [appId, ...existing.filter(id => id !== appId)].slice(0, 20);
        localStorage.setItem('blue_recent_apps', JSON.stringify(updated));
    },

    invalidateAppCache: async (): Promise<void> => {
        if (isTauri) await invoke('invalidate_app_cache');
    },

    launchApp: async (exec: string, appId?: string): Promise<void> => {
        if (isTauri) { await invoke('launch_process', { command: exec, appId: appId ?? null }); return; }
        console.log(`[Mock] Launch: ${exec}`);
    },

    getAllApps: async (): Promise<DesktopEntry[]> => SystemBridge.getSystemApps(false),

    // --- External windows ---
    getExternalWindows: async (): Promise<ExternalWindow[]> => {
        if (isTauri) {
            const windows = await invoke('get_external_windows') ?? [];
            return windows.map((w: any) => ({
                id: w.id, pid: w.pid, title: w.title, class: w.class,
                iconPath: w.icon_path || '', isMinimized: w.is_minimized || false, desktop: w.desktop || 0,
            }));
        }
        return [];
    },

    focusExternalWindow:    async (winId: string) => { if (isTauri) await invoke('focus_external_window',    { winId }); },
    minimizeExternalWindow: async (winId: string) => { if (isTauri) await invoke('minimize_external_window', { winId }); },
    closeExternalWindow:    async (winId: string) => { if (isTauri) await invoke('close_external_window',    { winId }); },

    embedExternalWindow: async (winId: string, parentId: string): Promise<boolean> => {
        if (isTauri) return await invoke('embed_external_window', { winId, parentId }) ?? false;
        return false;
    },

    // --- Files ---
    getFiles: async (path: string): Promise<any[]> => {
        if (isTauri) return await invoke('list_files', { path }) ?? [];
        return [];
    },

    readFile: async (path: string): Promise<string> => {
        if (isTauri) return await invoke('read_text_file', { path }) ?? '';
        return 'Mock file content';
    },

    writeFile: async (path: string, content: string): Promise<void> => {
        if (isTauri) await invoke('write_text_file', { path, content });
    },

    saveFile: async (filePath: string, dataUrl: string): Promise<void> => {
        if (isTauri) {
            await invoke('save_file_from_data_url', { path: filePath, dataUrl });
        } else {
            const link = document.createElement('a');
            link.href = dataUrl;
            link.download = filePath.split('/').pop() || 'file.png';
            link.click();
        }
    },

    // --- File pickers (stay inside the shell — use Tauri plugin-dialog,
    //     NOT <input type="file"> which opens an OS file-chooser outside
    //     the Tauri window, showing "Wybór plików" on a white background) ---
    pickFile: async (filters: { name: string; extensions: string[] }[] = [], title?: string): Promise<string | null> => {
        const paths = await pluginDialogOpenFile(filters, false, title);
        return paths[0] ?? null;
    },
    pickFiles: async (filters: { name: string; extensions: string[] }[] = [], title?: string): Promise<string[]> => {
        return pluginDialogOpenFile(filters, true, title);
    },

    pickDirectory: async (): Promise<string> => {
        if (isTauri) return pluginDialogOpenDirectory();
        return prompt('Wybierz katalog (mock):', '/home/user') || '/home/user';
    },

    getHomePath: async (): Promise<string> => {
        if (isTauri) {
            try {
                const { homeDir } = await import('@tauri-apps/api/path');
                return await homeDir();
            } catch {
                return (await invoke('get_home_path')) ?? '/home/user';
            }
        }
        return '/home/user';
    },

    // --- Git ---
    gitStatus: async (path: string): Promise<string[]> => {
        if (isTauri) { try { return await invoke('git_status', { path }) ?? []; } catch { return []; } }
        return [];
    },

    // --- System stats ---
    getSystemStats: async () => {
        if (isTauri) {
            const s = await invoke('get_system_stats');
            return {
                cpu: s.cpu, ram: s.ram, battery: s.battery, isCharging: s.is_charging,
                volume: s.volume, brightness: s.brightness, wifiSSID: s.wifi_ssid,
                kernel: s.kernel, sessionType: s.session_type,
                netRx: s.net_rx_mb, netTx: s.net_tx_mb,
                diskRead: s.disk_read_mb, diskWrite: s.disk_write_mb,
            };
        }
        return {
            cpu: 15, ram: 45, battery: 82, isCharging: false,
            volume: mockVolume, brightness: mockBrightness,
            wifiSSID: mockWifi.connectedSSID, kernel: 'WebKernel 1.0', sessionType: 'wayland:mock',
            netRx: 0, netTx: 0, diskRead: 0, diskWrite: 0,
        };
    },

    getProcesses: async () => {
        if (isTauri) return await invoke('get_processes') ?? [];
        return [
            { pid: '1234', name: 'firefox',  cpu: 12.5, memory: 400_000_000 },
            { pid: '5678', name: 'blue-env', cpu:  5.2, memory: 150_000_000 },
        ];
    },

    // --- Audio ---
    getAudioSinks: async () => {
        if (isTauri) return await invoke('get_audio_sinks') ?? [];
        return [
            { id: 0, name: 'alsa_output.pci-0000_00_1f.3.analog-stereo', description: 'Built-in Speakers', volume: 65, muted: false, is_default: true },
            { id: 1, name: 'alsa_output.usb-Sony_WH1000XM4.analog-stereo', description: 'Sony WH-1000XM4', volume: 80, muted: false, is_default: false },
        ];
    },

    setVolume:      async (level: number)                      => { mockVolume = level; if (isTauri) await invoke('set_volume',      { level }); },
    setSinkVolume:  async (sinkName: string, volume: number)   => { if (isTauri) await invoke('set_sink_volume',  { sinkName, volume }); },
    setDefaultSink: async (sinkName: string)                   => { if (isTauri) await invoke('set_default_sink', { sinkName }); },
    toggleSinkMute: async (sinkName: string)                   => { if (isTauri) await invoke('toggle_sink_mute', { sinkName }); },

    // --- Wi-Fi ---
    // Android-style mobile data — see commands/network.rs::has_cellular_modem.
    // Laptops without WWAN hardware never see this in the UI at all.
    hasCellularModem: async (): Promise<boolean> => {
        if (isTauri) return await invoke('has_cellular_modem').catch(() => false);
        return false;
    },
    getCellularStatus: async (): Promise<{ connected: boolean; signal: number; carrier: string } | null> => {
        if (isTauri) return await invoke('get_cellular_status').catch(() => null);
        return null;
    },
    setCellularEnabled: async (enabled: boolean): Promise<void> => {
        if (isTauri) await invoke('set_cellular_enabled', { enabled }).catch(() => {});
    },

    getWifiNetworks: async () => {
        if (isTauri) { try { return await invoke('get_wifi_networks_real'); } catch { return []; } }
        await new Promise(r => setTimeout(r, 400));
        return mockWifi.networks;
    },

    connectWifi: async (ssid: string, pass: string): Promise<boolean> => {
        if (isTauri) { try { await invoke('connect_wifi_real', { ssid, password: pass }); return true; } catch { return false; } }
        await new Promise(r => setTimeout(r, 1200));
        mockWifi.connectedSSID = ssid;
        mockWifi.networks = mockWifi.networks.map(n => ({ ...n, in_use: n.ssid === ssid }));
        return true;
    },

    disconnectWifi: async () => { if (isTauri) await invoke('disconnect_wifi'); else mockWifi.connectedSSID = ''; },
    toggleWifi:     async (enabled: boolean) => { if (isTauri) await invoke('toggle_wifi', { enabled }); else if (!enabled) mockWifi.connectedSSID = ''; },

    // --- Bluetooth ---
    getBluetoothDevices: async () => {
        if (isTauri) { try { return await invoke('get_bluetooth_devices_real'); } catch { return []; } }
        await new Promise(r => setTimeout(r, 300));
        return mockBt;
    },

    bluetoothConnect:    async (mac: string) => { if (isTauri) await invoke('bluetooth_connect',    { mac }); else mockBt = mockBt.map(d => d.mac === mac ? { ...d, connected: true }  : d); },
    bluetoothDisconnect: async (mac: string) => { if (isTauri) await invoke('bluetooth_disconnect', { mac }); else mockBt = mockBt.map(d => d.mac === mac ? { ...d, connected: false } : d); },
    bluetoothPair:       async (mac: string) => { if (isTauri) await invoke('bluetooth_pair',       { mac }); },

    toggleBluetoothDevice: async (mac: string) => {
        const dev = mockBt.find(d => d.mac === mac);
        if (!dev) return;
        if (dev.connected) await SystemBridge.bluetoothDisconnect(mac);
        else               await SystemBridge.bluetoothConnect(mac);
    },

    // --- Brightness ---
    setBrightness: async (level: number) => { mockBrightness = level; if (isTauri) await invoke('set_brightness', { level }); },

    // --- Screenshot ---
    takeScreenshot: async (): Promise<string | null> => {
        if (isTauri) {
            const path: string = await invoke('take_screenshot');
            return path || null;
        }
        return null;
    },

    // --- Camera (native fallback for when getUserMedia is unavailable,
    //     which is the case in Tauri's Linux webview — see CameraApp/mod.rs) ---
    cameraListDevices: async (): Promise<{ path: string; name: string }[]> => {
        if (isTauri) return await invoke('camera_list_devices') ?? [];
        return [];
    },
    cameraCheckAvailable: async (): Promise<boolean> => {
        if (isTauri) return await invoke('camera_check_available');
        return false;
    },
    cameraCaptureFrame: async (device: string, width: number, height: number): Promise<string> => {
        if (isTauri) return await invoke('camera_capture_frame', { device, width, height });
        throw new Error('Not running under Tauri');
    },
    cameraCapturePhoto: async (device: string, width: number, height: number): Promise<string> => {
        if (isTauri) return await invoke('camera_capture_photo', { device, width, height });
        throw new Error('Not running under Tauri');
    },
    cameraRecordVideo: async (device: string, width: number, height: number, durationSecs: number): Promise<string> => {
        if (isTauri) return await invoke('camera_record_video', { device, width, height, durationSecs });
        throw new Error('Not running under Tauri');
    },

    // --- Clipboard image ---
    writeClipboardImage: async (dataUrl: string): Promise<void> => {
        if (isTauri) {
            try {
                const blob = await (await fetch(dataUrl)).blob();
                await pluginClipboardWriteImage(blob);
            } catch (e) {
                console.error('writeClipboardImage error:', e);
            }
        } else {
            await navigator.clipboard.writeText(dataUrl);
            console.log('[Mock] writeClipboardImage (dataUrl)');
        }
    },

    // --- Wallpapers & config ---
    getWallpapers: async (): Promise<string[]> => {
        if (isTauri) {
            try {
                const wps: string[] = await invoke('get_wallpapers') ?? [];
                if (wps.length > 0) return wps.map(wp => wp.startsWith('file://') ? wp : `file://${wp}`);
            } catch {}
        }
        return [
            'file:///usr/share/Blue-Environment/wallpapers/default.png',
            'file:///usr/share/wallpapers/default.png',
        ];
    },

    getWallpaperPreview: async (path: string): Promise<string | null> => {
        const cleanPath = path.replace(/^file:\/\//, '');
        if (isTauri) {
            try {
                const data = await invoke('get_wallpaper_preview', { path: cleanPath });
                if (data && typeof data === 'string') return data;
            } catch (e) { console.error('Wallpaper preview error:', e); }
        }
        return null;
    },

    getDistroInfo: async () => {
        if (isTauri) return await invoke('load_distro_info') ?? {};
        return { Name: 'HackerOS', Version: '0.2.0-alpha', Copyright: '© 2026 HackerOS Team' };
    },

    powerAction: async (action: string): Promise<void> => {
        if (isTauri) await invoke('system_power', { action });
        else console.log(`[Mock] Power: ${action}`);
    },

    saveConfig: async (config: UserConfig): Promise<void> => {
        if (isTauri) await invoke('save_config', { config: JSON.stringify(config) });
        localStorage.setItem('blue_user_config', JSON.stringify(config));
    },

    loadConfig: async (): Promise<UserConfig> => {
        if (isTauri) {
            const raw = await invoke('load_config');
            if (raw && raw !== '{}') { try { return JSON.parse(raw); } catch {} }
        }
        const local = localStorage.getItem('blue_user_config');
        if (local) { try { return JSON.parse(local); } catch {} }
        return {
            wallpaper: 'file:///usr/share/Blue-Environment/wallpapers/default.png',
            theme: 'dark', themeName: 'blue-default', accentColor: 'blue', displayScale: 1,
            desktopPath: 'HOME/Desktop', panelEnabled: true, panelPosition: 'bottom',
            panelSize: 40, panelOpacity: 0.9, language: 'en',
            nightLightEnabled: false, nightLightTemperature: 4000,
            nightLightSchedule: 'manual', nightLightStartHour: 20, nightLightEndHour: 6,
            appsEnabled: { blueAI: true, blueCode: true, blueSoftware: true, mail: true },
            accounts: {},
        };
    },

    // --- Window state ---
    saveWindowState: async (windows: any[]): Promise<void> => {
        if (isTauri) await invoke('save_window_state', { windows });
        else localStorage.setItem('blue_window_state', JSON.stringify(windows));
    },

    loadWindowState: async (): Promise<any[]> => {
        if (isTauri) return await invoke('load_window_state') ?? [];
        try { return JSON.parse(localStorage.getItem('blue_window_state') || '[]'); } catch { return []; }
    },

    // --- Text clipboard ---
    async copyText(text: string): Promise<void> {
        if (isTauri) { await pluginClipboardWriteText(text); return; }
        await navigator.clipboard.writeText(text).catch(() => {});
    },

    async readText(): Promise<string> {
        if (isTauri) return pluginClipboardReadText();
        try { return await navigator.clipboard.readText(); } catch { return ''; }
    },

    async hasText(): Promise<boolean> {
        return (await SystemBridge.readText()).trim().length > 0;
    },

    async clear(): Promise<void> {
        await SystemBridge.copyText('');
    },

    // --- Clipboard history ---
    async getClipboardHistory(): Promise<{ id: string; content: string; timestamp: number }[]> {
        if (isTauri) return await invoke('get_clipboard_history') ?? [];
        const hist = localStorage.getItem('clipboard_history');
        return hist ? JSON.parse(hist) : [];
    },

    async addToClipboardHistory(content: string): Promise<void> {
        if (isTauri) { await invoke('add_to_clipboard_history', { content }); return; }
        const hist = await SystemBridge.getClipboardHistory();
        const newItem = { id: Date.now().toString(), content, timestamp: Date.now() };
        localStorage.setItem('clipboard_history', JSON.stringify([newItem, ...hist].slice(0, 50)));
    },

    async clearClipboardHistory(): Promise<void> {
        if (isTauri) await invoke('clear_clipboard_history');
        else localStorage.removeItem('clipboard_history');
    },

    // --- Night light ---
    async setNightLightEnabled(enabled: boolean): Promise<void>       { if (isTauri) await invoke('set_night_light_enabled',     { enabled }); },
    async setNightLightTemperature(temperature: number): Promise<void> { if (isTauri) await invoke('set_night_light_temperature', { temperature }); },

    // --- Notifications ---
    async getNotificationHistory(): Promise<Notification[]> {
        if (isTauri) return await invoke('get_notification_history') ?? [];
        const hist = localStorage.getItem('notification_history');
        return hist ? JSON.parse(hist) : [];
    },

    async saveNotificationHistory(notifications: Notification[]): Promise<void> {
        if (isTauri) await invoke('save_notification_history', { notifications });
        else localStorage.setItem('notification_history', JSON.stringify(notifications));
    },

    // --- Custom themes ---
    async getCustomThemes(): Promise<ThemeDefinition[]> {
        if (isTauri) return await invoke('get_custom_themes') ?? [];
        return [];
    },

    async saveCustomTheme(theme: ThemeDefinition): Promise<void> {
        if (isTauri) await invoke('save_custom_theme', { theme });
    },

    async deleteCustomTheme(themeId: string): Promise<void> {
        if (isTauri) await invoke('delete_custom_theme', { themeId });
    },

    // --- Power profiles ---
    async getPowerProfiles(): Promise<PowerProfile[]> {
        if (isTauri) return await invoke('get_power_profiles') ?? [];
        return [
            { name: 'power-saver',  active: false, icon: 'Battery', description: 'Oszczędzanie energii' },
            { name: 'balanced',     active: true,  icon: 'Wind',    description: 'Zrównoważony' },
            { name: 'performance',  active: false, icon: 'Zap',     description: 'Wydajność' },
        ];
    },

    async setPowerProfile(profile: string): Promise<void> {
        if (isTauri) await invoke('set_power_profile', { profile });
    },

    // --- Extra file ops ---
    readFileAsDataURL: async (path: string): Promise<string> => {
        if (isTauri) { try { return await invoke('read_file_as_data_url', { path }); } catch { return ''; } }
        return '';
    },

    createFolder:  async (path: string, name: string)                => { if (isTauri) await invoke('create_folder',  { path, name }); },
    deleteFile:    async (path: string)                               => { if (isTauri) await invoke('delete_file',    { path }); },
    copyFile:      async (src: string, dest: string)                  => { if (isTauri) await invoke('copy_file',      { src, dest }); },
    moveFile:      async (src: string, dest: string)                  => { if (isTauri) await invoke('move_file',      { src, dest }); },
    createTextFile: async (path: string, name: string, content = '') => { if (isTauri) await invoke('create_text_file', { path, name, content }); },

    // --- Terminal & commands ---
    async executeCommand(command: string): Promise<{ stdout: string; stderr: string; code?: number }> {
        if (isTauri) {
            try {
                const result = await invoke('execute_command', { command });
                return result as { stdout: string; stderr: string; code?: number };
            } catch (e) {
                return { stdout: '', stderr: String(e), code: 1 };
            }
        }
        return { stdout: `Mock output for: ${command}`, stderr: '', code: 0 };
    },

    async spawnTerminal(windowId: string): Promise<{ success: boolean; error?: string }> {
        if (isTauri) { try { return await invoke('spawn_terminal', { windowId }); } catch (e) { return { success: false, error: String(e) }; } }
        return { success: false, error: 'Only in Tauri environment' };
    },

    async writeToTerminal(command: string): Promise<{ error?: string }> {
        if (isTauri) { try { return await invoke('write_to_terminal', { command }); } catch (e) { return { error: String(e) }; } }
        return { error: 'Only in Tauri environment' };
    },

    // --- Desktop path ---
    async getDefaultDesktopPath(): Promise<string> {
        if (isTauri) { try { return await invoke('get_default_desktop_path'); } catch {} }
        return 'HOME/Desktop';
    },

    // --- LSP ---
    async startLanguageServer(language: string, rootPath: string): Promise<{ success: boolean; error?: string }> {
        if (isTauri) { try { return await invoke('start_language_server', { language, rootPath }); } catch (e) { return { success: false, error: String(e) }; } }
        return { success: false, error: 'Only in Tauri environment' };
    },

    async stopLanguageServer(language: string, rootPath: string): Promise<boolean> {
        if (isTauri) { try { return await invoke('stop_language_server', { language, rootPath }); } catch { return false; } }
        return false;
    },

    // --- Google sign-in (mock) ---
    googleSignIn: async (): Promise<{ accessToken: string; user: any } | null> =>
    new Promise(resolve => setTimeout(() => resolve({ accessToken: 'mock-token-123', user: { name: 'Jan Kowalski', email: 'jan@example.com', picture: '' } }), 1000)),

    googleSignOut: async (): Promise<void> => { console.log('Signed out from Google'); },

    // --- AI ---
    async getAIConfig(): Promise<AIConfig | null> {
        if (isTauri) { try { return await invoke('get_ai_config'); } catch { return null; } }
        const stored = localStorage.getItem('ai_config');
        return stored ? JSON.parse(stored) : null;
    },

    async saveAIConfig(config: AIConfig): Promise<void> {
        if (isTauri) await invoke('save_ai_config', { config });
        else localStorage.setItem('ai_config', JSON.stringify(config));
    },

    async aiCall(request: AICallRequest): Promise<string> {
        if (isTauri) {
            try { return await invoke('ai_call', { request }); }
            catch (e) { throw new Error(`AI call failed: ${e}`); }
        }
        const lastMsg = request.messages[request.messages.length - 1]?.content || 'empty';
        return `[Mock] ${request.service} response to: ${lastMsg}`;
    },

    // --- Package manager (dnf — LegendaryOS is Fedora-based, no apt; snap is not part of the supported stack) ---
    async getDnfPackages():     Promise<PackageInfo[]> { if (isTauri) return await invoke('get_dnf_packages')     ?? []; return MOCK_PACKAGES; },
    async getFlatpakPackages(): Promise<PackageInfo[]> { if (isTauri) return await invoke('get_flatpak_packages') ?? []; return []; },
    async getAppImagePackages(): Promise<PackageInfo[]> { if (isTauri) return await invoke('get_appimage_packages') ?? []; return []; },

    async installDnfPackage(pkgId: string):    Promise<boolean> { if (isTauri) return await invoke('install_dnf_package', { pkgId }); return true; },
    async removeDnfPackage(pkgId: string):     Promise<boolean> { if (isTauri) return await invoke('remove_dnf_package',  { pkgId }); return true; },
    async updateDnfPackage(pkgId: string):     Promise<boolean> { if (isTauri) return await invoke('update_dnf_package',  { pkgId }); return true; },
    async installFlatpakPackage(pkgId: string): Promise<boolean> { if (isTauri) return await invoke('install_flatpak_package', { pkgId }); return true; },
    async removeFlatpakPackage(pkgId: string):  Promise<boolean> { if (isTauri) return await invoke('remove_flatpak_package',  { pkgId }); return true; },
    async updateFlatpakPackage(pkgId: string):  Promise<boolean> { if (isTauri) return await invoke('update_flatpak_package',  { pkgId }); return true; },
    async installAppImage(pkgId: string):  Promise<boolean> { if (isTauri) return await invoke('install_appimage', { pkgId }); return true; },
    async removeAppImage(pkgId: string):   Promise<boolean> { if (isTauri) return await invoke('remove_appimage',  { pkgId }); return true; },
    async updateAppImage(pkgId: string):   Promise<boolean> { if (isTauri) return await invoke('update_appimage',  { pkgId }); return true; },

    // --- Panel ---
    async setPanelEnabled(enabled: boolean): Promise<void> { if (isTauri) await invoke('set_panel_enabled', { enabled }); },

    // --- Generic invoke passthrough (for modules that build their own calls) ---
    async invokeCommand<T = any>(cmd: string, args?: any): Promise<T> {
        if (isTauri) return await invoke<T>(cmd, args);
        throw new Error(`invokeCommand: not in Tauri (cmd=${cmd})`);
    },
};

// ============================================================================
// Inline mock package list (used when not in Tauri)
// ============================================================================
const MOCK_PACKAGES: PackageInfo[] = [
    { id: 'firefox',     name: 'Firefox',     description: 'Fast, private web browser',     version: '125.0',  source: 'dnf',      installed: false },
{ id: 'vlc',         name: 'VLC',         description: 'Versatile media player',         version: '3.0.20', source: 'dnf',      installed: true, update_available: true },
{ id: 'gimp',        name: 'GIMP',        description: 'GNU Image Manipulation Program', version: '2.10.36',source: 'dnf',      installed: false },
{ id: 'libreoffice', name: 'LibreOffice', description: 'Open source office suite',       version: '7.6.0',  source: 'dnf',      installed: true },
{ id: 'code',        name: 'VS Code',     description: 'Code editor by Microsoft',       version: '1.89.0', source: 'flatpak',  installed: false },
{ id: 'discord',     name: 'Discord',     description: 'Chat for communities',            version: '0.0.45', source: 'flatpak',  installed: false },
{ id: 'spotify',     name: 'Spotify',     description: 'Music streaming',                 version: '1.2.35', source: 'flatpak',  installed: false },
{ id: 'obsidian',    name: 'Obsidian',    description: 'Knowledge base & notes',          version: '1.4.16', source: 'appimage', installed: false },
];
