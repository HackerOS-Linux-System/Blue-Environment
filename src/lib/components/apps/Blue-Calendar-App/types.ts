export interface RecurrenceRule {
  freq: 'daily' | 'weekly' | 'monthly' | 'yearly';
  /** Every N days/weeks/months/years (>= 1). */
  interval: number;
  /** Weekly only: two-letter codes, Monday first — `MO TU WE TH FR SA SU` (iCalendar BYDAY). */
  byDay?: string[] | null;
  /** Inclusive last day, `YYYY-MM-DD`. */
  until?: string | null;
  /** Total number of occurrences, including the first. */
  count?: number | null;
}

export interface CalendarEvent {
  id: string;
  title: string;
  /** ISO date, YYYY-MM-DD — the calendar day this event belongs to (first day of a series). */
  date: string;
  /** HH:MM 24h, or null for an all-day event. */
  time: string | null;
  durationMinutes: number | null;
  description: string;
  /** One of EVENT_COLORS below — a fixed palette, not free-form. */
  color: string;
  /** Repeat rule, or null/undefined for a one-off event. */
  recurrence?: RecurrenceRule | null;
  /** Set on events that come from an ICS subscription — read-only. */
  subscriptionId?: string | null;
  /** Deleted single occurrences of a recurring event (`YYYY-MM-DD`). */
  exdates?: string[];
  /** Minutes before the start to remind (0 = at start); null/undefined = no reminder.
   *  For all-day events the "start" is 09:00 that day. */
  reminderMinutes?: number | null;
}

export interface CalendarSubscription {
  id: string;
  name: string;
  url: string;
  color: string;
  enabled: boolean;
  lastSynced?: string | null;
}

/** One concrete appearance of an event on a calendar day (a recurring event has many). */
export interface Occurrence {
  event: CalendarEvent;
  date: string;
  /** Unique per occurrence: `<event id>@<date>`. */
  key: string;
}

export type CalendarView = 'month' | 'week' | 'day';

export const EVENT_COLORS = ['#3b82f6', '#22c55e', '#f59e0b', '#ef4444', '#a855f7', '#06b6d4'] as const;

/** Weekday codes in Monday-first order (index = column in the week view). */
export const WEEKDAY_CODES = ['MO', 'TU', 'WE', 'TH', 'FR', 'SA', 'SU'] as const;

/** Reminder choices offered in the editor (minutes before). */
export const REMINDER_CHOICES = [0, 5, 10, 15, 30, 60, 120, 1440, 10080] as const;
