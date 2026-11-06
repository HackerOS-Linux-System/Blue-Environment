import { describe, it, expect } from 'vitest';
import { collect, diff, splitRemote } from './settingsSync';

describe('settings sync (frontend)', () => {
    const fields = ['theme', 'language', 'hotCorners', 'store:a11y'];

    it('collects only allow-listed fields and routes store:* keys to the extras', () => {
        const s = collect(fields, { theme: 'dark', language: 'pl', accounts: { secret: 1 }, aiConfig: { key: 'sk' }, wallpaper: '/x.png' }, { 'store:a11y': { textScale: 1.2 } });
        expect(s).toEqual({ theme: 'dark', language: 'pl', 'store:a11y': { textScale: 1.2 } });
        expect(JSON.stringify(s)).not.toContain('secret');
        expect(JSON.stringify(s)).not.toContain('sk');
    });

    it('reports nothing when nothing changed and only the changed keys otherwise', () => {
        const base = { theme: 'dark', hotCorners: { a: 1 } };
        expect(diff(base, { theme: 'dark', hotCorners: { a: 1 } })).toEqual({});
        expect(diff(base, { theme: 'light', hotCorners: { a: 1 } })).toEqual({ theme: 'light' });
        expect(diff(base, { theme: 'dark', hotCorners: { a: 2 } })).toEqual({ hotCorners: { a: 2 } });
    });

    it('reports additions and deletions', () => {
        expect(diff({ theme: 'dark' }, { theme: 'dark', language: 'de' })).toEqual({ language: 'de' });
        expect(diff({ theme: 'dark', language: 'de' }, { theme: 'dark' })).toEqual({ language: null });
    });

    it('a value applied from a remote device becomes the baseline and so produces no diff (no echo)', () => {
        const afterApply = { theme: 'light', language: 'pl' };
        expect(diff(afterApply, { ...afterApply })).toEqual({});
    });

    it('splits remote changes into a config patch and store extras; deletions unset the field', () => {
        const r = splitRemote([
            { key: 'theme', value: 'light', deleted: false },
            { key: 'language', value: null, deleted: true },
            { key: 'store:a11y', value: { textScale: 1.3 }, deleted: false },
            { key: 'store:focus', value: { strict: true }, deleted: false },
        ]);
        expect(r.configPatch).toEqual({ theme: 'light', language: undefined });
        expect(r.a11y).toEqual({ textScale: 1.3 });
        expect(r.focus).toEqual({ strict: true });
    });
});
