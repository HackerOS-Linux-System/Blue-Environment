import { describe, it, expect } from 'vitest';
import { COMPACT_MAX_WIDTH, isCompactWidth } from './compact';

describe('isCompactWidth', () => {
  it('is compact only below the breakpoint', () => {
    expect(isCompactWidth(360)).toBe(true);
    expect(isCompactWidth(COMPACT_MAX_WIDTH - 1)).toBe(true);
    expect(isCompactWidth(COMPACT_MAX_WIDTH)).toBe(false);
    expect(isCompactWidth(1280)).toBe(false);
  });
  it('treats an unmeasured width (0) as not compact, so the first paint is the desktop layout', () => {
    expect(isCompactWidth(0)).toBe(false);
    expect(isCompactWidth(-5)).toBe(false);
  });
  it('honours a custom breakpoint', () => {
    expect(isCompactWidth(500, 480)).toBe(false);
    expect(isCompactWidth(400, 480)).toBe(true);
  });
});
