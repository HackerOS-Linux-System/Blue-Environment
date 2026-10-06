import { notificationManager } from '../../../utils/notificationManager';
import { translate } from '../../../stores/language';
import { CALENDAR_CHANGED, loadAllEventsForReminders } from './calendarStore';
import { dueReminders, type DueReminder } from './reminders';
import type { CalendarEvent } from './types';

const FIRED_KEY = 'blue-calendar-fired';
const TICK_MS = 20_000;
const RELOAD_MS = 5 * 60_000;

function readFired(): Record<string, number> {
  try { return JSON.parse(localStorage.getItem(FIRED_KEY) ?? '{}'); } catch { return {}; }
}
function writeFired(map: Record<string, number>) {
  const cutoff = Date.now() - 2 * 86_400_000;
  const pruned = Object.fromEntries(Object.entries(map).filter(([, t]) => t > cutoff));
  try { localStorage.setItem(FIRED_KEY, JSON.stringify(pruned)); } catch { /* storage full/unavailable */ }
}

const pad = (n: number) => String(n).padStart(2, '0');

function bodyFor(r: DueReminder, now: Date): string {
  const start = new Date(r.startMs);
  const mins = Math.round((r.startMs - now.getTime()) / 60_000);
  const hhmm = `${pad(start.getHours())}:${pad(start.getMinutes())}`;
  if (!r.event.time) return translate('cal.notify.all_day', { date: r.date });
  if (mins <= 0) return translate('cal.notify.now');
  if (mins < 60) return translate('cal.notify.in_min', { n: mins });
  const sameDay = start.toDateString() === now.toDateString();
  return translate('cal.notify.at', { when: sameDay ? hhmm : `${r.date} ${hhmm}` });
}

export function startCalendarReminders(): () => void {
  let events: CalendarEvent[] = [];
  let loadedAt = 0;
  let stopped = false;
  let busy = false;

  async function reload() { try { events = await loadAllEventsForReminders(); loadedAt = Date.now(); } catch { /* keep the old list */ } }

  async function tick() {
    if (stopped || busy) return;
    busy = true;
    try {
      if (Date.now() - loadedAt > RELOAD_MS) await reload();
      const fired = readFired();
      const now = new Date();
      const due = dueReminders(events, now, new Set(Object.keys(fired)));
      for (const r of due) {
        notificationManager.add({ title: r.event.title, message: bodyFor(r, now), body: bodyFor(r, now), appId: 'blue_calendar', app: 'Calendar', icon: '' });
        fired[r.key] = Date.now();
      }
      if (due.length) writeFired(fired);
    } finally { busy = false; }
  }

  const onChanged = () => { loadedAt = 0; tick(); };
  window.addEventListener(CALENDAR_CHANGED, onChanged);
  const timer = setInterval(tick, TICK_MS);
  reload().then(tick);
  return () => { stopped = true; clearInterval(timer); window.removeEventListener(CALENDAR_CHANGED, onChanged); };
}
