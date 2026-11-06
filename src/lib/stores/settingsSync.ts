import { get } from 'svelte/store';
import { configStore } from '../utils/configStore';
import { SystemBridge } from '../utils/systemBridge';
import { a11y, normalizeA11y } from './accessibility';
import { focusConfig, setFocusConfig, normalizeConfig } from './focusMode';

/**
 * Frontend half of settings sync (backend: src-tauri/src/settings_sync.rs).
 *
 *  • Collects the allow-listed settings (the backend is the authority on which
 *    keys may travel — `sync_status().fields`; unknown keys are dropped there too).
 *  • Reports only what changed *on this computer* since the last report, so a
 *    value that arrived from another device is never echoed back as a new edit.
 *  • Applies what arrives from other devices to the live configuration.
 *
 * On first run nothing is reported: the baseline is whatever is configured now.
 * "Send this computer's settings" is the explicit way to make this machine win.
 */

export type Snapshot = Record<string, unknown>;
export interface PendingChange { key: string; value: unknown; deleted: boolean }

const BASELINE_KEY = 'blue.sync.baseline.v1';
const A11Y_KEY = 'store:a11y';
const FOCUS_KEY = 'store:focus';

/** Current values of the synced fields, `undefined` ones omitted. */
export function collect(fields: string[], config: Record<string, unknown>, extra: Record<string, unknown>): Snapshot {
    const out: Snapshot = {};
    for (const f of fields) {
        const v = f.startsWith('store:') ? extra[f] : config[f];
        if (v !== undefined) out[f] = v;
    }
    return out;
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/**
 * What changed between the last report and now. A key present before and gone
 * now is reported as `null` (= deleted).
 */
export function diff(baseline: Snapshot, current: Snapshot): Record<string, unknown | null> {
    const changes: Record<string, unknown | null> = {};
    for (const [k, v] of Object.entries(current)) if (!(k in baseline) || !same(baseline[k], v)) changes[k] = v;
    for (const k of Object.keys(baseline)) if (!(k in current)) changes[k] = null;
    return changes;
}

/** Splits remote changes into a `UserConfig` patch and the `store:*` extras. */
export function splitRemote(changes: PendingChange[]): { configPatch: Record<string, unknown>; a11y?: unknown; focus?: unknown } {
    const configPatch: Record<string, unknown> = {};
    let a11yV: unknown, focusV: unknown;
    for (const c of changes) {
        if (c.key === A11Y_KEY) { if (!c.deleted) a11yV = c.value; }
        else if (c.key === FOCUS_KEY) { if (!c.deleted) focusV = c.value; }
        else configPatch[c.key] = c.deleted ? undefined : c.value;
    }
    return { configPatch, a11y: a11yV, focus: focusV };
}

function extras(): Record<string, unknown> {
    const f = get(focusConfig);
    // never sync which apps are blocked — they may not exist on the other computer; the rest is preference
    const { blockedApps: _blocked, ...rest } = f;
    return { [A11Y_KEY]: get(a11y), [FOCUS_KEY]: rest };
}

function loadBaseline(): Snapshot | null {
    try { const raw = localStorage.getItem(BASELINE_KEY); return raw ? JSON.parse(raw) : null; } catch { return null; }
}
function saveBaseline(s: Snapshot) { try { localStorage.setItem(BASELINE_KEY, JSON.stringify(s)); } catch { /* ignore */ } }

let fields: string[] = [];

function currentSnapshot(): Snapshot {
    return collect(fields, configStore.get() as unknown as Record<string, unknown>, extras());
}

/** Reports local edits to the backend. Returns how many were real changes. */
export async function reportLocalChanges(force = false): Promise<number> {
    if (!SystemBridge.isTauri() || fields.length === 0) return 0;
    const current = currentSnapshot();
    const baseline = force ? {} : loadBaseline();
    if (baseline === null) { saveBaseline(current); return 0; } // first run: adopt, don't claim
    const changes = diff(baseline, current);
    if (Object.keys(changes).length === 0) return 0;
    const n = await SystemBridge.invoke<number>('sync_report_local', { changes });
    saveBaseline(current);
    return n;
}

/** Applies changes that arrived from another device to this computer. */
export async function applyRemote(changes: PendingChange[]): Promise<void> {
    if (changes.length === 0) return;
    const { configPatch, a11y: a, focus: f } = splitRemote(changes);
    if (a !== undefined) a11y.set(normalizeA11y(a));
    if (f !== undefined) setFocusConfig(normalizeConfig({ ...get(focusConfig), ...(f as object) }));
    if (Object.keys(configPatch).length) await configStore.save(configPatch as never);
    // what we just applied is now the baseline, so it is not reported back as our own edit
    saveBaseline(currentSnapshot());
}

export async function drainPending(): Promise<number> {
    if (!SystemBridge.isTauri()) return 0;
    const pending = await SystemBridge.invoke<PendingChange[]>('sync_take_pending');
    await applyRemote(pending);
    return pending.length;
}

/** Exchanges with one device now; applies whatever it brought. */
export async function syncNow(deviceId: string): Promise<number> {
    await reportLocalChanges();
    const changes = await SystemBridge.invoke<PendingChange[]>('sync_now', { deviceId });
    await applyRemote(changes);
    return changes.length;
}

/** "Make this computer win": reports every synced setting as a fresh edit, then syncs. */
export async function sendMySettings(deviceId: string): Promise<number> {
    await reportLocalChanges(true);
    return syncNow(deviceId);
}

/** Start-up: learn the allow-list, adopt the baseline, follow edits and remote events. */
export function startSettingsSync(): () => void {
    if (!SystemBridge.isTauri()) return () => {};
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unsubs: Array<() => void> = [];

    async function autoSync() {
        try {
            const st = await SystemBridge.invoke<{ peers: { deviceId: string; enabled: boolean }[] }>('sync_status');
            for (const p of st.peers.filter((x) => x.enabled)) await syncNow(p.deviceId).catch(() => {});
        } catch { /* offline peers are normal */ }
    }
    function schedule() {
        if (timer) clearTimeout(timer);
        timer = setTimeout(async () => { if (!stopped && (await reportLocalChanges()) > 0) void autoSync(); }, 3000);
    }

    (async () => {
        try {
            fields = (await SystemBridge.invoke<{ fields: string[] }>('sync_status')).fields;
            await reportLocalChanges();
            await drainPending();
            if (stopped) return;
            unsubs.push(configStore.subscribe(schedule));
            unsubs.push(a11y.subscribe(schedule));
            unsubs.push(focusConfig.subscribe(schedule));
            const { listen } = await import('@tauri-apps/api/event');
            unsubs.push(await listen('blue-sync://remote-changes', () => void drainPending()));
        } catch { /* backend without sync: nothing to do */ }
    })();

    return () => { stopped = true; if (timer) clearTimeout(timer); unsubs.forEach((u) => u()); };
}
