import { writable, get, type Readable } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';

/**
 * "Linguist": GitHub-style language statistics for the open project.
 * The scan itself runs in Rust (`code_linguist_scan`, see
 * src-tauri/src/BlueCodeApp/linguist.rs); this file holds the types, the
 * presentation helpers (pure, unit-tested) and the store that keeps the report
 * fresh.
 */

export type LangKind = 'programming' | 'markup' | 'data' | 'prose';

export interface LanguageShare {
    name: string;
    bytes: number;
    files: number;
    /** 0–100 */
    percent: number;
    /** #rrggbb */
    color: string;
    type: LangKind;
    /** HackerOS built-ins and languages declared in a languages.json. */
    custom: boolean;
    /** Project-icon id for H#, HackerScript and Hacker Lang. */
    brand: string | null;
}

export interface LinguistReport {
    root: string;
    totalBytes: number;
    countedFiles: number;
    scannedFiles: number;
    languages: LanguageShare[];
    excluded: { vendored: number; generated: number; documentation: number; binary: number; dataOrProse: number; unknown: number; tooLarge: number };
    truncated: boolean;
    source: 'git' | 'walk';
    elapsedMs: number;
    customLanguages: number;
}

export interface OtherShare { percent: number; bytes: number; files: number; count: number }

/** GitHub folds everything under 0.1% into "Other". */
export const OTHER_THRESHOLD = 0.1;

export function groupShares(shares: LanguageShare[], threshold = OTHER_THRESHOLD): { visible: LanguageShare[]; other: OtherShare | null } {
    const visible = shares.filter((s) => s.percent >= threshold);
    const rest = shares.filter((s) => s.percent < threshold);
    if (rest.length === 0) return { visible, other: null };
    return {
        visible,
        other: { percent: rest.reduce((a, s) => a + s.percent, 0), bytes: rest.reduce((a, s) => a + s.bytes, 0), files: rest.reduce((a, s) => a + s.files, 0), count: rest.length },
    };
}

/** "44.4%" — and "<0.1%" instead of a misleading "0.0%". */
export function formatPercent(p: number): string {
    if (!(p > 0)) return '0%';
    if (p < 0.1) return '<0.1%';
    return `${p.toFixed(1)}%`;
}

export function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function luminance(hex: string): number | null {
    let h = hex.trim().replace('#', '');
    if (h.length === 3) h = h.split('').map((c) => c + c).join('');
    if (!/^[0-9a-fA-F]{6}$/.test(h)) return null;
    const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/**
 * Near-white swatches (hk is #ffffff) and near-black ones (Crystal, Lua, the
 * dark grey of HackerScript) vanish on one theme or the other, so they get an outline.
 */
export function needsOutline(color: string): boolean {
    const l = luminance(color);
    return l === null ? false : l > 0.85 || l < 0.12;
}

/** "H# leads this project" — only when the biggest language has a project icon. */
export function leadBrand(shares: LanguageShare[]): { name: string; brand: string } | null {
    const top = shares[0];
    return top && top.brand ? { name: top.name, brand: top.brand } : null;
}

export function tooltip(s: { name?: string; bytes: number; files: number; percent: number }): string {
    return `${s.name ? s.name + ' — ' : ''}${formatPercent(s.percent)} · ${s.files} file${s.files === 1 ? '' : 's'} · ${formatBytes(s.bytes)}`;
}

export type LinguistStatus = 'idle' | 'scanning' | 'error' | 'unavailable';

export function createLinguist(rootPath: Readable<string>) {
    const report = writable<LinguistReport | null>(null);
    const status = writable<LinguistStatus>('idle');
    const error = writable('');
    let timer: ReturnType<typeof setTimeout> | undefined;
    let seq = 0;

    async function scan() {
        const root = get(rootPath);
        if (!root) { report.set(null); status.set('idle'); return; }
        const mine = ++seq;
        status.set('scanning');
        try {
            const r = await SystemBridge.codeLinguistScan(root);
            if (mine !== seq) return; // a newer scan superseded this one
            if (r === null) { status.set('unavailable'); return; }
            report.set(r);
            error.set('');
            status.set('idle');
        } catch (e) {
            if (mine !== seq) return;
            error.set(e instanceof Error ? e.message : String(e));
            status.set('error');
        }
    }

    /** Debounced rescan: saves, tree refreshes and window focus all funnel here. */
    function schedule(delay = 1500) {
        if (timer) clearTimeout(timer);
        timer = setTimeout(() => { timer = undefined; void scan(); }, delay);
    }

    /** Creates .blue/languages.json if needed and returns its path (to open in the editor). */
    async function initConfig(): Promise<string | null> {
        const root = get(rootPath);
        if (!root) return null;
        return SystemBridge.codeLinguistInitConfig(root);
    }

    const unsubscribe = rootPath.subscribe(() => schedule(150));

    return {
        report, status, error, scan, schedule, initConfig,
        destroy() { if (timer) clearTimeout(timer); unsubscribe(); },
    };
}

export type Linguist = ReturnType<typeof createLinguist>;
