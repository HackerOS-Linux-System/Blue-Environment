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

import { classifyCoverage, sameRect } from './occlusion';

describe('classifyCoverage', () => {
  const page: Rect = { x: 100, y: 50, width: 800, height: 600 };

  it('is "none" when nothing overlaps, or only a border-thin sliver does', () => {
    expect(classifyCoverage(page, [])).toEqual({ kind: 'none' });
    expect(classifyCoverage(page, [{ x: 1000, y: 0, width: 300, height: 700 }])).toEqual({ kind: 'none' });
    expect(classifyCoverage(page, [{ x: 899, y: 0, width: 300, height: 700 }])).toEqual({ kind: 'none' });
  });

  it('cuts to the left half when a window covers the right half', () => {
    const c = classifyCoverage(page, [{ x: 500, y: 0, width: 600, height: 800 }]);
    expect(c.kind).toBe('partial');
    if (c.kind === 'partial') {
      expect(c.clip.x).toBe(100);
      expect(c.clip.y).toBe(50);
      expect(c.clip.height).toBe(600);
      expect(c.clip.width).toBeGreaterThanOrEqual(399);
      expect(c.clip.width).toBeLessThanOrEqual(401);
    }
  });

  it('keeps the largest free rectangle when a corner is covered (L-shaped remainder)', () => {
    const c = classifyCoverage(page, [{ x: 600, y: 400, width: 500, height: 500 }]);
    expect(c.kind).toBe('partial');
    if (c.kind === 'partial') {
      // Either the full-height left strip (500x600) or the full-width top strip (800x350): the top strip is bigger.
      expect(c.clip.width * c.clip.height).toBeGreaterThanOrEqual(500 * 600);
      expect(c.clip.x).toBe(100);
      expect(c.clip.y).toBe(50);
    }
  });

  it('is "full" when the page is completely covered', () => {
    expect(classifyCoverage(page, [{ x: 0, y: 0, width: 2000, height: 2000 }])).toEqual({ kind: 'full' });
  });

  it('is "full" when only a sliver would remain visible', () => {
    expect(classifyCoverage(page, [{ x: 120, y: 0, width: 2000, height: 2000 }])).toEqual({ kind: 'full' });
  });

  it('combines several covers (two side-by-side windows leave the gap between them)', () => {
    const c = classifyCoverage(page, [
      { x: 0, y: 0, width: 300, height: 800 },
      { x: 700, y: 0, width: 400, height: 800 },
    ]);
    expect(c.kind).toBe('partial');
    if (c.kind === 'partial') expect(c.clip.width).toBeGreaterThanOrEqual(398);
  });

  it('a clip never extends beyond the page', () => {
    const c = classifyCoverage(page, [{ x: 0, y: 0, width: 160, height: 5000 }]);
    expect(c.kind).toBe('partial');
    if (c.kind === 'partial') {
      expect(c.clip.x).toBeGreaterThanOrEqual(page.x);
      expect(c.clip.x + c.clip.width).toBeLessThanOrEqual(page.x + page.width);
      expect(c.clip.y + c.clip.height).toBeLessThanOrEqual(page.y + page.height);
    }
  });
});

describe('sameRect', () => {
  it('compares rectangles and nulls', () => {
    expect(sameRect(null, null)).toBe(true);
    expect(sameRect({ x: 1, y: 2, width: 3, height: 4 }, { x: 1, y: 2, width: 3, height: 4 })).toBe(true);
    expect(sameRect({ x: 1, y: 2, width: 3, height: 4 }, null)).toBe(false);
    expect(sameRect({ x: 1, y: 2, width: 3, height: 4 }, { x: 1, y: 2, width: 3, height: 5 })).toBe(false);
  });
});
