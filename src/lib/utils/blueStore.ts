import { SystemBridge } from './systemBridge';

export type PackageKind = 'app' | 'plugin' | 'theme';

/** Mirrors `BlueStore::error::StoreError` (src-tauri). */
export interface StoreError {
    code: string;
    message: string;
    hint?: string;
    detail?: string;
}

export function isStoreError(e: unknown): e is StoreError {
    return !!e && typeof e === 'object' && typeof (e as any).code === 'string' && typeof (e as any).message === 'string';
}

/** Best-effort human message out of anything a rejected invoke() can throw. */
export function storeErrorMessage(e: unknown): string {
    if (isStoreError(e)) return e.hint ? `${e.message} (${e.hint})` : e.message;
    if (e instanceof Error) return e.message;
    return String(e);
}

export interface StoreIndexEntry {
    name: string;
    description: string;
    author: string;
    icon: string;
    preview: string;
    downloadUrl: string;
}

export interface StoreIndexResult {
    kind: PackageKind;
    entries: StoreIndexEntry[];
    /** Entries present in the index but skipped (bad URL, missing name, …). */
    warnings: string[];
    sourceUrl: string;
}

export interface Manifest {
    id: string;
    name: string;
    kind: PackageKind;
    version: string;
    author: string;
    description: string;
    icon?: string;
    archive?: string;
    sha256?: string;
    homepage?: string;
    license?: string;
    category?: string;
    minBlueVersion?: string;
    entry: string;
    style?: string;
    width?: number;
    height?: number;
    minWidth?: number;
    minHeight?: number;
    pluginKind?: string;
    permissions: string[];
}

export interface ResolvedPackage {
    manifest: Manifest;
    manifestUrl: string;
    installedVersion?: string;
    updateAvailable: boolean;
}

/** Mirrors `BlueStore::install::Receipt` — what's written as
 * `.blue-install.json` inside every installed package. */
export interface Receipt {
    id: string;
    kind: PackageKind;
    name: string;
    version: string;
    author: string;
    description: string;
    icon?: string;
    category?: string;
    entry: string;
    style?: string;
    width?: number;
    height?: number;
    minWidth?: number;
    minHeight?: number;
    pluginKind?: string;
    permissions: string[];
    sourceUrl: string;
    sha256: string;
    installedAt: string;
}

export interface UpdateInfo {
    id: string;
    kind: PackageKind;
    installedVersion: string;
    latestVersion: string;
    manifestUrl: string;
}

export interface StoreProgress {
    opId: string;
    pct: number;
    message: string;
}

let nextOpId = 1;
function newOpId(): string {
    return `store-op-${Date.now()}-${nextOpId++}`;
}

/** The community index for one kind (apps-store.json / plugins-store.json /
 * themes-store.json), fetched fresh — no in-webview CORS/CSP dependency. */
export async function fetchStoreIndex(kind: PackageKind): Promise<StoreIndexResult> {
    return SystemBridge.invoke<StoreIndexResult>('store_fetch_index', { kind });
}

/** Fetches and validates a `blue.hk` (from a store entry's `downloadUrl`,
 * or a URL the person pasted in directly) without installing anything. */
export async function resolvePackage(url: string): Promise<ResolvedPackage> {
    return SystemBridge.invoke<ResolvedPackage>('store_resolve', { url });
}

/**
 * Downloads + installs the package behind `url`. Resolves once fully
 * installed; rejects with a {@link StoreError} on any failure (nothing is
 * left half-installed either way — see `install.rs`'s stage-then-rename
 * swap). `onProgress` is optional; percentages are 0–100 across the whole
 * operation (manifest → download → verify → privileged install).
 */
export async function installPackage(
    url: string,
    expectedKind: PackageKind | null,
    onProgress?: (pct: number, message: string) => void,
): Promise<Receipt> {
    const opId = newOpId();
    let unlisten: (() => void) | null = null;
    if (onProgress && SystemBridge.isTauri()) {
        const { listen } = await import('@tauri-apps/api/event');
        unlisten = await listen('store-progress', (evt: { payload: StoreProgress }) => {
            if (evt.payload.opId === opId) onProgress(evt.payload.pct, evt.payload.message);
        });
    }
    try {
        return await SystemBridge.invoke<Receipt>('store_install', { url, expectedKind: expectedKind ?? undefined, opId });
    } finally {
        unlisten?.();
    }
}

export async function uninstallPackage(kind: PackageKind, id: string): Promise<void> {
    await SystemBridge.invoke<void>('store_uninstall', { kind, id });
}

export async function listInstalled(kind: PackageKind): Promise<Receipt[]> {
    return SystemBridge.invoke<Receipt[]>('store_list_installed', { kind });
}

export async function checkUpdates(kind: PackageKind): Promise<UpdateInfo[]> {
    return SystemBridge.invoke<UpdateInfo[]>('store_check_updates', { kind });
}

/** UTF-8 text of a file inside an installed package (the JS entry, a
 * stylesheet, ...). Confined to that package's own directory. */
export async function readInstalledFile(kind: PackageKind, id: string, path: string): Promise<string> {
    return SystemBridge.invoke<string>('store_read_file', { kind, id, path });
}

/** An installed package's icon as a ready-to-use `data:` URL. */
export async function readInstalledIcon(kind: PackageKind, id: string, path: string): Promise<string> {
    return SystemBridge.invoke<string>('store_read_icon', { kind, id, path });
}

/** Subscribes to `store-changed`, fired after every install/uninstall
 * completes (successfully or not) — sections use this to refresh their
 * "installed" list without polling. Returns the unsubscribe function. */
export async function onStoreChanged(cb: () => void): Promise<() => void> {
    if (!SystemBridge.isTauri()) return () => {};
    const { listen } = await import('@tauri-apps/api/event');
    const un = await listen('store-changed', () => cb());
    return un;
}
