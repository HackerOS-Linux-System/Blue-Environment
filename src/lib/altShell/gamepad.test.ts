import { describe, it, expect } from 'vitest';
import { PadTracker, heldActions, REPEAT_DELAY_MS, REPEAT_INTERVAL_MS } from './gamepad';

const snap = (pressed: number[] = [], axes: number[] = [0, 0]) => {
  const buttons = Array.from({ length: 17 }, (_, i) => pressed.includes(i));
  return { buttons, axes };
};

describe('gamepad', () => {
  it('maps standard buttons and the left stick', () => {
    expect([...heldActions(snap([0, 12]))].sort()).toEqual(['select', 'up']);
    expect([...heldActions(snap([], [0.9, 0.1]))]).toEqual(['right']);
    expect([...heldActions(snap([], [-0.2, -0.9]))]).toEqual(['up']);
    expect([...heldActions(snap([], [0.3, 0.3]))]).toEqual([]); // inside the dead zone
    expect([...heldActions(snap([16]))]).toEqual(['home']);
  });

  it('fires once on press, not while merely held (non-directional)', () => {
    const t = new PadTracker();
    expect(t.step(0, snap([0]))).toEqual(['select']);
    expect(t.step(50, snap([0]))).toEqual([]);
    expect(t.step(5000, snap([0]))).toEqual([]);
    expect(t.step(5010, snap([]))).toEqual([]);
    expect(t.step(5020, snap([0]))).toEqual(['select']);
  });

  it('repeats held directions after a delay, at a steady rate', () => {
    const t = new PadTracker();
    expect(t.step(0, snap([15]))).toEqual(['right']);
    expect(t.step(REPEAT_DELAY_MS - 1, snap([15]))).toEqual([]);
    expect(t.step(REPEAT_DELAY_MS, snap([15]))).toEqual(['right']);
    expect(t.step(REPEAT_DELAY_MS + REPEAT_INTERVAL_MS - 1, snap([15]))).toEqual([]);
    expect(t.step(REPEAT_DELAY_MS + REPEAT_INTERVAL_MS, snap([15]))).toEqual(['right']);
  });

  it('releasing resets repeat state', () => {
    const t = new PadTracker();
    t.step(0, snap([14])); t.step(10, snap([]));
    expect(t.step(20, snap([14]))).toEqual(['left']);
  });
});
