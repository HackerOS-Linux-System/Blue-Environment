import { writable, derived, get } from 'svelte/store';
import { dnd, setDnd, normalizeDnd, type DndState } from './doNotDisturb';
import { occurrencesInRange } from '../components/apps/Blue-Calendar-App/recurrence';
import type { CalendarEvent } from '../components/apps/Blue-Calendar-App/types';

/**
 * Focus mode — Do Not Disturb + app blocking + calendar, in one switch.
 *
 *  • Start it by hand ("focus for 45 min"), as a Pomodoro cycle, or let the
 *    calendar start it: any event whose title contains a keyword ("Focus",
 *    "Deep work", "🎯") or whose description contains #focus is a focus block.
 *  • While focus is on: Do Not Disturb is switched on (the person's previous DND
 *    setting is restored afterwards), and the apps on the block list can't be
 *    launched — same gate as Parental Controls, checked next to it.
 *  • Relationship with Parental Controls: Focus never changes them. With *strict*
 *    on and a Parental Controls PIN set, ending a session early needs that PIN
 *    (so a parent can start a homework block a child cannot just cancel).
 *
 * Everything decision-shaped is a pure function (tested in focusMode.test.ts);
 * the store at the bottom only wires them to the clock, the calendar and DND.
 */

export interface FocusConfig {
  calendarAuto: boolean;
  keywords: string[];
  blockedApps: string[];
  allowUrgent: boolean;
  strict: boolean;
  workMinutes: number;
  breakMinutes: number;
}

export const DEFAULT_FOCUS_CONFIG: FocusConfig = {
  calendarAuto: true,
  keywords: ['focus', 'deep work', '🎯'],
  blockedApps: [],
  allowUrgent: true,
  strict: false,
  workMinutes: 25,
  breakMinutes: 5,
};

export interface FocusSession {
  source: 'manual' | 'pomodoro';
  startedAt: number;
  /** null = until stopped (manual only). For pomodoro: end of the last cycle. */
  endsAt: number | null;
  /** Pomodoro: length of work / break phases in ms and number of cycles. */
  workMs?: number;
  breakMs?: number;
}

export type FocusPhase = 'work' | 'break';

export interface FocusStatus {
  active: boolean;
  source: 'manual' | 'pomodoro' | 'calendar' | null;
  /** End of the current focus stretch (null = open-ended). */
  endsAt: number | null;
  label: string;
  phase: FocusPhase | null;
}

export const INACTIVE: FocusStatus = { active: false, source: null, endsAt: null, label: '', phase: null };

/** Apps that can never be blocked: the way back out must always exist. */
export const NEVER_BLOCKED = ['settings', 'blueSettings', 'blue-settings'];

const MIN = 60_000;

// ── calendar ───────────────────────────────────────────────────────────────

export function isFocusEvent(title: string, description: string, keywords: string[]): boolean {
  const t = (title ?? '').toLowerCase();
  if (/(^|\s)#focus\b/i.test(description ?? '')) return true;
  return keywords.map((k) => k.trim().toLowerCase()).filter(Boolean).some((k) => t.includes(k));
}

const pad = (n: number) => String(n).padStart(2, '0');
export const isoLocal = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

/** [start, end) of one occurrence in epoch ms (local time). All-day = the whole day. */
export function occurrenceSpan(ev: CalendarEvent, dateIso: string): { start: number; end: number } {
  const [y, m, d] = dateIso.split('-').map(Number);
  if (!ev.time) return { start: new Date(y, m - 1, d, 0, 0).getTime(), end: new Date(y, m - 1, d + 1, 0, 0).getTime() };
  const [hh, mm] = ev.time.split(':').map(Number);
  const start = new Date(y, m - 1, d, hh, mm).getTime();
  return { start, end: start + (ev.durationMinutes ?? 60) * MIN };
}

export function focusBlocks(events: CalendarEvent[], now: number, keywords: string[], horizonDays = 2): { title: string; start: number; end: number }[] {
  const from = new Date(now - 86_400_000);
  const to = new Date(now + horizonDays * 86_400_000);
  const focus = events.filter((e) => isFocusEvent(e.title, e.description, keywords));
  return occurrencesInRange(focus, isoLocal(from), isoLocal(to))
    .map((o) => ({ title: o.event.title, ...occurrenceSpan(o.event, o.date) }))
    .sort((a, b) => a.start - b.start);
}

export function activeCalendarFocus(events: CalendarEvent[], now: number, keywords: string[]) {
  // merge back-to-back / overlapping blocks so the end shown is the real end
  const blocks = focusBlocks(events, now, keywords).filter((b) => b.end > now);
  const current = blocks.find((b) => b.start <= now && now < b.end);
  if (!current) return null;
  let end = current.end;
  for (const b of blocks) if (b.start <= end && b.end > end) end = b.end;
  return { title: current.title, start: current.start, end };
}

export function nextCalendarFocus(events: CalendarEvent[], now: number, keywords: string[]) {
  return focusBlocks(events, now, keywords, 7).find((b) => b.start > now) ?? null;
}

// ── pomodoro ───────────────────────────────────────────────────────────────

export function pomodoroSession(workMin: number, breakMin: number, cycles: number, now: number): FocusSession {
  const w = Math.max(1, Math.round(workMin)) * MIN;
  const b = Math.max(0, Math.round(breakMin)) * MIN;
  const n = Math.max(1, Math.round(cycles));
  // no break after the last work block
  return { source: 'pomodoro', startedAt: now, workMs: w, breakMs: b, endsAt: now + n * w + (n - 1) * b };
}

export function pomodoroPhase(s: FocusSession, now: number): { phase: FocusPhase; phaseEndsAt: number; cycle: number } | null {
  if (s.source !== 'pomodoro' || s.workMs === undefined || s.breakMs === undefined || s.endsAt === null) return null;
  if (now < s.startedAt || now >= s.endsAt) return null;
  const period = s.workMs + s.breakMs;
  const pos = (now - s.startedAt) % period;
  const cycleStart = now - pos;
  const cycle = Math.floor((now - s.startedAt) / period) + 1;
  return pos < s.workMs
    ? { phase: 'work', phaseEndsAt: Math.min(cycleStart + s.workMs, s.endsAt), cycle }
    : { phase: 'break', phaseEndsAt: Math.min(cycleStart + period, s.endsAt), cycle };
}

// ── the combined decision ──────────────────────────────────────────────────

export function computeFocus(config: FocusConfig, session: FocusSession | null, events: CalendarEvent[], now: number): FocusStatus {
  if (session && (session.endsAt === null || now < session.endsAt) && now >= session.startedAt) {
    if (session.source === 'pomodoro') {
      const p = pomodoroPhase(session, now);
      if (p) return { active: p.phase === 'work', source: 'pomodoro', endsAt: p.phaseEndsAt, label: p.phase === 'work' ? `Focus ${p.cycle}` : 'Break', phase: p.phase };
    } else {
      return { active: true, source: 'manual', endsAt: session.endsAt, label: 'Focus', phase: 'work' };
    }
  }
  if (config.calendarAuto) {
    const c = activeCalendarFocus(events, now, config.keywords);
    if (c) return { active: true, source: 'calendar', endsAt: c.end, label: c.title, phase: 'work' };
  }
  return INACTIVE;
}

/** Reason to show when launching `appId` is blocked, or null. */
export function blockedLaunch(appId: string, config: FocusConfig, status: FocusStatus): string | null {
  if (!status.active || NEVER_BLOCKED.includes(appId) || !config.blockedApps.includes(appId)) return null;
  const until = status.endsAt ? ` until ${pad(new Date(status.endsAt).getHours())}:${pad(new Date(status.endsAt).getMinutes())}` : '';
  return `Focus mode is on${until}. This app is on your focus block list.`;
}

/**
 * Ending a manual / pomodoro session early needs the Parental Controls PIN
 * when *strict* is on and a PIN exists. Calendar focus ends with its event.
 */
export function endNeedsPin(config: FocusConfig, status: FocusStatus, pinIsSet: boolean): boolean {
  return config.strict && pinIsSet && status.active && (status.source === 'manual' || status.source === 'pomodoro');
}

// ── DND coupling ───────────────────────────────────────────────────────────

export type DndAction = { type: 'enable'; save: DndState } | { type: 'restore'; to: DndState } | { type: 'none' };

/**
 * Focus turning on: remember the current DND setting and switch DND on.
 * Focus turning off: restore what the person had — but only if they didn't
 * change DND themselves in the meantime (then their choice stands).
 */
export function dndTransition(wasActive: boolean, isActive: boolean, current: DndState, saved: DndState | null): DndAction {
  if (!wasActive && isActive) return { type: 'enable', save: current };
  if (wasActive && !isActive && saved) {
    const stillOurs = current.mode === 'on' && current.untilTs === null;
    return stillOurs ? { type: 'restore', to: saved } : { type: 'none' };
  }
  return { type: 'none' };
}

// ── persistence + store ────────────────────────────────────────────────────

const KEY = 'blue.focus.v1';

interface Persisted { config: FocusConfig; session: FocusSession | null; savedDnd: DndState | null }

export function normalizeConfig(raw: unknown): FocusConfig {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const num = (v: unknown, d: number, lo: number, hi: number) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, Math.round(v))) : d);
  const strs = (v: unknown, d: string[]) => (Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string' && x.length > 0 && x.length < 100).slice(0, 100) : d);
  return {
    calendarAuto: typeof r.calendarAuto === 'boolean' ? r.calendarAuto : DEFAULT_FOCUS_CONFIG.calendarAuto,
    keywords: strs(r.keywords, DEFAULT_FOCUS_CONFIG.keywords),
    blockedApps: strs(r.blockedApps, []),
    allowUrgent: typeof r.allowUrgent === 'boolean' ? r.allowUrgent : true,
    strict: r.strict === true,
    workMinutes: num(r.workMinutes, 25, 1, 180),
    breakMinutes: num(r.breakMinutes, 5, 0, 60),
  };
}

function normalizeSession(raw: unknown): FocusSession | null {
  if (!raw || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  if ((r.source !== 'manual' && r.source !== 'pomodoro') || typeof r.startedAt !== 'number') return null;
  return {
    source: r.source,
    startedAt: r.startedAt,
    endsAt: typeof r.endsAt === 'number' ? r.endsAt : null,
    workMs: typeof r.workMs === 'number' ? r.workMs : undefined,
    breakMs: typeof r.breakMs === 'number' ? r.breakMs : undefined,
  };
}

function load(): Persisted {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? 'null');
    if (raw && typeof raw === 'object') {
      return { config: normalizeConfig(raw.config), session: normalizeSession(raw.session), savedDnd: raw.savedDnd ? normalizeDnd(raw.savedDnd) : null };
    }
  } catch { /* fall through */ }
  return { config: { ...DEFAULT_FOCUS_CONFIG }, session: null, savedDnd: null };
}

const state = writable<Persisted>(load());
state.subscribe((s) => { try { localStorage.setItem(KEY, JSON.stringify(s)); } catch { /* storage unavailable */ } });

export const focusConfig = derived(state, ($s) => $s.config);
export const focusSession = derived(state, ($s) => $s.session);
export const focusStatus = writable<FocusStatus>(INACTIVE);
export const nextFocus = writable<{ title: string; start: number; end: number } | null>(null);

export function setFocusConfig(patch: Partial<FocusConfig>) {
  state.update((s) => ({ ...s, config: normalizeConfig({ ...s.config, ...patch }) }));
}

export function startFocus(minutes: number | null, now = Date.now()) {
  const endsAt = minutes === null ? null : now + Math.max(1, Math.round(minutes)) * MIN;
  state.update((s) => ({ ...s, session: { source: 'manual', startedAt: now, endsAt } }));
}

export function startPomodoro(cycles = 4, now = Date.now()) {
  state.update((s) => ({ ...s, session: pomodoroSession(s.config.workMinutes, s.config.breakMinutes, cycles, now) }));
}

/** Ends the session. Returns false (and does nothing) when the PIN check fails. */
export async function stopFocus(opts: { pinIsSet: boolean; verifyPin: (pin: string) => Promise<boolean>; pin?: string }): Promise<boolean> {
  const { config } = get(state);
  if (endNeedsPin(config, get(focusStatus), opts.pinIsSet)) {
    if (!opts.pin || !(await opts.verifyPin(opts.pin))) return false;
  }
  state.update((s) => ({ ...s, session: null }));
  return true;
}

/** Re-evaluates now: updates the status and performs the DND transition. */
export function evaluate(events: CalendarEvent[], now = Date.now()) {
  const s = get(state);
  let session = s.session;
  if (session && session.endsAt !== null && now >= session.endsAt) { // finished on its own
    session = null;
    state.update((x) => ({ ...x, session: null }));
  }
  const status = computeFocus(s.config, session, events, now);
  const was = get(focusStatus).active;
  focusStatus.set(status);
  nextFocus.set(s.config.calendarAuto ? nextCalendarFocus(events, now, s.config.keywords) : null);

  const action = dndTransition(was, status.active, get(dnd), s.savedDnd);
  if (action.type === 'enable') {
    state.update((x) => ({ ...x, savedDnd: action.save }));
    setDnd({ mode: 'on', untilTs: null, allowUrgent: s.config.allowUrgent });
  } else if (action.type === 'restore') {
    setDnd(action.to);
    state.update((x) => ({ ...x, savedDnd: null }));
  }
  return status;
}

/** Checked by the launcher next to Parental Controls. */
export function checkFocusBlock(appId: string): string | null {
  return blockedLaunch(appId, get(state).config, get(focusStatus));
}

/** Runs the engine: every 15 s, and whenever the calendar changes. Returns a stop function. */
export function startFocusEngine(loadEvents: () => Promise<CalendarEvent[]>): () => void {
  let events: CalendarEvent[] = [];
  let loadedAt = 0;
  let stopped = false;
  async function tick(force = false) {
    if (stopped) return;
    const now = Date.now();
    if (force || now - loadedAt > 60_000) {
      try { events = await loadEvents(); loadedAt = now; } catch { /* keep the last list */ }
    }
    if (!stopped) evaluate(events, Date.now());
  }
  void tick(true);
  const timer = setInterval(() => void tick(), 15_000);
  const onChange = () => void tick(true);
  window.addEventListener('blue-calendar-changed', onChange);
  return () => { stopped = true; clearInterval(timer); window.removeEventListener('blue-calendar-changed', onChange); };
}
