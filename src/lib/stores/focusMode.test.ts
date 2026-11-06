import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import type { CalendarEvent } from '../components/apps/Blue-Calendar-App/types';
import { DEFAULT_DND, dnd, setDnd, type DndState } from './doNotDisturb';
import {
  DEFAULT_FOCUS_CONFIG, isFocusEvent, occurrenceSpan, activeCalendarFocus, nextCalendarFocus, pomodoroSession, pomodoroPhase, computeFocus,
  blockedLaunch, endNeedsPin, dndTransition, normalizeConfig, INACTIVE, startFocus, startPomodoro, stopFocus, evaluate, focusStatus, checkFocusBlock, setFocusConfig, type FocusConfig, type FocusStatus,
} from './focusMode';

const ev = (p: Partial<CalendarEvent>): CalendarEvent => ({ id: 'e', title: 'Focus: report', date: '2026-11-02', time: '09:00', durationMinutes: 60, description: '', color: 'blue', ...p });
const at = (iso: string, hhmm: string) => { const [y, m, d] = iso.split('-').map(Number); const [h, mi] = hhmm.split(':').map(Number); return new Date(y, m - 1, d, h, mi).getTime(); };
const KW = ['focus', 'deep work', '🎯'];
const cfg = (p: Partial<FocusConfig> = {}): FocusConfig => ({ ...DEFAULT_FOCUS_CONFIG, ...p });

describe('calendar focus blocks', () => {
  it('recognises keywords (any case) and #focus in the description', () => {
    expect(isFocusEvent('FOCUS time', '', KW)).toBe(true);
    expect(isFocusEvent('Deep Work: design', '', KW)).toBe(true);
    expect(isFocusEvent('🎯 Ship it', '', KW)).toBe(true);
    expect(isFocusEvent('Write', 'plan #focus', KW)).toBe(true);
    expect(isFocusEvent('Lunch with Anna', 'at the café', KW)).toBe(false);
    expect(isFocusEvent('Unfocused chat', '', ['focus'])).toBe(true); // substring match is intentional and documented
    expect(isFocusEvent('anything', '#focused', KW)).toBe(false);
    expect(isFocusEvent('x', '', [''])).toBe(false);
  });

  it('is active inside the event and not at its edges', () => {
    const e = [ev({})];
    expect(activeCalendarFocus(e, at('2026-11-02', '08:59'), KW)).toBeNull();
    expect(activeCalendarFocus(e, at('2026-11-02', '09:00'), KW)?.title).toBe('Focus: report');
    expect(activeCalendarFocus(e, at('2026-11-02', '09:59'), KW)).not.toBeNull();
    expect(activeCalendarFocus(e, at('2026-11-02', '10:00'), KW)).toBeNull();
  });

  it('handles recurring blocks and exdates', () => {
    const weekly = ev({ date: '2026-10-26', recurrence: { freq: 'weekly', interval: 1, byDay: ['MO'] } as any, exdates: ['2026-11-09'] });
    expect(activeCalendarFocus([weekly], at('2026-11-02', '09:30'), KW)).not.toBeNull();
    expect(activeCalendarFocus([weekly], at('2026-11-09', '09:30'), KW)).toBeNull();
    expect(activeCalendarFocus([weekly], at('2026-11-16', '09:30'), KW)).not.toBeNull();
  });

  it('a block that crosses midnight is still active after it', () => {
    const late = ev({ time: '23:30', durationMinutes: 120 });
    expect(activeCalendarFocus([late], at('2026-11-03', '00:30'), KW)).not.toBeNull();
    expect(activeCalendarFocus([late], at('2026-11-03', '01:30'), KW)).toBeNull();
  });

  it('all-day focus events cover the whole day; back-to-back blocks merge their end', () => {
    expect(activeCalendarFocus([ev({ time: null, durationMinutes: null })], at('2026-11-02', '17:00'), KW)).not.toBeNull();
    const a = ev({ id: 'a', time: '09:00', durationMinutes: 60 });
    const b = ev({ id: 'b', time: '10:00', durationMinutes: 30 });
    expect(activeCalendarFocus([a, b], at('2026-11-02', '09:30'), KW)?.end).toBe(at('2026-11-02', '10:30'));
  });

  it('finds the next block', () => {
    const n = nextCalendarFocus([ev({})], at('2026-11-02', '07:00'), KW);
    expect(n?.start).toBe(at('2026-11-02', '09:00'));
    expect(nextCalendarFocus([ev({})], at('2026-11-02', '12:00'), KW)).toBeNull();
    expect(occurrenceSpan(ev({ durationMinutes: null }), '2026-11-02').end - occurrenceSpan(ev({ durationMinutes: null }), '2026-11-02').start).toBe(3_600_000);
  });
});

describe('pomodoro', () => {
  const t0 = at('2026-11-02', '10:00');
  const s = pomodoroSession(25, 5, 2, t0);
  it('alternates work and break and has no trailing break', () => {
    expect(s.endsAt).toBe(t0 + 25 * 60_000 * 2 + 5 * 60_000);
    expect(pomodoroPhase(s, t0 + 60_000)).toMatchObject({ phase: 'work', cycle: 1 });
    expect(pomodoroPhase(s, t0 + 26 * 60_000)).toMatchObject({ phase: 'break', cycle: 1, phaseEndsAt: t0 + 30 * 60_000 });
    expect(pomodoroPhase(s, t0 + 31 * 60_000)).toMatchObject({ phase: 'work', cycle: 2 });
    expect(pomodoroPhase(s, t0 + 56 * 60_000)).toBeNull();
    expect(pomodoroPhase(s, t0 - 1)).toBeNull();
  });
  it('clamps silly input', () => {
    const z = pomodoroSession(0, -5, 0, t0);
    expect(z.workMs).toBe(60_000); expect(z.breakMs).toBe(0); expect(z.endsAt).toBe(t0 + 60_000);
  });
});

describe('combined status', () => {
  const now = at('2026-11-02', '09:30');
  it('manual beats calendar; calendar is the fallback; both can be switched off', () => {
    const manual = { source: 'manual' as const, startedAt: now - 1000, endsAt: now + 600_000 };
    expect(computeFocus(cfg(), manual, [ev({})], now)).toMatchObject({ active: true, source: 'manual' });
    expect(computeFocus(cfg(), null, [ev({})], now)).toMatchObject({ active: true, source: 'calendar', label: 'Focus: report' });
    expect(computeFocus(cfg({ calendarAuto: false }), null, [ev({})], now)).toEqual(INACTIVE);
    expect(computeFocus(cfg(), { ...manual, endsAt: now - 1 }, [], now)).toEqual(INACTIVE);
  });
  it('a pomodoro break is not focus', () => {
    const p = pomodoroSession(25, 5, 2, now - 26 * 60_000);
    expect(computeFocus(cfg({ calendarAuto: false }), p, [], now)).toMatchObject({ active: false, phase: 'break', source: 'pomodoro' });
  });
});

describe('blocking and the PIN', () => {
  const on: FocusStatus = { active: true, source: 'manual', endsAt: at('2026-11-02', '10:15'), label: 'Focus', phase: 'work' };
  it('blocks only listed apps, only while active, and never Settings', () => {
    const c = cfg({ blockedApps: ['discord', 'settings'] });
    expect(blockedLaunch('discord', c, on)).toContain('until 10:15');
    expect(blockedLaunch('blueCode', c, on)).toBeNull();
    expect(blockedLaunch('settings', c, on)).toBeNull();
    expect(blockedLaunch('discord', c, INACTIVE)).toBeNull();
  });
  it('strict mode asks for the Parental Controls PIN only when one exists and the session is manual/pomodoro', () => {
    expect(endNeedsPin(cfg({ strict: true }), on, true)).toBe(true);
    expect(endNeedsPin(cfg({ strict: true }), on, false)).toBe(false);
    expect(endNeedsPin(cfg({ strict: false }), on, true)).toBe(false);
    expect(endNeedsPin(cfg({ strict: true }), { ...on, source: 'calendar' }, true)).toBe(false);
  });
});

describe('DND coupling', () => {
  const mine: DndState = { ...DEFAULT_DND, mode: 'schedule' };
  const forced: DndState = { ...DEFAULT_DND, mode: 'on' };
  it('enables on start, restores on end, respects a manual change in between', () => {
    expect(dndTransition(false, true, mine, null)).toEqual({ type: 'enable', save: mine });
    expect(dndTransition(true, false, forced, mine)).toEqual({ type: 'restore', to: mine });
    expect(dndTransition(true, false, { ...DEFAULT_DND, mode: 'off' }, mine)).toEqual({ type: 'none' });
    expect(dndTransition(true, true, forced, mine)).toEqual({ type: 'none' });
    expect(dndTransition(false, false, mine, null)).toEqual({ type: 'none' });
  });
});

describe('config normalisation', () => {
  it('repairs corrupt stored data', () => {
    const c = normalizeConfig({ workMinutes: 9999, breakMinutes: -3, keywords: ['ok', 5, ''], blockedApps: 'x', strict: 'yes' });
    expect(c.workMinutes).toBe(180); expect(c.breakMinutes).toBe(0); expect(c.keywords).toEqual(['ok']); expect(c.blockedApps).toEqual([]); expect(c.strict).toBe(false);
    expect(normalizeConfig(null)).toEqual(DEFAULT_FOCUS_CONFIG);
  });
});

describe('store: a manual session drives DND and blocking end to end', () => {
  beforeEach(() => { localStorage.clear(); focusStatus.set(INACTIVE); setDnd({ ...DEFAULT_DND }); setFocusConfig({ blockedApps: ['discord'], strict: false, calendarAuto: false }); vi.useRealTimers(); });

  it('start → DND on + app blocked; stop → DND restored + app allowed', async () => {
    setDnd({ mode: 'schedule' });
    startFocus(30);
    evaluate([]);
    expect(get(focusStatus).active).toBe(true);
    expect(get(dnd).mode).toBe('on');
    expect(checkFocusBlock('discord')).toContain('Focus mode is on');
    expect(await stopFocus({ pinIsSet: false, verifyPin: async () => true })).toBe(true);
    evaluate([]);
    expect(get(focusStatus).active).toBe(false);
    expect(get(dnd).mode).toBe('schedule');
    expect(checkFocusBlock('discord')).toBeNull();
  });

  it('strict + PIN: wrong or missing PIN keeps the session going', async () => {
    setFocusConfig({ strict: true });
    startFocus(30); evaluate([]);
    expect(await stopFocus({ pinIsSet: true, verifyPin: async () => false })).toBe(false);
    expect(await stopFocus({ pinIsSet: true, verifyPin: async () => false, pin: '0000' })).toBe(false);
    evaluate([]);
    expect(get(focusStatus).active).toBe(true);
    expect(await stopFocus({ pinIsSet: true, verifyPin: async (p) => p === '1234', pin: '1234' })).toBe(true);
  });

  it('a pomodoro break lets DND go and the work phase turns it back on', () => {
    startPomodoro(2, 1_000_000);
    evaluate([], 1_000_000 + 60_000);            // work
    expect(get(dnd).mode).toBe('on');
    evaluate([], 1_000_000 + 26 * 60_000);       // break
    expect(get(dnd).mode).toBe('off');
    evaluate([], 1_000_000 + 31 * 60_000);       // work again
    expect(get(dnd).mode).toBe('on');
  });
});
