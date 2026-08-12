export interface CalendarEvent {
  id: string;
  title: string;
  /** ISO date, YYYY-MM-DD — the calendar day this event belongs to. */
  date: string;
  /** HH:MM 24h, or null for an all-day event. */
  time: string | null;
  durationMinutes: number | null;
  description: string;
  /** One of EVENT_COLORS below — a fixed palette, not free-form. */
  color: string;
}

export const EVENT_COLORS = ['#3b82f6', '#22c55e', '#f59e0b', '#ef4444', '#a855f7', '#06b6d4'] as const;
