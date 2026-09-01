import { writable, get } from 'svelte/store';
import { DEFAULT_WEB_SETTINGS } from './types';
import type { BlueWebSettings } from './types';
import { SystemBridge } from '../../../utils/systemBridge';

const LS_KEY = 'blue-web-settings';

/**
 * A short built-in list of well-known ad/tracker domains — nowhere near
 * a real filter list (EasyList etc. has tens of thousands of rules and
 * updates constantly; shipping and maintaining that is a different,
 * much bigger feature than what's here). This exists to make content
 * blocking a real, working feature rather than an empty toggle with no
 * actual list behind it, while being honest that it's a small starter
 * set, not a serious adblocker's rule database.
 */
export const BUILTIN_BLOCKLIST = [
    'doubleclick.net', 'googlesyndication.com', 'googleadservices.com',
    'google-analytics.com', 'googletagmanager.com', 'googletagservices.com',
    'adservice.google.com', 'facebook.net', 'connect.facebook.net',
    'scorecardresearch.com', 'quantserve.com', 'outbrain.com', 'taboola.com',
    'adnxs.com', 'criteo.com', 'moatads.com', 'amazon-adsystem.com',
];

export function createWebSettings() {
    const stored = (() => {
        try {
            const raw = localStorage.getItem(LS_KEY);
            return raw ? { ...DEFAULT_WEB_SETTINGS, ...JSON.parse(raw) } : { ...DEFAULT_WEB_SETTINGS };
        } catch {
            return { ...DEFAULT_WEB_SETTINGS };
        }
    })();

    const settings = writable<BlueWebSettings>(stored);

    function persist(s: BlueWebSettings) {
        try { localStorage.setItem(LS_KEY, JSON.stringify(s)); } catch { /* best effort */ }
        // Push the effective blocklist to the backend so `on_navigation`
        // (see BlueWebApp/mod.rs) can actually enforce it — content
        // blocking only works if the Rust side knows the current list;
        // this frontend store is the source of truth for it.
        if (SystemBridge.isTauri()) {
            const domains = s.contentBlockingEnabled ? [...BUILTIN_BLOCKLIST, ...s.customBlockedDomains] : [];
            SystemBridge.invokeCommand('web_set_blocklist', { domains }).catch(() => {});
        }
    }

    function update(patch: Partial<BlueWebSettings>) {
        settings.update((prev) => {
            const next = { ...prev, ...patch };
            persist(next);
            return next;
        });
    }

    function addBlockedDomain(domain: string) {
        const d = domain.trim().toLowerCase().replace(/^https?:\/\//, '').replace(/\/.*$/, '');
        if (!d) return;
        update({ customBlockedDomains: [...get(settings).customBlockedDomains, d] });
    }

    function removeBlockedDomain(domain: string) {
        update({ customBlockedDomains: get(settings).customBlockedDomains.filter((d) => d !== domain) });
    }

    /** Pushes the current settings' blocklist to the backend — call once
     * on startup, since `persist()` only runs on subsequent changes. */
    function syncBlocklistToBackend() {
        persist(get(settings));
    }

    return { settings, update, addBlockedDomain, removeBlockedDomain, syncBlocklistToBackend };
}

export type WebSettingsStore = ReturnType<typeof createWebSettings>;
