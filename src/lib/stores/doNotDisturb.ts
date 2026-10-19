import { writable, get } from 'svelte/store';

/**
 * Do Not Disturb. While active, toasts and sounds are suppressed; the
 * notification itself still lands in the Notification Center so nothing is
 * lost. Urgent notifications can optionally break through.
 */
export type DndMode = 'off' | 'on' | 'until' | 'schedule';

export interface DndState {
  mode: DndMode;
  /** Epoch ms; only for mode 'until'. */
  untilTs: number | null;
  /** "HH:MM" window for mode 'schedule' (may cross midnight). */
  scheduleStart: string;
  scheduleEnd: string;
  allowUrgent: boolean;
}

export const DEFAULT_DND: DndState = { mode: 'off', untilTs: null, scheduleStart: '22:00', scheduleEnd: '07:00', allowUrgent: true };

const KEY = 'blue.dnd.v1';

export function parseHHMM(s: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(s.trim());
  if (!m) return null;
  const h = Number(m[1]);
  const mi = Number(m[2]);
  return h < 24 && mi < 60 ? h * 60 + mi : null;
}

/** Inside [start, end)? Handles windows that cross midnight; an invalid or empty window is never active. */
export function inWindow(start: string, end: string, minuteOfDay: number): boolean {
  const s = parseHHMM(start);
  const e = parseHHMM(end);
  if (s === null || e === null || s === e) return false;
  return s < e ? minuteOfDay >= s && minuteOfDay < e : minuteOfDay >= s || minuteOfDay < e;
}

export function normalizeDnd(raw: unknown): DndState {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const mode: DndMode = ['off', 'on', 'until', 'schedule'].includes(r.mode as string) ? (r.mode as DndMode) : 'off';
  const until = typeof r.untilTs === 'number' && Number.isFinite(r.untilTs) ? r.untilTs : null;
  const start = typeof r.scheduleStart === 'string' && parseHHMM(r.scheduleStart) !== null ? r.scheduleStart : DEFAULT_DND.scheduleStart;
  const end = typeof r.scheduleEnd === 'string' && parseHHMM(r.scheduleEnd) !== null ? r.scheduleEnd : DEFAULT_DND.scheduleEnd;
  return { mode: mode === 'until' && until === null ? 'off' : mode, untilTs: until, scheduleStart: start, scheduleEnd: end, allowUrgent: typeof r.allowUrgent === 'boolean' ? r.allowUrgent : true };
}

export function isDndActive(s: DndState, now: Date = new Date()): boolean {
  switch (s.mode) {
    case 'on': return true;
    case 'until': return s.untilTs !== null && now.getTime() < s.untilTs;
    case 'schedule': return inWindow(s.scheduleStart, s.scheduleEnd, now.getHours() * 60 + now.getMinutes());
    default: return false;
  }
}

/** Should a toast / sound be shown right now? */
export function shouldShowToast(s: DndState, urgent = false, now: Date = new Date()): boolean {
  if (!isDndActive(s, now)) return true;
  return urgent && s.allowUrgent;
}

/** Minutes left of an 'until' period (rounded up), or null. */
export function minutesLeft(s: DndState, now: Date = new Date()): number | null {
  if (s.mode !== 'until' || s.untilTs === null) return null;
  const ms = s.untilTs - now.getTime();
  return ms > 0 ? Math.ceil(ms / 60000) : null;
}

function load(): DndState {
  try {
    const v = typeof localStorage !== 'undefined' ? localStorage.getItem(KEY) : null;
    return normalizeDnd(v ? JSON.parse(v) : null);
  } catch {
    return { ...DEFAULT_DND };
  }
}

export const dnd = writable<DndState>(load());

function save(s: DndState) {
  try { localStorage.setItem(KEY, JSON.stringify(s)); } catch { /* ignore */ }
}

export function setDnd(patch: Partial<DndState>) {
  dnd.update((cur) => { const next = normalizeDnd({ ...cur, ...patch }); save(next); return next; });
}

export const enableDndFor = (minutes: number, now: Date = new Date()) =>
  setDnd({ mode: 'until', untilTs: now.getTime() + Math.max(1, Math.round(minutes)) * 60000 });

export const disableDnd = () => setDnd({ mode: 'off', untilTs: null });

/** Toggle used by the Control Center tile: on → off, anything else → on. */
export function toggleDnd(now: Date = new Date()) {
  if (isDndActive(get(dnd), now)) disableDnd(); else setDnd({ mode: 'on', untilTs: null });
}
