import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('./systemBridge', () => ({ SystemBridge: { isTauri: () => false, readText: vi.fn(), copyText: vi.fn() } }));
vi.mock('./notificationManager', () => ({ notificationManager: { add: vi.fn() } }));

import { createBlueApi, type BlueAppInfo } from './blueApi';

const app = (permissions: string[]): BlueAppInfo => ({ id: 'x', name: 'X', version: '1.0.0', kind: 'app', permissions });

describe('createBlueApi media gating', () => {
    const getUserMedia = vi.fn(async () => ({ id: 'stream' }) as unknown as MediaStream);
    const getDisplayMedia = vi.fn(async () => ({ id: 'screen' }) as unknown as MediaStream);
    beforeEach(() => {
        getUserMedia.mockClear(); getDisplayMedia.mockClear();
        Object.defineProperty(globalThis.navigator, 'mediaDevices', { value: { getUserMedia, getDisplayMedia }, configurable: true });
    });

    it('refuses camera, microphone and screen without the matching permission — and never reaches the browser API', async () => {
        const api = createBlueApi(app(['storage']), () => {});
        await expect(api.media.camera()).rejects.toThrow(/camera/);
        await expect(api.media.microphone()).rejects.toThrow(/microphone/);
        await expect(api.media.screen()).rejects.toThrow(/screen_capture/);
        expect(getUserMedia).not.toHaveBeenCalled();
        expect(getDisplayMedia).not.toHaveBeenCalled();
    });

    it('each permission unlocks only its own call', async () => {
        const api = createBlueApi(app(['camera']), () => {});
        await api.media.camera();
        expect(getUserMedia).toHaveBeenCalledWith({ video: true });
        await expect(api.media.microphone()).rejects.toThrow();
    });

    it('microphone and screen use the right browser calls', async () => {
        const api = createBlueApi(app(['microphone', 'screen_capture']), () => {});
        await api.media.microphone({ echoCancellation: true });
        expect(getUserMedia).toHaveBeenCalledWith({ audio: { echoCancellation: true } });
        await api.media.screen();
        expect(getDisplayMedia).toHaveBeenCalledWith({ video: true });
    });

    it('a permission the person did not grant never reaches the app (the Rust side filters receipts)', async () => {
        // store_list_installed returns granted ∩ declared, which is what createBlueApi receives:
        const granted = app([]);
        await expect(createBlueApi(granted, () => {}).media.camera()).rejects.toThrow();
    });
});
