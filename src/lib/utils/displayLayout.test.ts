import { describe, it, expect } from 'vitest';
import { logicalSize, overlaps, anyOverlap, touches, isConnected, snapMove, normalize, arrangeRow, resolveOverlap, type Rect } from './displayLayout';

const r = (name: string, x: number, y: number, w = 1920, h = 1080): Rect => ({ name, x, y, w, h });

describe('displayLayout', () => {
  it('computes logical size with scale and rotation', () => {
    expect(logicalSize({ width: 3840, height: 2160, scale: 2, transform: 'normal' })).toEqual({ w: 1920, h: 1080 });
    expect(logicalSize({ width: 1920, height: 1080, scale: 1, transform: '90' })).toEqual({ w: 1080, h: 1920 });
    expect(logicalSize({ width: 1920, height: 1080, scale: 1, transform: 'flipped-270' })).toEqual({ w: 1080, h: 1920 });
    expect(logicalSize({ width: 1920, height: 1080, scale: 1, transform: 'flipped' })).toEqual({ w: 1920, h: 1080 });
    expect(logicalSize({ width: 1920, height: 1080, scale: 0, transform: 'normal' })).toEqual({ w: 1920, h: 1080 });
  });

  it('detects overlap, touching and connectivity', () => {
    expect(overlaps(r('a', 0, 0), r('b', 1000, 0))).toBe(true);
    expect(overlaps(r('a', 0, 0), r('b', 1920, 0))).toBe(false);
    expect(touches(r('a', 0, 0), r('b', 1920, 200))).toBe(true);
    expect(touches(r('a', 0, 0), r('b', 1920, 1080))).toBe(false); // only a corner
    expect(anyOverlap([r('a', 0, 0), r('b', 1920, 0), r('c', 100, 100)])).toBe(true);
    expect(isConnected([r('a', 0, 0), r('b', 1920, 0), r('c', 3840, 0)])).toBe(true);
    expect(isConnected([r('a', 0, 0), r('b', 5000, 0)])).toBe(false);
  });

  it('snaps to neighbouring edges within the threshold only', () => {
    const others = [r('a', 0, 0)];
    expect(snapMove(r('b', 1930, 10), others, 24)).toEqual({ x: 1920, y: 0 });
    expect(snapMove(r('b', 2100, 500), others, 24)).toEqual({ x: 2100, y: 500 });
    expect(snapMove(r('b', 1925, 1090), others, 24)).toEqual({ x: 1920, y: 1080 });
  });

  it('normalises and arranges', () => {
    expect(normalize([r('a', -100, 50), r('b', 1820, 50)])).toEqual([r('a', 0, 0), r('b', 1920, 0)]);
    expect(arrangeRow([r('a', 9, 9), r('b', 9, 9, 1000, 500)]).map((x) => [x.x, x.y])).toEqual([[0, 0], [1920, 0]]);
  });

  it('pushes an overlapping monitor to the nearest free edge', () => {
    const fixed = r('a', 0, 0);
    const moved = resolveOverlap(r('b', 1500, 100), [fixed]);
    expect(overlaps(moved, fixed)).toBe(false);
    expect(touches(moved, fixed)).toBe(true);
    const ok = r('c', 1920, 0);
    expect(resolveOverlap(ok, [fixed])).toBe(ok);
  });
});
