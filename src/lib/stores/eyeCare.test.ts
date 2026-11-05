import { describe, it, expect } from 'vitest';
import {
  DEFAULT_EYECARE, PRESETS, normalizeEyeCare, advance, freshRuntime, completeBreak, skipBreak, healthScore,
  inWindow, parseHHMM, lateFactor, escapeBurst, formatDuration, type EyeCareConfig,
} from './eyeCare';

const on = (patch: Partial<EyeCareConfig> = {}): EyeCareConfig => normalizeEyeCare({ ...DEFAULT_EYECARE, enabled: true, lateHours: false, ...patch });
const noon = new Date(2026, 0, 5, 12, 0, 0);
const nowMs = noon.getTime();

/** Runs `sec` seconds of continuous activity, returns the first event. */
function run(cfg: EyeCareConfig, sec: number, start = freshRuntime(noon)) {
  let rt = start;
  for (let i = 0; i < sec; i++) {
    const r = advance(rt, cfg, 1, 0, noon, nowMs);
    rt = r.rt;
    if (r.event) return { rt, event: r.event, at: i + 1 };
  }
  return { rt, event: null, at: sec };
}

describe('normalizeEyeCare', () => {
  it('returns defaults for garbage and clamps numbers', () => {
    expect(normalizeEyeCare(null)).toEqual(DEFAULT_EYECARE);
    const c = normalizeEyeCare({ lookAwayEveryMin: 9999, longBreakMin: -5, mode: 'weird', lateStart: '99:99' });
    expect(c.lookAwayEveryMin).toBe(60);
    expect(c.longBreakMin).toBe(1);
    expect(c.mode).toBe('gentle');
    expect(c.lateStart).toBe(DEFAULT_EYECARE.lateStart);
  });
  it('keeps the look-away shorter than its interval', () => {
    const c = normalizeEyeCare({ lookAwayEveryMin: 5, lookAwaySec: 120 });
    expect(c.lookAwaySec).toBeLessThan(5 * 60);
  });
  it('every preset is valid and survives normalisation unchanged', () => {
    for (const p of Object.values(PRESETS)) {
      const c = normalizeEyeCare({ ...DEFAULT_EYECARE, ...p });
      for (const [k, v] of Object.entries(p)) expect((c as any)[k]).toBe(v);
    }
  });
});

describe('advance', () => {
  it('does nothing while disabled', () => {
    const r = advance(freshRuntime(noon), { ...DEFAULT_EYECARE, enabled: false }, 1, 0, noon, nowMs);
    expect(r.event).toBeNull();
    expect(r.rt.activeSec).toBe(0);
  });
  it('fires the 20-20-20 look-away after the interval', () => {
    const cfg = on({ longBreak: false, blink: false, posture: false, stretch: false });
    const { event, at } = run(cfg, 20 * 60 + 5);
    expect(event).toEqual({ type: 'lookAway', seconds: 20 });
    expect(at).toBe(20 * 60);
  });
  it('a long break wins over a look-away that is due at the same time', () => {
    const cfg = on({ lookAwayEveryMin: 20, longBreakEveryMin: 20, blink: false, posture: false, stretch: false });
    expect(run(cfg, 20 * 60).event?.type).toBe('longBreak');
  });
  it('never stacks several reminders in one step', () => {
    const cfg = on({ rule2020: false, longBreak: false, blinkEveryMin: 10, postureEveryMin: 10 });
    const first = run(cfg, 10 * 60);
    expect(first.event?.type).toBe('blink');
    // posture is due too, but comes one tick later instead of together
    expect(advance(first.rt, cfg, 1, 0, noon, nowMs).event?.type).toBe('posture');
  });
  it('idle time is not counted as work, and a long idle resets the timers', () => {
    const cfg = on({ idleSec: 60 });
    let rt = { ...freshRuntime(noon), sinceLookAway: 1000, sinceLongBreak: 1000 };
    rt = advance(rt, cfg, 1, 25, noon, nowMs).rt; // short idle: still counts as "at the computer", but not away yet
    expect(rt.sinceLookAway).toBeGreaterThan(1000);
    const away = advance(rt, cfg, 1, 90, noon, nowMs).rt; // 90 s away ≥ 20 s look-away
    expect(away.sinceLookAway).toBe(0);
    expect(away.sinceLongBreak).toBe(rt.sinceLongBreak); // not long enough for a long break
    expect(away.activeSec).toBe(rt.activeSec);
    const gone = advance(rt, cfg, 1, 6 * 60, noon, nowMs).rt;
    expect(gone.sinceLongBreak).toBe(0);
  });
  it('snooze suppresses events but time still counts', () => {
    const cfg = on();
    const rt = { ...freshRuntime(noon), sinceLookAway: 5000, snoozeUntil: nowMs + 60_000 };
    const r = advance(rt, cfg, 1, 0, noon, nowMs);
    expect(r.event).toBeNull();
    expect(r.rt.activeSec).toBe(1);
  });
  it('warns once at the daily limit, then every 30 minutes', () => {
    const cfg = on({ rule2020: false, longBreak: false, blink: false, posture: false, stretch: false, dailyLimitMin: 60 });
    let rt = { ...freshRuntime(noon), activeSec: 3599 };
    let r = advance(rt, cfg, 1, 0, noon, nowMs);
    expect(r.event).toMatchObject({ type: 'limit', overMin: 0 });
    rt = r.rt;
    expect(advance(rt, cfg, 1, 0, noon, nowMs).event).toBeNull();
    rt = { ...rt, activeSec: rt.activeSec + 1800 };
    expect(advance(rt, cfg, 1, 0, noon, nowMs).event).toMatchObject({ type: 'limit', overMin: 30 });
  });
  it('starts a fresh day at midnight', () => {
    const cfg = on();
    const yesterday = { ...freshRuntime(new Date(2026, 0, 4)), activeSec: 9999, skipped: 3 };
    const r = advance(yesterday, cfg, 1, 0, noon, nowMs);
    expect(r.rt.day).toBe('2026-01-05');
    expect(r.rt.skipped).toBe(0);
    expect(r.rt.activeSec).toBeLessThan(10);
  });
  it('breaks come sooner late at night', () => {
    const cfg = normalizeEyeCare({ ...DEFAULT_EYECARE, enabled: true, lateHours: true, lateStart: '22:00', lateEnd: '06:00' });
    const night = new Date(2026, 0, 5, 23, 30);
    expect(lateFactor(cfg, night)).toBe(0.7);
    expect(lateFactor(cfg, noon)).toBe(1);
    const rt = { ...freshRuntime(night), sinceLookAway: 20 * 60 * 0.7 - 1 };
    expect(advance(rt, { ...cfg, longBreak: false }, 1, 0, night, night.getTime()).event?.type).toBe('lookAway');
  });
});

describe('stats', () => {
  it('completing a break resets timers and builds a streak', () => {
    const rt = completeBreak({ ...freshRuntime(noon), sinceLookAway: 900, sinceLongBreak: 900 }, 'lookAway');
    expect(rt.sinceLookAway).toBe(0);
    expect(rt.sinceLongBreak).toBe(900);
    expect(rt.lookAways).toBe(1);
    const lb = completeBreak(rt, 'longBreak');
    expect(lb.sinceLongBreak).toBe(0);
    expect(lb.streak).toBe(2);
    expect(lb.bestStreak).toBe(2);
  });
  it('skipping breaks the streak, counts and snoozes', () => {
    const rt = skipBreak({ ...freshRuntime(noon), streak: 4 }, nowMs, 5);
    expect(rt.streak).toBe(0);
    expect(rt.skipped).toBe(1);
    expect(rt.snoozeUntil).toBe(nowMs + 300_000);
  });
  it('health score reflects taken vs skipped breaks and an exceeded limit', () => {
    const cfg = on({ dailyLimitMin: 60 });
    expect(healthScore(freshRuntime(noon), cfg)).toBe(100);
    expect(healthScore({ ...freshRuntime(noon), lookAways: 3, skipped: 1 }, cfg)).toBe(75);
    expect(healthScore({ ...freshRuntime(noon), lookAways: 1, activeSec: 3600 + 1200 }, cfg)).toBe(90);
    expect(healthScore({ ...freshRuntime(noon), skipped: 5 }, cfg)).toBe(0);
  });
});

describe('helpers', () => {
  it('parses times and windows (also across midnight)', () => {
    expect(parseHHMM('07:30')).toBe(450);
    expect(parseHHMM('25:00')).toBeNull();
    expect(inWindow('22:00', '06:00', 23 * 60)).toBe(true);
    expect(inWindow('22:00', '06:00', 12 * 60)).toBe(false);
    expect(inWindow('10:00', '10:00', 600)).toBe(false);
  });
  it('escape ×3 within 1.5 s fires the strict-mode emergency exit', () => {
    let t: number[] = [];
    let r = escapeBurst(t, 1000); t = r.times; expect(r.fired).toBe(false);
    r = escapeBurst(t, 1400); t = r.times; expect(r.fired).toBe(false);
    r = escapeBurst(t, 1800); expect(r.fired).toBe(true);
    expect(escapeBurst([0, 100], 5000).fired).toBe(false); // too slow
  });
  it('formats durations', () => {
    expect(formatDuration(3 * 3600 + 125)).toEqual({ h: 3, m: 2 });
    expect(formatDuration(-5)).toEqual({ h: 0, m: 0 });
  });
});
