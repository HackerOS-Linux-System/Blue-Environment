import { describe, it, expect } from 'vitest';
import { cornerAt, normalizeHotCorners, DEFAULT_HOT_CORNERS } from './hotCorners';

describe('hot corners', () => {
  it('detects all four corners and nothing in the middle', () => {
    expect(cornerAt(0, 0, 1920, 1080, 6)).toBe('topLeft');
    expect(cornerAt(1919, 0, 1920, 1080, 6)).toBe('topRight');
    expect(cornerAt(0, 1079, 1920, 1080, 6)).toBe('bottomLeft');
    expect(cornerAt(1919, 1079, 1920, 1080, 6)).toBe('bottomRight');
    expect(cornerAt(960, 540, 1920, 1080, 6)).toBeNull();
    expect(cornerAt(960, 0, 1920, 1080, 6)).toBeNull(); // sama krawędź to nie róg
  });
  it('is disabled by default and sanitizes garbage', () => {
    expect(DEFAULT_HOT_CORNERS.enabled).toBe(false);
    const n = normalizeHotCorners({ triggerSize: 9999, dwellMs: -5, corners: { topLeft: { action: 'rm -rf' as any } } as any });
    expect(n.triggerSize).toBe(40);
    expect(n.dwellMs).toBe(0);
    expect(n.corners.topLeft.action).toBe('fullscreen-menu');
  });
});
