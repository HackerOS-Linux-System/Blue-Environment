import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { DEFAULT_DND, inWindow, normalizeDnd, isDndActive, shouldShowToast, minutesLeft, dnd, setDnd, enableDndFor, disableDnd, toggleDnd, parseHHMM } from './doNotDisturb';

const at = (h: number, m = 0) => new Date(2026, 9, 16, h, m, 0);

describe('Do Not Disturb logic', () => {
  it('parses times strictly', () => {
    expect(parseHHMM('07:05')).toBe(425);
    expect(parseHHMM('7:05')).toBe(425);
    expect(parseHHMM('24:00')).toBeNull();
    expect(parseHHMM('ab')).toBeNull();
  });

  it('handles windows across midnight', () => {
    expect(inWindow('22:00', '07:00', 23 * 60)).toBe(true);
    expect(inWindow('22:00', '07:00', 3 * 60)).toBe(true);
    expect(inWindow('22:00', '07:00', 12 * 60)).toBe(false);
    expect(inWindow('22:00', '07:00', 7 * 60)).toBe(false); // end is exclusive
    expect(inWindow('09:00', '17:00', 12 * 60)).toBe(true);
    expect(inWindow('09:00', '09:00', 9 * 60)).toBe(false);
    expect(inWindow('bad', '09:00', 9 * 60)).toBe(false);
  });

  it('is active per mode', () => {
    expect(isDndActive({ ...DEFAULT_DND, mode: 'off' }, at(23))).toBe(false);
    expect(isDndActive({ ...DEFAULT_DND, mode: 'on' }, at(12))).toBe(true);
    const until = at(12).getTime() + 30 * 60000;
    expect(isDndActive({ ...DEFAULT_DND, mode: 'until', untilTs: until }, at(12, 29))).toBe(true);
    expect(isDndActive({ ...DEFAULT_DND, mode: 'until', untilTs: until }, at(12, 31))).toBe(false);
    expect(isDndActive({ ...DEFAULT_DND, mode: 'schedule' }, at(23))).toBe(true);
    expect(isDndActive({ ...DEFAULT_DND, mode: 'schedule' }, at(12))).toBe(false);
  });

  it('lets urgent notifications through only when allowed', () => {
    const on = { ...DEFAULT_DND, mode: 'on' as const };
    expect(shouldShowToast(on, false, at(12))).toBe(false);
    expect(shouldShowToast(on, true, at(12))).toBe(true);
    expect(shouldShowToast({ ...on, allowUrgent: false }, true, at(12))).toBe(false);
    expect(shouldShowToast(DEFAULT_DND, false, at(12))).toBe(true);
  });

  it('normalises corrupt state', () => {
    expect(normalizeDnd(null)).toEqual(DEFAULT_DND);
    expect(normalizeDnd({ mode: 'until' }).mode).toBe('off'); // until without a timestamp can't be active
    expect(normalizeDnd({ mode: 'bogus', scheduleStart: '99:99' })).toEqual(DEFAULT_DND);
  });

  it('reports minutes left', () => {
    const s = { ...DEFAULT_DND, mode: 'until' as const, untilTs: at(12).getTime() + 90_000 };
    expect(minutesLeft(s, at(12))).toBe(2);
    expect(minutesLeft(s, at(13))).toBeNull();
    expect(minutesLeft(DEFAULT_DND, at(12))).toBeNull();
  });
});

describe('Do Not Disturb store', () => {
  beforeEach(() => { disableDnd(); });
  it('enables for a duration, persists and toggles', () => {
    enableDndFor(60, at(12));
    expect(get(dnd).mode).toBe('until');
    expect(JSON.parse(localStorage.getItem('blue.dnd.v1')!).mode).toBe('until');
    setDnd({ mode: 'on', untilTs: null });
    toggleDnd(at(12));
    expect(get(dnd).mode).toBe('off');
    toggleDnd(at(12));
    expect(get(dnd).mode).toBe('on');
  });
});
