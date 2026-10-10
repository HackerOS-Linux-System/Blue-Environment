import { describe, it, expect } from 'vitest';
import { rectsOverlap, windowsAbove, isCovered, type Rect, type WindowLayer } from './occlusion';

const r = (x: number, y: number, width: number, height: number): Rect => ({ x, y, width, height });
const w = (id: string, zIndex: number, extra: Partial<WindowLayer> = {}): WindowLayer => ({ id, zIndex, isMinimized: false, ...extra });

describe('rectsOverlap', () => {
  it('detects a real overlap (the screenshot: Files over the corner of Blue Web)', () => {
    expect(rectsOverlap(r(150, 220, 998, 555), r(955, 211, 818, 558))).toBe(true);
  });
  it('is false for separate rectangles', () => {
    expect(rectsOverlap(r(0, 0, 100, 100), r(200, 0, 100, 100))).toBe(false);
    expect(rectsOverlap(r(0, 0, 100, 100), r(0, 150, 100, 100))).toBe(false);
  });
  it('does not count rectangles that merely touch or share a border', () => {
    expect(rectsOverlap(r(0, 0, 100, 100), r(100, 0, 100, 100))).toBe(false);
    expect(rectsOverlap(r(0, 0, 100, 100), r(99, 0, 100, 100))).toBe(false); // 1px
  });
  it('counts containment', () => {
    expect(rectsOverlap(r(0, 0, 500, 500), r(100, 100, 50, 50))).toBe(true);
  });
});

describe('windowsAbove', () => {
  const self = w('web', 5);
  it('returns only visible windows with a higher z-index', () => {
    const above = windowsAbove(self, [w('a', 3), w('b', 9), w('c', 7, { isMinimized: true }), self]);
    expect(above.map((x) => x.id)).toEqual(['b']);
  });
  it('lets a PiP window float above everything', () => {
    expect(windowsAbove(w('web', 50), [w('pip', 1, { isPiP: true })]).map((x) => x.id)).toEqual(['pip']);
  });
  it('is never covered by something below a PiP window of its own kind', () => {
    expect(windowsAbove(w('pip', 1, { isPiP: true }), [w('normal', 99)])).toEqual([]);
  });
});

describe('isCovered', () => {
  it('is true when any cover overlaps, false otherwise', () => {
    const page = r(0, 0, 400, 300);
    expect(isCovered(page, [r(500, 500, 10, 10), r(350, 250, 100, 100)])).toBe(true);
    expect(isCovered(page, [r(500, 500, 10, 10)])).toBe(false);
    expect(isCovered(page, [])).toBe(false);
  });
});
