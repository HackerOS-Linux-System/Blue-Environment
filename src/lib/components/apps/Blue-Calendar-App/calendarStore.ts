import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { CalendarEvent } from './types';

export function createCalendarStore() {
  const events = writable<CalendarEvent[]>([]);
  const loading = writable(true);
  const error = writable<string | null>(null);

  async function load() {
    loading.set(true);
    try {
      const list = await SystemBridge.calendarLoadEvents();
      events.set(list);
    } finally {
      loading.set(false);
    }
  }

  function newId(): string {
    // Timestamp + random suffix — good enough for a single-user local
    // store where collisions only matter within the same millisecond,
    // not a real UUID (no crypto.randomUUID() dependency needed).
    return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  }

  async function upsert(event: Omit<CalendarEvent, 'id'> & { id?: string }): Promise<void> {
    const full: CalendarEvent = { ...event, id: event.id ?? newId() };
    const res = await SystemBridge.calendarSaveEvent(full);
    if (!res.ok) { error.set(res.error ?? 'Failed to save event'); return; }
    events.update((prev) => {
      const idx = prev.findIndex((e) => e.id === full.id);
      if (idx >= 0) { const next = [...prev]; next[idx] = full; return next; }
      return [...prev, full];
    });
  }

  async function remove(id: string): Promise<void> {
    const res = await SystemBridge.calendarDeleteEvent(id);
    if (!res.ok) { error.set(res.error ?? 'Failed to delete event'); return; }
    events.update((prev) => prev.filter((e) => e.id !== id));
  }

  function eventsOn(dateIso: string): CalendarEvent[] {
    return get(events)
      .filter((e) => e.date === dateIso)
      .sort((a, b) => (a.time ?? '').localeCompare(b.time ?? ''));
  }

  return { events, loading, error, load, upsert, remove, eventsOn };
}

export type CalendarStore = ReturnType<typeof createCalendarStore>;
