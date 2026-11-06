import { describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import { needsConsent, defaultSelection, isNew, requestPermissionConsent, consentRequest, type ConsentPreview } from './permissionConsent';

const perm = (id: string, risk: 'low' | 'medium' | 'high') => ({ id, risk, title: id, description: '' });
const preview = (o: Partial<ConsentPreview> = {}): ConsentPreview => ({
    manifest: { id: 'a', name: 'A', version: '1', author: 'x' },
    permissions: [perm('camera', 'high'), perm('network', 'medium'), perm('storage', 'low')],
    risk: 'high', newPermissions: ['camera', 'network', 'storage'], previousGranted: null, signature: { status: 'unsigned' }, ...o,
});

describe('permission consent', () => {
    it('is needed for a first install that asks for anything, never for a package that asks for nothing', () => {
        expect(needsConsent(preview())).toBe(true);
        expect(needsConsent(preview({ permissions: [], newPermissions: [] }))).toBe(false);
    });
    it('an update with nothing new keeps the old decision; a widening update asks again', () => {
        expect(needsConsent(preview({ previousGranted: ['network'], newPermissions: [] }))).toBe(false);
        expect(needsConsent(preview({ previousGranted: ['network'], newPermissions: ['camera'] }))).toBe(true);
    });
    it('pre-selects only low-risk permissions on a first install, and what was granted before on an update', () => {
        expect(defaultSelection(preview())).toEqual(['storage']);
        expect(defaultSelection(preview({ previousGranted: ['network'], newPermissions: ['camera'] })).sort()).toEqual(['network', 'storage']);
    });
    it('flags new permissions', () => {
        const p = preview({ newPermissions: ['camera'] });
        expect(isNew(p, 'camera')).toBe(true);
        expect(isNew(p, 'network')).toBe(false);
    });
    it('queues concurrent requests and resolves each with its own answer', async () => {
        const first = requestPermissionConsent(preview());
        const second = requestPermissionConsent(preview({ manifest: { id: 'b', name: 'B', version: '1', author: 'y' } }));
        await Promise.resolve(); await Promise.resolve();
        expect(get(consentRequest)?.preview.manifest.id).toBe('a');
        get(consentRequest)!.answer(['storage']);
        expect(await first).toEqual(['storage']);
        await new Promise((r) => setTimeout(r, 0));
        expect(get(consentRequest)?.preview.manifest.id).toBe('b');
        get(consentRequest)!.answer(null);
        expect(await second).toBeNull();
        expect(get(consentRequest)).toBeNull();
    });
});
