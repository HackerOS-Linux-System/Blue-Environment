import { describe, it, expect, afterEach } from 'vitest';
import { createManagedStyle } from './managedStyle';

describe('managed style (fallback path, as in jsdom)', () => {
    afterEach(() => { document.head.innerHTML = ''; });
    it('creates one <style>, updates it in place, clears and removes it', () => {
        const s = createManagedStyle('t');
        s.set('a{color:red}');
        s.set('a{color:blue}');
        expect(document.querySelectorAll('style#t')).toHaveLength(1);
        expect(document.querySelector('style#t')!.textContent).toBe('a{color:blue}');
        s.clear();
        expect(document.querySelector('style#t')!.textContent).toBe('');
        s.remove();
        expect(document.querySelector('style#t')).toBeNull();
    });
});

describe('managed style (constructable stylesheet path)', () => {
    it('adopts one sheet, replaces its text, and detaches it again — without any <style> element', () => {
        const replaced: string[] = [];
        class FakeSheet { replaceSync(c: string) { replaced.push(c); } }
        const doc = { adoptedStyleSheets: [] as unknown[], createElement: () => { throw new Error('no <style> element may be created'); } } as unknown as Document;
        const g = globalThis as unknown as { CSSStyleSheet: unknown };
        const old = g.CSSStyleSheet;
        g.CSSStyleSheet = FakeSheet;
        const hadAdopted = 'adoptedStyleSheets' in document;
        if (!hadAdopted) Object.defineProperty(document, 'adoptedStyleSheets', { value: [], configurable: true, writable: true });
        try {
            const s = createManagedStyle('t', doc);
            s.set('x{}'); s.set('y{}');
            expect(doc.adoptedStyleSheets).toHaveLength(1);
            expect(replaced).toEqual(['x{}', 'y{}']);
            s.clear();
            expect(replaced.at(-1)).toBe('');
            s.remove();
            expect(doc.adoptedStyleSheets).toHaveLength(0);
        } finally {
            g.CSSStyleSheet = old;
            if (!hadAdopted) delete (document as unknown as Record<string, unknown>).adoptedStyleSheets;
        }
    });
});
