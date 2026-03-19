import { DesktopEntry, UserConfig, PowerProfile, ThemeDefinition } from '../types';

// @ts-ignore
const isTauri = typeof window !== 'undefined' && window.__TAURI__ !== undefined;
// @ts-ignore
const invoke = isTauri ? window.__TAURI__.invoke : async (cmd: string, args?: any) => {
    console.log(`[Mock] invoke: ${cmd}`, args);
    return null;
};

// ── Mock state ─────────────────────────────────────────────────────────────

let mockWifi = {
    connectedSSID: 'BlueNet 5G',
    networks: [
        { ssid: 'BlueNet 5G', signal: 92, secure: true, in_use: true, bssid: 'AA:BB:CC:DD:EE:FF', frequency: '5 GHz' },
        { ssid: 'BlueNet 2.4', signal: 75, secure: true, in_use: false, bssid: 'AA:BB:CC:DD:EE:FE', frequency: '2.4 GHz' },
        { ssid: 'Free WiFi',   signal: 40, secure: false, in_use: false, bssid: '11:22:33:44:55:66', frequency: '2.4 GHz' },
        { ssid: 'Neighbor',    signal: 25, secure: true, in_use: false, bssid: '66:55:44:33:22:11', frequency: '2.4 GHz' },
    ],
};

let mockBt = [
    { name: 'Sony WH-1000XM4', mac: '00:11:22:33:44', device_type: 'audio-headphones', connected: true, paired: true, trusted: true, battery: 72 },
{ name: 'Logitech MX Master 3', mac: 'AA:BB:CC:DD:EE', device_type: 'input-mouse', connected: true, paired: true, trusted: true, battery: null },
{ name: 'iPhone 15 Pro', mac: '11:22:33:44:55', device_type: 'phone', connected: false, paired: true, trusted: false, battery: null },
];

let mockVolume = 65;
let mockBrightness = 80;

let mockSinks = [
    { id: 0, name: 'alsa_output.pci-0000_00_1f.3.analog-stereo', description: 'Built-in Speakers', volume: 65, muted: false, is_default: true },
{ id: 1, name: 'alsa_output.usb-Sony_WH1000XM4.analog-stereo', description: 'Sony WH-1000XM4', volume: 80, muted: false, is_default: false },
];

// ── SystemBridge ───────────────────────────────────────────────────────────

export const SystemBridge = {

    // ── Helpers ────────────────────────────────────────────────────────
    isTauri: (): boolean => isTauri,

    // ── Session ──────────────────────────────────────────────────────────

    getSessionType: async (): Promise<string> => {
        if (isTauri) return await invoke('get_session_type');
        return 'wayland:wayland-1';
    },

    getUsername: (): string => {
        try {
            return (window as any).__TAURI_ENV_USERNAME__ || 'user';
        } catch {
            return 'user';
        }
    },

    // ── Apps ─────────────────────────────────────────────────────────────

    getSystemApps: async (forceRefresh: boolean = false): Promise<any[]> => {
        if (isTauri) return await invoke('get_system_apps', { forceRefresh }) ?? [];
        return [];
    },

    getRecentApps: async (): Promise<string[]> => {
        if (isTauri) return await invoke('get_recent_apps') ?? [];
        try {
            return JSON.parse(localStorage.getItem('blue_recent_apps') || '[]');
        } catch { return []; }
    },

    recordAppLaunch: async (appId: string): Promise<void> => {
        if (isTauri) {
            await invoke('record_app_launch', { appId });
        } else {
            try {
                const existing: string[] = JSON.parse(localStorage.getItem('blue_recent_apps') || '[]');
                const updated = [appId, ...existing.filter(id => id !== appId)].slice(0, 20);
                localStorage.setItem('blue_recent_apps', JSON.stringify(updated));
            } catch {}
        }
    },

    invalidateAppCache: async (): Promise<void> => {
        if (isTauri) await invoke('invalidate_app_cache');
    },

    launchApp: async (exec: string, appId?: string): Promise<void> => {
        if (isTauri) {
            await invoke('launch_process', { command: exec, appId: appId ?? null });
        } else {
            console.log(`[Mock] Launch: ${exec}`);
        }
    },

    getAllApps: async (): Promise<DesktopEntry[]> => {
        if (isTauri) return await invoke('get_system_apps', { forceRefresh: false }) ?? [];
        return [];
    },

    // ── Files ─────────────────────────────────────────────────────────────

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

    // ── System stats ──────────────────────────────────────────────────────

    getSystemStats: async () => {
        if (isTauri) {
            const s = await invoke('get_system_stats');
            return {
                cpu: s.cpu,
                ram: s.ram,
                battery: s.battery,
                isCharging: s.is_charging,
                volume: s.volume,
                brightness: s.brightness,
                wifiSSID: s.wifi_ssid,
                kernel: s.kernel,
                sessionType: s.session_type,
            };
        }
        return {
            cpu: 15, ram: 45, battery: 82, isCharging: false,
            volume: mockVolume, brightness: mockBrightness,
            wifiSSID: mockWifi.connectedSSID,
            kernel: 'WebKernel 1.0',
            sessionType: 'wayland:mock',
        };
    },

    getProcesses: async () => {
        if (isTauri) return await invoke('get_processes') ?? [];
        return [
            { pid: '1234', name: 'firefox', cpu: 12.5, memory: 400_000_000 },
            { pid: '5678', name: 'blue-env', cpu: 5.2, memory: 150_000_000 },
        ];
    },

    // ── Audio (PipeWire/PulseAudio) ───────────────────────────────────────

    getAudioSinks: async () => {
        if (isTauri) return await invoke('get_audio_sinks') ?? [];
        return mockSinks;
    },

    setVolume: async (level: number): Promise<void> => {
        mockVolume = level;
        if (isTauri) await invoke('set_volume', { level });
    },

    setSinkVolume: async (sinkName: string, volume: number): Promise<void> => {
        if (isTauri) await invoke('set_sink_volume', { sinkName, volume });
    },

    setDefaultSink: async (sinkName: string): Promise<void> => {
        mockSinks = mockSinks.map(s => ({ ...s, is_default: s.name === sinkName }));
        if (isTauri) await invoke('set_default_sink', { sinkName });
    },

    toggleSinkMute: async (sinkName: string): Promise<void> => {
        if (isTauri) await invoke('toggle_sink_mute', { sinkName });
    },

    // ── Wi-Fi ─────────────────────────────────────────────────────────────

    getWifiNetworks: async () => {
        if (isTauri) {
            try { return await invoke('get_wifi_networks_real'); }
            catch { return []; }
        }
        await new Promise(r => setTimeout(r, 400));
        return mockWifi.networks;
    },

    connectWifi: async (ssid: string, pass: string): Promise<boolean> => {
        if (isTauri) {
            try {
                await invoke('connect_wifi_real', { ssid, password: pass });
                return true;
            } catch { return false; }
        }
        await new Promise(r => setTimeout(r, 1200));
        mockWifi.connectedSSID = ssid;
        mockWifi.networks = mockWifi.networks.map(n => ({ ...n, in_use: n.ssid === ssid }));
        return true;
    },

    disconnectWifi: async (): Promise<void> => {
        if (isTauri) await invoke('disconnect_wifi');
        else {
            mockWifi.connectedSSID = '';
            mockWifi.networks = mockWifi.networks.map(n => ({ ...n, in_use: false }));
        }
    },

    toggleWifi: async (enabled: boolean): Promise<void> => {
        if (isTauri) await invoke('toggle_wifi', { enabled });
        else if (!enabled) { mockWifi.connectedSSID = ''; }
    },

    // ── Bluetooth ─────────────────────────────────────────────────────────

    getBluetoothDevices: async () => {
        if (isTauri) {
            try { return await invoke('get_bluetooth_devices_real'); }
            catch { return []; }
        }
        await new Promise(r => setTimeout(r, 300));
        return mockBt;
    },

    bluetoothConnect: async (mac: string): Promise<void> => {
        if (isTauri) await invoke('bluetooth_connect', { mac });
        else mockBt = mockBt.map(d => d.mac === mac ? { ...d, connected: true } : d);
    },

    bluetoothDisconnect: async (mac: string): Promise<void> => {
        if (isTauri) await invoke('bluetooth_disconnect', { mac });
        else mockBt = mockBt.map(d => d.mac === mac ? { ...d, connected: false } : d);
    },

    bluetoothPair: async (mac: string): Promise<void> => {
        if (isTauri) await invoke('bluetooth_pair', { mac });
    },

    toggleBluetoothDevice: async (mac: string): Promise<void> => {
        const dev = mockBt.find(d => d.mac === mac);
        if (!dev) return;
        if (dev.connected) await SystemBridge.bluetoothDisconnect(mac);
        else await SystemBridge.bluetoothConnect(mac);
    },

    // ── Brightness ────────────────────────────────────────────────────────

    setBrightness: async (level: number): Promise<void> => {
        mockBrightness = level;
        if (isTauri) await invoke('set_brightness', { level });
    },

    // ── Screenshots ───────────────────────────────────────────────────────

    takeScreenshot: async (): Promise<void> => {
        if (isTauri) await invoke('take_screenshot');
        else alert('[Mock] Screenshot taken');
    },

    // ── Wallpapers & config ───────────────────────────────────────────────

    getWallpapers: async (): Promise<string[]> => {
        if (isTauri) return await invoke('get_wallpapers') ?? [];
        return ['file:///usr/share/wallpapers/default.png'];
    },

    getWallpaperPreview: async (path: string): Promise<string | null> => {
        if (isTauri) {
            try {
                return await invoke('get_wallpaper_preview', { path });
            } catch (e) {
                console.error('Błąd pobierania podglądu tapety:', e);
                return null;
            }
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
            if (raw && raw !== '{}') {
                try { return JSON.parse(raw); } catch {}
            }
        }
        const local = localStorage.getItem('blue_user_config');
        if (local) {
            try { return JSON.parse(local); } catch {}
        }
        return {
            wallpaper: 'file:///usr/share/wallpapers/default.png',
            theme: 'dark',
            themeName: 'blue-default',
            accentColor: 'blue',
            displayScale: 1,
        };
    },

    // ── Window state ──────────────────────────────────────────────────────

    saveWindowState: async (windows: any[]): Promise<void> => {
        if (isTauri) await invoke('save_window_state', { windows });
        else localStorage.setItem('blue_window_state', JSON.stringify(windows));
    },

    loadWindowState: async (): Promise<any[]> => {
        if (isTauri) return await invoke('load_window_state') ?? [];
        try {
            return JSON.parse(localStorage.getItem('blue_window_state') || '[]');
        } catch { return []; }
    },

    // ── Clipboard ─────────────────────────────────────────────────────────

    async copyText(text: string): Promise<void> {
        if (isTauri) {
            try {
                // @ts-ignore
                await window.__TAURI__.clipboard.writeText(text);
                return;
            } catch (e) {
                console.warn('Tauri clipboard write failed, fallback to invoke', e);
            }
        }
        await invoke('clipboard_copy', { text });
    },

    async readText(): Promise<string> {
        if (isTauri) {
            try {
                // @ts-ignore
                return await window.__TAURI__.clipboard.readText();
            } catch (e) {
                console.warn('Tauri clipboard read failed', e);
            }
        }
        return (await invoke('clipboard_paste')) || '';
    },

    async hasText(): Promise<boolean> {
        const text = await this.readText();
        return text.trim().length > 0;
    },

    async clear(): Promise<void> {
        await this.copyText('');
    },

    // ── Nowe metody dla motywów, profili zasilania itp. ───────────────────

    getCustomThemes: async (): Promise<ThemeDefinition[]> => {
        return new Promise(resolve => {
            setTimeout(() => {
                resolve([
                    {
                        id: 'my-dark-theme',
                        name: 'Mój Ciemny Motyw',
                        type: 'custom',
                        css: ':root { --bg-primary: #1a1a1a; --bg-secondary: #2a2a2a; --text-primary: #ffffff; --accent: #ff6600; }',
                        colors: { primary: '#1a1a1a', secondary: '#2a2a2a', text: '#ffffff', accent: '#ff6600' }
                    }
                ]);
            }, 500);
        });
    },

    saveCustomTheme: async (theme: ThemeDefinition): Promise<void> => {
        console.log('Zapisywanie motywu:', theme);
    },

    deleteCustomTheme: async (themeId: string): Promise<void> => {
        console.log('Usuwanie motywu:', themeId);
    },

    getPowerProfiles: async (): Promise<PowerProfile[]> => {
        if (isTauri) {
            try {
                return await invoke('get_power_profiles');
            } catch { return []; }
        }
        return [
            { name: 'power-saver', active: false, icon: 'Battery', description: 'Oszczędzanie energii' },
            { name: 'balanced', active: true, icon: 'Wind', description: 'Zrównoważony' },
            { name: 'performance', active: false, icon: 'Zap', description: 'Wydajność' },
        ];
    },

    setPowerProfile: async (profile: string): Promise<void> => {
        if (isTauri) {
            await invoke('set_power_profile', { profile });
        } else {
            console.log('[Mock] Ustawiono profil zasilania:', profile);
        }
    },

    // ── File operations for Explorer ─────────────────────────────────────

    readFileAsDataURL: async (path: string): Promise<string> => {
        if (isTauri) {
            try {
                return await invoke('read_file_as_data_url', { path });
            } catch (e) {
                console.error('Błąd odczytu pliku jako data URL:', e);
                return '';
            }
        }
        return '';
    },

    createFolder: async (path: string, name: string): Promise<void> => {
        if (isTauri) {
            await invoke('create_folder', { path, name });
        } else {
            console.log('[Mock] Tworzenie folderu:', path, name);
        }
    },

    deleteFile: async (path: string): Promise<void> => {
        if (isTauri) {
            await invoke('delete_file', { path });
        } else {
            console.log('[Mock] Usuwanie pliku:', path);
        }
    },

    copyFile: async (src: string, dest: string): Promise<void> => {
        if (isTauri) {
            await invoke('copy_file', { src, dest });
        } else {
            console.log('[Mock] Kopiowanie pliku:', src, '->', dest);
        }
    },

    moveFile: async (src: string, dest: string): Promise<void> => {
        if (isTauri) {
            await invoke('move_file', { src, dest });
        } else {
            console.log('[Mock] Przenoszenie pliku:', src, '->', dest);
        }
    },

    // ── Terminal execution ───────────────────────────────────────────────

    async executeCommand(command: string): Promise<{ stdout: string; stderr: string }> {
        if (isTauri) {
            try {
                return await invoke('execute_command', { command });
            } catch (e) {
                console.error('Błąd wykonywania komendy:', e);
                return { stdout: '', stderr: String(e) };
            }
        } else {
            console.log(`[Mock] execute: ${command}`);
            return { stdout: `Mock output for: ${command}`, stderr: '' };
        }
    },

    // ── Integracja z Google ─────────────────────────────────────────────

    googleSignIn: async (): Promise<{ accessToken: string; user: any } | null> => {
        return new Promise(resolve => {
            setTimeout(() => {
                resolve({
                    accessToken: 'mock-token-123',
                    user: { name: 'Jan Kowalski', email: 'jan@example.com', picture: '' }
                });
            }, 1000);
        });
    },

    googleSignOut: async (): Promise<void> => {
        console.log('Wylogowano z Google');
    },
};
