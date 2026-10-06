import { writable, derived, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { CalendarEvent, CalendarSubscription } from './types';

/** Fired on `window` whenever calendar data changed — the reminder scheduler reloads on it. */
export const CALENDAR_CHANGED = 'blue-calendar-changed';
const changed = () => window.dispatchEvent(new CustomEvent(CALENDAR_CHANGED));

const invoke = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => SystemBridge.invokeCommand<T>(cmd, args);
const msg = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** Everything the reminder scheduler needs: the user's events plus enabled subscriptions' cached events. */
export async function loadAllEventsForReminders(): Promise<CalendarEvent[]> {
  const own = await SystemBridge.calendarLoadEvents().catch(() => [] as CalendarEvent[]);
  let subs: CalendarSubscription[] = [];
  try { subs = await invoke<CalendarSubscription[]>('calendar_list_subscriptions'); } catch { /* not in Tauri */ }
  const cached = await Promise.all(
    subs.filter((s) => s.enabled).map((s) => invoke<CalendarEvent[]>('calendar_cached_subscription_events', { id: s.id }).catch(() => [] as CalendarEvent[])),
  );
  return [...own, ...cached.flat()];
}

export function createCalendarStore() {
  const events = writable<CalendarEvent[]>([]);
  const subscriptions = writable<CalendarSubscription[]>([]);
  /** Cached events per subscription id (read-only). */
  const subEvents = writable<Record<string, CalendarEvent[]>>({});
  const syncing = writable<Record<string, boolean>>({});
  const loading = writable(true);
  const error = writable<string | null>(null);

  /** What the views show: own events + events of ENABLED subscriptions. */
  const allEvents = derived([events, subscriptions, subEvents], ([$e, $s, $c]) => [
    ...$e,
    ...$s.filter((s) => s.enabled).flatMap((s) => $c[s.id] ?? []),
  ]);

  async function load() {
    loading.set(true);
    try {
      events.set(await SystemBridge.calendarLoadEvents());
      await loadSubscriptions();
    } finally {
      loading.set(false);
    }
  }

  async function loadSubscriptions() {
    let subs: CalendarSubscription[] = [];
    try { subs = await invoke<CalendarSubscription[]>('calendar_list_subscriptions'); } catch { /* not in Tauri */ }
    subscriptions.set(subs);
    const entries = await Promise.all(subs.map(async (s) => [s.id, await invoke<CalendarEvent[]>('calendar_cached_subscription_events', { id: s.id }).catch(() => [])] as const));
    subEvents.set(Object.fromEntries(entries));
  }

  function newId(): string {
    // Timestamp + random suffix — good enough for a single-user local store.
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
    changed();
  }

  async function remove(id: string): Promise<void> {
    const res = await SystemBridge.calendarDeleteEvent(id);
    if (!res.ok) { error.set(res.error ?? 'Failed to delete event'); return; }
    events.update((prev) => prev.filter((e) => e.id !== id));
    changed();
  }

  /** Deletes ONE occurrence of a recurring event (adds an exception date). */
  async function removeOccurrence(event: CalendarEvent, date: string): Promise<void> {
    if (!event.recurrence) return remove(event.id);
    await upsert({ ...event, exdates: [...new Set([...(event.exdates ?? []), date])] });
  }

  // ── Subscriptions ─────────────────────────────────────────────────────
  async function addSubscription(name: string, url: string, color: string): Promise<CalendarSubscription | null> {
    try {
      const sub = await invoke<CalendarSubscription>('calendar_add_subscription', { name, url, color });
      subscriptions.update((p) => [...p, sub]);
      error.set(null);
      await syncSubscription(sub.id);
      return sub;
    } catch (e) { error.set(msg(e)); return null; }
  }

  async function syncSubscription(id: string): Promise<boolean> {
    syncing.update((m) => ({ ...m, [id]: true }));
    try {
      const evs = await invoke<CalendarEvent[]>('calendar_sync_subscription', { id });
      subEvents.update((m) => ({ ...m, [id]: evs }));
      await loadSubscriptionsMeta();
      error.set(null);
      changed();
      return true;
    } catch (e) {
      error.set(msg(e));
      return false;
    } finally {
      syncing.update((m) => ({ ...m, [id]: false }));
    }
  }

  /** Re-reads only subscription metadata (last-synced time) without dropping cached events. */
  async function loadSubscriptionsMeta() {
    try { subscriptions.set(await invoke<CalendarSubscription[]>('calendar_list_subscriptions')); } catch { /* ignore */ }
  }

  async function syncAll(): Promise<void> {
    for (const s of get(subscriptions).filter((x) => x.enabled)) await syncSubscription(s.id);
  }

  async function removeSubscription(id: string): Promise<void> {
    try {
      await invoke('calendar_remove_subscription', { id });
      subscriptions.update((p) => p.filter((s) => s.id !== id));
      subEvents.update((m) => { const { [id]: _drop, ...rest } = m; return rest; });
      changed();
    } catch (e) { error.set(msg(e)); }
  }

  async function setSubscriptionEnabled(id: string, enabled: boolean): Promise<void> {
    try {
      await invoke('calendar_set_subscription_enabled', { id, enabled });
      subscriptions.update((p) => p.map((s) => (s.id === id ? { ...s, enabled } : s)));
      changed();
    } catch (e) { error.set(msg(e)); }
  }

  // ── .ics import / export ──────────────────────────────────────────────
  async function importIcs(content: string): Promise<{ added: number; skipped: number } | null> {
    try {
      const res = await invoke<{ added: number; skipped: number }>('calendar_import_ics', { content });
      events.set(await SystemBridge.calendarLoadEvents());
      error.set(null);
      changed();
      return res;
    } catch (e) { error.set(msg(e)); return null; }
  }

  async function exportIcs(): Promise<string | null> {
    try { return await invoke<string>('calendar_export_ics'); } catch (e) { error.set(msg(e)); return null; }
  }

  return {
    events, subscriptions, subEvents, syncing, allEvents, loading, error,
    load, upsert, remove, removeOccurrence,
    addSubscription, syncSubscription, syncAll, removeSubscription, setSubscriptionEnabled,
    importIcs, exportIcs,
  };
}

export type CalendarStore = ReturnType<typeof createCalendarStore>;
