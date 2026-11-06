import { describe, it, expect, vi, beforeEach } from 'vitest';
import { writable } from 'svelte/store';

vi.mock('../../../utils/systemBridge', () => ({
    SystemBridge: { codeLinguistScan: vi.fn(), codeLinguistInitConfig: vi.fn() },
}));

import { SystemBridge } from '../../../utils/systemBridge';
import { groupShares, formatPercent, formatBytes, needsOutline, leadBrand, tooltip, createLinguist, type LanguageShare, type LinguistReport } from './linguist';
import { brandIcon, BRAND_ICONS } from './linguistBrands';

const share = (name: string, percent: number, extra: Partial<LanguageShare> = {}): LanguageShare => ({
    name, percent, bytes: Math.round(percent * 100), files: 1, color: '#336699', type: 'programming', custom: false, brand: null, ...extra,
});

const report = (languages: LanguageShare[]): LinguistReport => ({
    root: '/p', totalBytes: 100, countedFiles: languages.length, scannedFiles: languages.length, languages,
    excluded: { vendored: 0, generated: 0, documentation: 0, binary: 0, dataOrProse: 0, unknown: 0, tooLarge: 0 },
    truncated: false, source: 'git', elapsedMs: 3, customLanguages: 0,
});

describe('presentation helpers', () => {
    it('folds everything under 0.1% into "Other" like GitHub', () => {
        const g = groupShares([share('A', 99.85), share('B', 0.09), share('C', 0.06)]);
        expect(g.visible.map((s) => s.name)).toEqual(['A']);
        expect(g.other?.count).toBe(2);
        expect(g.other?.percent).toBeCloseTo(0.15);
        expect(groupShares([share('A', 60), share('B', 40)]).other).toBeNull();
    });

    it('formats percentages without a misleading 0.0%', () => {
        expect(formatPercent(44.386)).toBe('44.4%');
        expect(formatPercent(0.04)).toBe('<0.1%');
        expect(formatPercent(0)).toBe('0%');
        expect(formatPercent(NaN)).toBe('0%');
        expect(formatPercent(100)).toBe('100.0%');
    });

    it('formats byte sizes', () => {
        expect(formatBytes(512)).toBe('512 B');
        expect(formatBytes(2048)).toBe('2.0 KB');
        expect(formatBytes(50 * 1024)).toBe('50 KB');
        expect(formatBytes(3 * 1024 * 1024)).toBe('3.0 MB');
    });

    it('outlines swatches that vanish on a light or dark surface (hk is white, HackerScript is dark grey)', () => {
        expect(needsOutline('#ffffff')).toBe(true);   // hk
        expect(needsOutline('#000080')).toBe(true);   // Lua navy melts into the dark card
        expect(needsOutline('#000100')).toBe(true);   // Crystal
        expect(needsOutline('#fff')).toBe(true);
        expect(needsOutline('#9c1120')).toBe(false);  // H#
        expect(needsOutline('#8250df')).toBe(false);  // Hacker Lang
        expect(needsOutline('garbage')).toBe(false);
    });

    it('shows "leads this project" only when the top language has a project icon', () => {
        expect(leadBrand([share('H#', 70, { brand: 'hsharp', custom: true }), share('Rust', 30)])).toEqual({ name: 'H#', brand: 'hsharp' });
        expect(leadBrand([share('Rust', 70), share('H#', 30, { brand: 'hsharp' })])).toBeNull();
        expect(leadBrand([])).toBeNull();
    });

    it('builds tooltips with pluralised file counts', () => {
        expect(tooltip({ name: 'Rust', percent: 12.34, files: 1, bytes: 2048 })).toBe('Rust — 12.3% · 1 file · 2.0 KB');
        expect(tooltip({ percent: 5, files: 3, bytes: 10 })).toBe('5.0% · 3 files · 10 B');
    });

    it('has an embedded icon for each HackerOS brand and none for anything else', () => {
        for (const id of ['hsharp', 'hackerscript', 'hackerlang']) expect(brandIcon(id)).toMatch(/^data:image\/png;base64,/);
        expect(Object.keys(BRAND_ICONS)).toHaveLength(3);
        expect(brandIcon('rust')).toBeNull();
        expect(brandIcon(null)).toBeNull();
    });
});

describe('createLinguist store', () => {
    const scan = SystemBridge.codeLinguistScan as unknown as ReturnType<typeof vi.fn>;
    beforeEach(() => { vi.useFakeTimers(); scan.mockReset(); });

    it('scans when a project folder is opened and not while none is', async () => {
        const root = writable('');
        scan.mockResolvedValue(report([share('Rust', 100)]));
        const l = createLinguist(root);
        await vi.advanceTimersByTimeAsync(300);
        expect(scan).not.toHaveBeenCalled();
        root.set('/home/me/proj');
        await vi.advanceTimersByTimeAsync(300);
        expect(scan).toHaveBeenCalledWith('/home/me/proj');
        let value: LinguistReport | null = null;
        l.report.subscribe((v) => (value = v))();
        expect(value).not.toBeNull();
        l.destroy();
    });

    it('debounces bursts of saves into a single scan', async () => {
        const root = writable('/p');
        scan.mockResolvedValue(report([share('Rust', 100)]));
        const l = createLinguist(root);
        await vi.advanceTimersByTimeAsync(300);
        scan.mockClear();
        l.schedule(); l.schedule(); l.schedule();
        await vi.advanceTimersByTimeAsync(2000);
        expect(scan).toHaveBeenCalledTimes(1);
        l.destroy();
    });

    it('ignores a stale scan that finishes after a newer one', async () => {
        const root = writable('/a');
        let resolveFirst!: (r: LinguistReport) => void;
        scan.mockImplementationOnce(() => new Promise((r) => (resolveFirst = r)));
        const l = createLinguist(root);
        await vi.advanceTimersByTimeAsync(300);           // first scan in flight (/a)
        scan.mockResolvedValueOnce(report([share('Go', 100)]));
        root.set('/b');
        await vi.advanceTimersByTimeAsync(300);           // second scan (/b) completes
        resolveFirst(report([share('Rust', 100)]));        // late result for /a
        await vi.advanceTimersByTimeAsync(0);
        let value: LinguistReport | null = null;
        l.report.subscribe((v) => (value = v))();
        expect(value!.languages[0].name).toBe('Go');
        l.destroy();
    });

    it('reports errors and the browser-only "unavailable" state', async () => {
        const root = writable('/p');
        scan.mockRejectedValueOnce(new Error('boom'));
        const l = createLinguist(root);
        await vi.advanceTimersByTimeAsync(300);
        let st = ''; let err = '';
        l.status.subscribe((v) => (st = v))(); l.error.subscribe((v) => (err = v))();
        expect(st).toBe('error'); expect(err).toBe('boom');
        scan.mockResolvedValueOnce(null);
        await l.scan();
        l.status.subscribe((v) => (st = v))();
        expect(st).toBe('unavailable');
        l.destroy();
    });
});
