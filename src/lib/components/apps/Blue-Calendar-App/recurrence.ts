import type { CalendarEvent, Occurrence, RecurrenceRule } from './types';
import { WEEKDAY_CODES } from './types';

const DAY_MS = 86_400_000;
const MAX_STEPS = 20_000; // hard stop against absurd rules

export function parseIso(iso: string): { y: number; m: number; d: number } {
  const [y, m, d] = iso.split('-').map(Number);
  return { y, m, d };
}
export const dayNum = (iso: string): number => { const { y, m, d } = parseIso(iso); return Math.floor(Date.UTC(y, m - 1, d) / DAY_MS); };
export function fromDayNum(n: number): string {
  const dt = new Date(n * DAY_MS);
  return `${dt.getUTCFullYear()}-${String(dt.getUTCMonth() + 1).padStart(2, '0')}-${String(dt.getUTCDate()).padStart(2, '0')}`;
}
export const addDays = (iso: string, n: number): string => fromDayNum(dayNum(iso) + n);
/** 0 = Monday … 6 = Sunday. (Day 0 — 1970-01-01 — was a Thursday.) */
export const mondayIndex = (dn: number): number => (((dn + 3) % 7) + 7) % 7;
export const weekdayCode = (iso: string): string => WEEKDAY_CODES[mondayIndex(dayNum(iso))];
export const daysInMonth = (y: number, m: number): number => new Date(Date.UTC(y, m, 0)).getUTCDate();
export const mondayOf = (iso: string): string => fromDayNum(dayNum(iso) - mondayIndex(dayNum(iso)));

/** ISO dates on which `ev` occurs within [fromIso, toIso] (inclusive), exceptions removed. */
export function expandEvent(ev: CalendarEvent, fromIso: string, toIso: string): string[] {
  const from = dayNum(fromIso), to = dayNum(toIso);
  const start = dayNum(ev.date);
  const skip = new Set(ev.exdates ?? []);
  const out: string[] = [];
  const rule = ev.recurrence;
  const push = (dn: number) => { const iso = fromDayNum(dn); if (dn >= from && dn <= to && !skip.has(iso)) out.push(iso); };

  if (!rule) { push(start); return out; }
  const interval = Math.max(1, Math.floor(rule.interval || 1));
  const until = rule.until ? dayNum(rule.until) : Infinity;
  const limit = Math.min(to, until);
  const count = rule.count && rule.count > 0 ? rule.count : Infinity;
  const noCount = count === Infinity;
  let produced = 0;
  // returns false once nothing further can fall inside the range
  const take = (dn: number): boolean => {
    if (dn > limit) return false;
    if (produced >= count) return false;
    produced++;
    push(dn);
    return true;
  };

  if (rule.freq === 'daily') {
    // without COUNT we may jump straight to the first candidate in range
    let n = noCount && from > start ? Math.ceil((from - start) / interval) : 0;
    for (let steps = 0; steps < MAX_STEPS; steps++, n++) if (!take(start + n * interval)) break;
  } else if (rule.freq === 'weekly') {
    const startMon = start - mondayIndex(start);
    const codes = rule.byDay && rule.byDay.length ? rule.byDay : [WEEKDAY_CODES[mondayIndex(start)]];
    const idxs = [...new Set(codes.map((c) => WEEKDAY_CODES.indexOf(c as any)).filter((i) => i >= 0))].sort((a, b) => a - b);
    if (!idxs.length) idxs.push(mondayIndex(start));
    let w = noCount && from > start ? Math.max(0, Math.floor((from - startMon) / (7 * interval)) - 1) : 0;
    outer: for (let steps = 0; steps < MAX_STEPS; steps++, w++) {
      const weekStart = startMon + w * interval * 7;
      for (const i of idxs) {
        const dn = weekStart + i;
        if (dn < start) continue;
        if (!take(dn)) break outer;
      }
      if (weekStart > limit) break;
    }
  } else if (rule.freq === 'monthly' || rule.freq === 'yearly') {
    const { y, m, d } = parseIso(ev.date);
    for (let k = 0; k < 5000; k++) {
      let yy: number, mm: number;
      if (rule.freq === 'monthly') { const total = (m - 1) + k * interval; yy = y + Math.floor(total / 12); mm = (total % 12) + 1; }
      else { yy = y + k * interval; mm = m; }
      if (Date.UTC(yy, mm - 1, 1) / DAY_MS > limit) break;
      if (d > daysInMonth(yy, mm)) continue;           // e.g. the 31st / Feb 29: RFC 5545 skips such months
      const dn = Math.floor(Date.UTC(yy, mm - 1, d) / DAY_MS);
      if (!take(dn)) break;
    }
  }
  return out;
}

const timeKey = (e: CalendarEvent) => (e.time ?? '');

/** Every occurrence of `events` in [fromIso, toIso], ordered by day, all-day first, then by time. */
export function occurrencesInRange(events: CalendarEvent[], fromIso: string, toIso: string): Occurrence[] {
  const out: Occurrence[] = [];
  for (const event of events) {
    for (const date of expandEvent(event, fromIso, toIso)) out.push({ event, date, key: `${event.id}@${date}` });
  }
  out.sort((a, b) => a.date.localeCompare(b.date) || timeKey(a.event).localeCompare(timeKey(b.event)) || a.event.title.localeCompare(b.event.title));
  return out;
}

export function groupByDate(occs: Occurrence[]): Record<string, Occurrence[]> {
  const out: Record<string, Occurrence[]> = {};
  for (const o of occs) (out[o.date] ??= []).push(o);
  return out;
}

// ── Day layout for the week/day grid ────────────────────────────────────

export interface Placed { occ: Occurrence; startMin: number; endMin: number; lane: number; lanes: number }

export function toMinutes(time: string): number {
  const [h, m] = time.split(':').map(Number);
  return h * 60 + m;
}

/**
 * Places timed occurrences of ONE day side by side: overlapping events get their own lane,
 * and every event in a connected cluster shares the cluster's lane count (so widths are equal).
 * Events are clamped to the day (a late event never spills past midnight) and last >= 20 minutes
 * so they stay clickable.
 */
export function layoutDay(occs: Occurrence[]): Placed[] {
  const items = occs
    .filter((o) => o.event.time)
    .map((occ) => {
      const startMin = Math.min(toMinutes(occ.event.time!), 24 * 60 - 20);
      const dur = Math.max(20, occ.event.durationMinutes ?? 60);
      return { occ, startMin, endMin: Math.min(24 * 60, startMin + dur), lane: 0, lanes: 1 };
    })
    .sort((a, b) => a.startMin - b.startMin || b.endMin - a.endMin);

  let cluster: typeof items = [];
  let clusterEnd = -1;
  const laneEnds: number[] = [];
  const flush = () => { for (const it of cluster) it.lanes = Math.max(1, laneEnds.length); cluster = []; laneEnds.length = 0; };
  for (const it of items) {
    if (cluster.length && it.startMin >= clusterEnd) { flush(); clusterEnd = -1; }
    let lane = laneEnds.findIndex((end) => end <= it.startMin);
    if (lane === -1) { lane = laneEnds.length; laneEnds.push(it.endMin); } else laneEnds[lane] = it.endMin;
    it.lane = lane;
    cluster.push(it);
    clusterEnd = Math.max(clusterEnd, it.endMin);
  }
  flush();
  return items;
}

/** Human-readable repeat summary key parts (used by the editor's hint line). */
export function describeRule(rule: RecurrenceRule | null | undefined): string {
  if (!rule) return '';
  return `${rule.freq}:${rule.interval}:${(rule.byDay ?? []).join(',')}:${rule.until ?? ''}:${rule.count ?? ''}`;
}
