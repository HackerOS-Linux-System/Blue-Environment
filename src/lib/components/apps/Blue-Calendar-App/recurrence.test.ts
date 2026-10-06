import { describe, it, expect } from 'vitest';
import { expandEvent, occurrencesInRange, layoutDay, dayNum, fromDayNum, mondayIndex, weekdayCode, addDays, mondayOf, groupByDate } from './recurrence';
import { dueReminders, startMs, GRACE_MS } from './reminders';
import type { CalendarEvent, Occurrence } from './types';

const ev = (o: Partial<CalendarEvent> = {}): CalendarEvent => ({
  id: 'e', title: 'T', date: '2026-03-02', time: '10:00', durationMinutes: 60, description: '', color: '#3b82f6', ...o,
});
const rule = (r: Partial<NonNullable<CalendarEvent['recurrence']>>) => ({ freq: 'daily' as const, interval: 1, ...r });

describe('date helpers', () => {
  it('round-trips day numbers and finds weekdays', () => {
    expect(fromDayNum(dayNum('2026-03-02'))).toBe('2026-03-02');
    expect(weekdayCode('2026-03-02')).toBe('MO');           // 2 March 2026 is a Monday
    expect(weekdayCode('2026-03-08')).toBe('SU');
    expect(weekdayCode('1970-01-01')).toBe('TH');
    expect(mondayIndex(-1)).toBe(2);                         // 1969-12-31 = Wednesday
    expect(addDays('2026-02-28', 1)).toBe('2026-03-01');
    expect(addDays('2024-02-28', 1)).toBe('2024-02-29');
    expect(mondayOf('2026-03-08')).toBe('2026-03-02');
  });
  it('is immune to DST (no off-by-one across the spring/autumn switch)', () => {
    expect(addDays('2026-03-28', 2)).toBe('2026-03-30');
    expect(addDays('2026-10-24', 2)).toBe('2026-10-26');
  });
});

describe('expandEvent', () => {
  it('one-off events appear only inside the range', () => {
    expect(expandEvent(ev(), '2026-03-01', '2026-03-31')).toEqual(['2026-03-02']);
    expect(expandEvent(ev(), '2026-03-03', '2026-03-31')).toEqual([]);
  });
  it('daily with interval', () => {
    const e = ev({ recurrence: rule({ freq: 'daily', interval: 3 }) });
    expect(expandEvent(e, '2026-03-01', '2026-03-12')).toEqual(['2026-03-02', '2026-03-05', '2026-03-08', '2026-03-11']);
  });
  it('daily far in the past still lands on the right days (fast-forward is exact)', () => {
    const e = ev({ date: '2020-01-01', recurrence: rule({ freq: 'daily', interval: 7 }) });
    const got = expandEvent(e, '2026-03-01', '2026-03-31');
    for (const d of got) { expect((dayNum(d) - dayNum('2020-01-01')) % 7).toBe(0); expect(weekdayCode(d)).toBe('WE'); }
    expect(got).toEqual(['2026-03-04', '2026-03-11', '2026-03-18', '2026-03-25']); // 2020-01-01 was a Wednesday
  });
  it('weekly on several weekdays, starting mid-week', () => {
    const e = ev({ date: '2026-03-04', recurrence: rule({ freq: 'weekly', byDay: ['MO', 'WE', 'FR'] }) }); // Wed
    expect(expandEvent(e, '2026-03-01', '2026-03-14')).toEqual(['2026-03-04', '2026-03-06', '2026-03-09', '2026-03-11', '2026-03-13']);
  });
  it('weekly defaults to the start weekday and honours interval 2', () => {
    const e = ev({ recurrence: rule({ freq: 'weekly', interval: 2 }) }); // Mon 2 Mar
    expect(expandEvent(e, '2026-03-01', '2026-04-15')).toEqual(['2026-03-02', '2026-03-16', '2026-03-30', '2026-04-13']);
  });
  it('weekly: biweekly + several days keeps the "on" weeks aligned to the start week', () => {
    const e = ev({ recurrence: rule({ freq: 'weekly', interval: 2, byDay: ['MO', 'TH'] }) });
    expect(expandEvent(e, '2026-03-01', '2026-03-31')).toEqual(['2026-03-02', '2026-03-05', '2026-03-16', '2026-03-19', '2026-03-30']);
  });
  it('weekly fast-forward gives the same result as iterating from the start', () => {
    const base = ev({ date: '2025-01-06', recurrence: rule({ freq: 'weekly', interval: 3, byDay: ['MO', 'FR'] }) });
    const viaRange = expandEvent(base, '2026-03-01', '2026-03-31');
    const full = expandEvent(base, '2025-01-01', '2026-03-31').filter((d) => d >= '2026-03-01');
    expect(viaRange).toEqual(full);
  });
  it('monthly keeps the day number and SKIPS months that lack it (RFC 5545)', () => {
    const e = ev({ date: '2026-01-31', recurrence: rule({ freq: 'monthly' }) });
    expect(expandEvent(e, '2026-01-01', '2026-06-30')).toEqual(['2026-01-31', '2026-03-31', '2026-05-31']);
  });
  it('monthly with interval and across years', () => {
    const e = ev({ date: '2025-11-15', recurrence: rule({ freq: 'monthly', interval: 2 }) });
    expect(expandEvent(e, '2025-01-01', '2026-06-30')).toEqual(['2025-11-15', '2026-01-15', '2026-03-15', '2026-05-15']);
  });
  it('yearly, including Feb 29 only in leap years', () => {
    const e = ev({ date: '2024-02-29', recurrence: rule({ freq: 'yearly' }) });
    expect(expandEvent(e, '2024-01-01', '2032-12-31')).toEqual(['2024-02-29', '2028-02-29', '2032-02-29']);
    const b = ev({ date: '2026-07-04', recurrence: rule({ freq: 'yearly' }) });
    expect(expandEvent(b, '2026-01-01', '2028-12-31')).toEqual(['2026-07-04', '2027-07-04', '2028-07-04']);
  });
  it('count includes the first occurrence and is exact', () => {
    const e = ev({ recurrence: rule({ freq: 'daily', count: 3 }) });
    expect(expandEvent(e, '2026-01-01', '2026-12-31')).toEqual(['2026-03-02', '2026-03-03', '2026-03-04']);
    const w = ev({ recurrence: rule({ freq: 'weekly', byDay: ['MO', 'TU'], count: 3 }) });
    expect(expandEvent(w, '2026-01-01', '2026-12-31')).toEqual(['2026-03-02', '2026-03-03', '2026-03-09']);
  });
  it('count is measured from the START even when the range begins later', () => {
    const e = ev({ recurrence: rule({ freq: 'daily', count: 5 }) });
    expect(expandEvent(e, '2026-03-05', '2026-03-31')).toEqual(['2026-03-05', '2026-03-06']);
  });
  it('until is inclusive', () => {
    const e = ev({ recurrence: rule({ freq: 'daily', until: '2026-03-04' }) });
    expect(expandEvent(e, '2026-03-01', '2026-03-31')).toEqual(['2026-03-02', '2026-03-03', '2026-03-04']);
  });
  it('deleted occurrences disappear but still count towards COUNT', () => {
    const e = ev({ exdates: ['2026-03-03'], recurrence: rule({ freq: 'daily', count: 3 }) });
    expect(expandEvent(e, '2026-03-01', '2026-03-31')).toEqual(['2026-03-02', '2026-03-04']);
  });
  it('is bounded for absurd input', () => {
    const e = ev({ date: '1900-01-01', recurrence: rule({ freq: 'daily', count: 10_000_000 }) });
    expect(() => expandEvent(e, '2026-01-01', '2026-01-31')).not.toThrow();
    const bad = ev({ recurrence: rule({ freq: 'weekly', byDay: ['XX'] }) });
    expect(expandEvent(bad, '2026-03-01', '2026-03-31')).toContain('2026-03-02');
    const z = ev({ recurrence: rule({ freq: 'daily', interval: 0 }) });
    expect(expandEvent(z, '2026-03-02', '2026-03-04')).toHaveLength(3);
  });
});

describe('occurrencesInRange / grouping', () => {
  it('flattens, sorts all-day first then by time, and groups by day', () => {
    const a = ev({ id: 'a', time: '14:00' }), b = ev({ id: 'b', time: null }), c = ev({ id: 'c', time: '08:00', recurrence: rule({ freq: 'daily', count: 2 }) });
    const occ = occurrencesInRange([a, b, c], '2026-03-01', '2026-03-31');
    expect(occ.map((o) => o.key)).toEqual(['b@2026-03-02', 'c@2026-03-02', 'a@2026-03-02', 'c@2026-03-03']);
    expect(Object.keys(groupByDate(occ))).toEqual(['2026-03-02', '2026-03-03']);
  });
});

const occ = (id: string, time: string | null, dur: number | null = 60): Occurrence => ({ event: ev({ id, time, durationMinutes: dur }), date: '2026-03-02', key: `${id}@2026-03-02` });

describe('layoutDay', () => {
  it('puts non-overlapping events in one full-width lane', () => {
    const p = layoutDay([occ('a', '09:00'), occ('b', '11:00')]);
    expect(p.map((x) => [x.lane, x.lanes])).toEqual([[0, 1], [0, 1]]);
  });
  it('splits overlapping events into lanes that share the cluster width', () => {
    const p = layoutDay([occ('a', '09:00', 120), occ('b', '10:00', 60), occ('c', '10:30', 30)]);
    const by = Object.fromEntries(p.map((x) => [x.occ.event.id, x]));
    expect(by.a.lane).toBe(0); expect(by.b.lane).toBe(1); expect(by.c.lane).toBe(2);
    expect(p.every((x) => x.lanes === 3)).toBe(true);
  });
  it('reuses a free lane and keeps separate clusters independent', () => {
    const p = layoutDay([occ('a', '09:00', 60), occ('b', '09:30', 60), occ('c', '10:00', 30), occ('d', '15:00')]);
    const by = Object.fromEntries(p.map((x) => [x.occ.event.id, x]));
    expect(by.c.lane).toBe(0);            // lane 0 is free again at 10:00
    expect(by.a.lanes).toBe(2);
    expect(by.d.lanes).toBe(1);
  });
  it('ignores all-day events, clamps to the day and enforces a minimum height', () => {
    const p = layoutDay([occ('allday', null), occ('late', '23:50', 120), occ('tiny', '08:00', 5), occ('nodur', '12:00', null)]);
    expect(p).toHaveLength(3);
    const late = p.find((x) => x.occ.event.id === 'late')!;
    expect(late.endMin).toBeLessThanOrEqual(24 * 60);
    expect(p.find((x) => x.occ.event.id === 'tiny')!.endMin - p.find((x) => x.occ.event.id === 'tiny')!.startMin).toBe(20);
    expect(p.find((x) => x.occ.event.id === 'nodur')!.endMin - 12 * 60).toBe(60);
  });
});

describe('dueReminders', () => {
  const at = (iso: string, hm: string) => { const [y, m, d] = iso.split('-').map(Number); const [h, mi] = hm.split(':').map(Number); return new Date(y, m - 1, d, h, mi, 0, 0); };
  const e = ev({ id: 'r', time: '10:00', reminderMinutes: 15 });

  it('fires once the fire time has passed, not before', () => {
    expect(dueReminders([e], at('2026-03-02', '09:44'), new Set())).toHaveLength(0);
    const due = dueReminders([e], at('2026-03-02', '09:45'), new Set());
    expect(due).toHaveLength(1);
    expect(due[0].key).toBe('r@2026-03-02');
    expect(due[0].startMs).toBe(startMs('2026-03-02', '10:00'));
  });
  it('does not fire twice', () => {
    expect(dueReminders([e], at('2026-03-02', '09:50'), new Set(['r@2026-03-02']))).toHaveLength(0);
  });
  it('shows a reminder missed for a few minutes but drops stale ones', () => {
    expect(dueReminders([e], new Date(at('2026-03-02', '09:45').getTime() + GRACE_MS - 1000), new Set())).toHaveLength(1);
    expect(dueReminders([e], new Date(at('2026-03-02', '09:45').getTime() + GRACE_MS + 1000), new Set())).toHaveLength(0);
  });
  it('0 = at the start; null/undefined = no reminder', () => {
    expect(dueReminders([ev({ reminderMinutes: 0 })], at('2026-03-02', '10:00'), new Set())).toHaveLength(1);
    expect(dueReminders([ev({ reminderMinutes: null })], at('2026-03-02', '10:00'), new Set())).toHaveLength(0);
    expect(dueReminders([ev()], at('2026-03-02', '10:00'), new Set())).toHaveLength(0);
  });
  it('all-day events remind relative to 09:00; "1 day before" fires the previous morning', () => {
    const allDay = ev({ id: 'ad', time: null, reminderMinutes: 1440 });
    expect(dueReminders([allDay], at('2026-03-01', '09:00'), new Set())).toHaveLength(1);
    expect(dueReminders([allDay], at('2026-03-01', '08:59'), new Set())).toHaveLength(0);
  });
  it('a reminder offset that crosses midnight lands on the right (earlier) day', () => {
    const early = ev({ id: 'm', time: '00:30', reminderMinutes: 60 });
    const due = dueReminders([early], at('2026-03-01', '23:30'), new Set());
    expect(due.map((d) => d.key)).toEqual(['m@2026-03-02']);
  });
  it('works for recurring events and respects deleted occurrences', () => {
    const rec = ev({ id: 'w', recurrence: rule({ freq: 'daily' }), exdates: ['2026-03-03'], reminderMinutes: 5 });
    expect(dueReminders([rec], at('2026-03-03', '09:56'), new Set())).toHaveLength(0);
    expect(dueReminders([rec], at('2026-03-04', '09:56'), new Set()).map((d) => d.key)).toEqual(['w@2026-03-04']);
  });
});
