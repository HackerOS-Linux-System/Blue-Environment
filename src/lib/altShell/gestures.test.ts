import { describe, it, expect } from 'vitest';
import { classifySwipe, pageAfterDrag, shadeTab } from './gestures';

describe('gestures', () => {
  it('classifies clear swipes and rejects short or diagonal ones', () => {
    expect(classifySwipe(120, 10)).toBe('right');
    expect(classifySwipe(-120, 10)).toBe('left');
    expect(classifySwipe(5, 140)).toBe('down');
    expect(classifySwipe(5, -140)).toBe('up');
    expect(classifySwipe(20, 10)).toBeNull();
    expect(classifySwipe(100, 90)).toBeNull();
  });

  it('turns pages on a far drag or a flick, clamped to the range', () => {
    expect(pageAfterDrag(1, 3, -200, 400, 600)).toBe(2);
    expect(pageAfterDrag(1, 3, 200, 400, 600)).toBe(0);
    expect(pageAfterDrag(1, 3, -60, 400, 600)).toBe(1); // slow + short → stays
    expect(pageAfterDrag(1, 3, -60, 400, 120)).toBe(2); // quick flick
    expect(pageAfterDrag(0, 3, 300, 400, 100)).toBe(0);
    expect(pageAfterDrag(2, 3, -300, 400, 100)).toBe(2);
    expect(pageAfterDrag(0, 0, -300, 400, 100)).toBe(0);
  });

  it('splits the shade by the side the pull started on', () => {
    expect(shadeTab(50, 400)).toBe('notifications');
    expect(shadeTab(350, 400)).toBe('quick');
  });
});
