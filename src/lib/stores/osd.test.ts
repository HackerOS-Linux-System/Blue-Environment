import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  osd, showOsd, hideOsd, noteLocalChange, setOsdEnabled, currentOsd,
  volumeIconKind, brightnessIconKind, barFill, OSD_VISIBLE_MS, LOCAL_ECHO_MS,
} from './osd';

// Every test starts at a fresh instant so the module-level echo window of an
// earlier test can never leak into the next one.
let clock = 1_000_000;
beforeEach(() => { vi.useFakeTimers(); setOsdEnabled(true); hideOsd(); clock += 100_000; vi.setSystemTime(clock); });
afterEach(() => { vi.useRealTimers(); });

describe('showOsd', () => {
  it('shows the level and hides itself after the timeout', () => {
    showOsd('volume', 42);
    expect(currentOsd()).toMatchObject({ kind: 'volume', value: 42, muted: false, visible: true });
    vi.advanceTimersByTime(OSD_VISIBLE_MS - 1);
    expect(currentOsd().visible).toBe(true);
    vi.advanceTimersByTime(2);
    expect(currentOsd().visible).toBe(false);
  });

  it('a new change restarts the timer instead of hiding on the old one', () => {
    showOsd('volume', 10);
    vi.advanceTimersByTime(OSD_VISIBLE_MS - 200);
    showOsd('volume', 15);
    vi.advanceTimersByTime(OSD_VISIBLE_MS - 200);
    expect(currentOsd().visible).toBe(true);
    vi.advanceTimersByTime(300);
    expect(currentOsd().visible).toBe(false);
  });

  it('bumps seq on every show so the view can re-trigger', () => {
    showOsd('volume', 10); const a = currentOsd().seq;
    showOsd('volume', 11); expect(currentOsd().seq).toBe(a + 1);
  });

  it('clamps and rounds: volume up to 150, brightness up to 100, never negative', () => {
    showOsd('volume', 400); expect(currentOsd().value).toBe(150);
    showOsd('volume', -5); expect(currentOsd().value).toBe(0);
    showOsd('brightness', 400); expect(currentOsd().value).toBe(100);
    showOsd('volume', 33.6); expect(currentOsd().value).toBe(34);
    showOsd('volume', NaN); expect(currentOsd().value).toBe(0);
  });

  it('muted only applies to volume', () => {
    showOsd('volume', 50, true); expect(currentOsd().muted).toBe(true);
    showOsd('brightness', 50, true); expect(currentOsd().muted).toBe(false);
  });

  it('does nothing when disabled, and disabling hides a visible OSD', () => {
    showOsd('volume', 50);
    setOsdEnabled(false);
    expect(currentOsd().visible).toBe(false);
    showOsd('volume', 60);
    expect(currentOsd().visible).toBe(false);
  });
});

describe('local echo', () => {
  it('ignores the echo of a change the shell made itself, per kind', () => {
    noteLocalChange('volume');
    showOsd('volume', 30);
    expect(currentOsd().visible).toBe(false);
    showOsd('brightness', 30); // a different level is unaffected
    expect(currentOsd().visible).toBe(true);
  });

  it('stops ignoring after the echo window', () => {
    noteLocalChange('volume');
    vi.setSystemTime(clock + LOCAL_ECHO_MS + 1);
    showOsd('volume', 30);
    expect(currentOsd().visible).toBe(true);
  });

  it('force bypasses the filter (media key handled by the shell)', () => {
    noteLocalChange('volume');
    showOsd('volume', 30, false, true);
    expect(currentOsd().visible).toBe(true);
  });
});

describe('helpers', () => {
  it('picks the speaker glyph', () => {
    expect(volumeIconKind(0, false)).toBe('muted');
    expect(volumeIconKind(80, true)).toBe('muted');
    expect(volumeIconKind(10, false)).toBe('low');
    expect(volumeIconKind(50, false)).toBe('medium');
    expect(volumeIconKind(90, false)).toBe('high');
    expect(volumeIconKind(130, false)).toBe('high');
  });
  it('picks the sun glyph', () => {
    expect(brightnessIconKind(5)).toBe('dim');
    expect(brightnessIconKind(50)).toBe('medium');
    expect(brightnessIconKind(100)).toBe('full');
  });
  it('bar fill', () => {
    expect(barFill(40, false)).toBe(40);
    expect(barFill(140, false)).toBe(100);
    expect(barFill(40, true)).toBe(0);
  });
  it('store is subscribable', () => {
    let seen = 0; const un = osd.subscribe(() => seen++); showOsd('volume', 1); un();
    expect(seen).toBeGreaterThan(1);
  });
});
