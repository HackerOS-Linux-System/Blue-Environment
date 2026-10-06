import type { CalendarEvent } from './types';
import { addDays, expandEvent, toMinutes } from './recurrence';

/** Reminders for all-day events are relative to this time of day. */
export const ALL_DAY_REMINDER_TIME = '09:00';
/** A reminder that was due while the shell was off is still shown if it is at most this old. */
export const GRACE_MS = 10 * 60_000;
/** Largest reminder offset offered in the UI (1 week) + slack — how far ahead occurrences are expanded. */
const LOOKAHEAD_DAYS = 9;

export interface DueReminder { key: string; event: CalendarEvent; date: string; startMs: number; fireMs: number }

const isoLocal = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/** Local epoch ms of an occurrence's start. */
export function startMs(date: string, time: string | null): number {
  const [y, m, d] = date.split('-').map(Number);
  const min = toMinutes(time ?? ALL_DAY_REMINDER_TIME);
  return new Date(y, m - 1, d, Math.floor(min / 60), min % 60, 0, 0).getTime();
}

/**
 * Reminders that should fire now: fire time has passed, is no older than `GRACE_MS`,
 * and was not fired before. Subscription events without a reminder are ignored.
 */
export function dueReminders(events: CalendarEvent[], now: Date, fired: ReadonlySet<string>): DueReminder[] {
  const nowMs = now.getTime();
  const from = isoLocal(new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1));
  const to = addDays(isoLocal(now), LOOKAHEAD_DAYS);
  const out: DueReminder[] = [];
  for (const event of events) {
    if (event.reminderMinutes == null) continue;
    for (const date of expandEvent(event, from, to)) {
      const start = startMs(date, event.time);
      const fireMs = start - event.reminderMinutes * 60_000;
      const key = `${event.id}@${date}`;
      if (fireMs <= nowMs && nowMs - fireMs <= GRACE_MS && !fired.has(key)) out.push({ key, event, date, startMs: start, fireMs });
    }
  }
  return out.sort((a, b) => a.fireMs - b.fireMs);
}
