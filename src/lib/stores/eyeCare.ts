import { writable, get } from 'svelte/store';

/**
 * Eye & health care ("Dbanie o zdrowie / wzrok").
 *
 * The shell watches how long the person has actually been working and
 * actively nudges them to stay inside healthy limits:
 *
 *  • 20-20-20 rule   — every 20 min of activity: look at something ≥ 6 m
 *                      (20 ft) away for 20 s. A small card (or a full-screen
 *                      overlay in strict mode) counts the seconds down.
 *  • Long break      — every N min (default 60) a real 5-minute break away
 *                      from the screen (full-screen, dimmed).
 *  • Blink reminder  — dry eyes: "blink slowly" every few minutes.
 *  • Posture / stretch / water reminders — gentle toasts.
 *  • Daily limit     — warns when the day's screen time goes over the norm.
 *  • Natural breaks  — if the person simply walks away (idle), the timers
 *                      reset by themselves; a break that already happened
 *                      is never asked for twice.
 *  • Strict mode     — the break cannot be skipped (only an emergency exit:
 *                      press Escape three times quickly).
 *
 * Everything decision-shaped is a pure function (tested in eyeCare.test.ts);
 * the store at the bottom only wires them to the clock and to input events.
 */

export type EyeCareMode = 'gentle' | 'firm' | 'strict';

export interface EyeCareConfig {
  enabled: boolean;
  mode: EyeCareMode;
  // 20-20-20
  rule2020: boolean;
  lookAwayEveryMin: number;
  lookAwaySec: number;
  // long break
  longBreak: boolean;
  longBreakEveryMin: number;
  longBreakMin: number;
  // reminders
  blink: boolean;
  blinkEveryMin: number;
  posture: boolean;
  postureEveryMin: number;
  stretch: boolean;
  stretchEveryMin: number;
  water: boolean;
  waterEveryMin: number;
  // daily limit
  dailyLimit: boolean;
  dailyLimitMin: number;
  // late hours: breaks come more often (×0.7) between these times
  lateHours: boolean;
  lateStart: string;
  lateEnd: string;
  /** Pause everything while Do Not Disturb is on (presentations, films). Off by default: health first. */
  pauseInDnd: boolean;
  /** Seconds without input after which the person counts as "away". */
  idleSec: number;
}

export const DEFAULT_EYECARE: EyeCareConfig = {
  enabled: false,
  mode: 'gentle',
  rule2020: true,
  lookAwayEveryMin: 20,
  lookAwaySec: 20,
  longBreak: true,
  longBreakEveryMin: 60,
  longBreakMin: 5,
  blink: true,
  blinkEveryMin: 10,
  posture: true,
  postureEveryMin: 30,
  stretch: true,
  stretchEveryMin: 45,
  water: false,
  waterEveryMin: 60,
  dailyLimit: true,
  dailyLimitMin: 480,
  lateHours: true,
  lateStart: '22:00',
  lateEnd: '06:00',
  pauseInDnd: false,
  idleSec: 60,
};

export type EyeCarePreset = 'recommended' | 'light' | 'intense';

export const PRESETS: Record<EyeCarePreset, Partial<EyeCareConfig>> = {
  light: {
    mode: 'gentle', rule2020: true, lookAwayEveryMin: 30, lookAwaySec: 20, longBreak: true, longBreakEveryMin: 90, longBreakMin: 5,
    blink: false, posture: false, stretch: true, stretchEveryMin: 60, water: false, dailyLimit: true, dailyLimitMin: 600,
  },
  recommended: {
    mode: 'gentle', rule2020: true, lookAwayEveryMin: 20, lookAwaySec: 20, longBreak: true, longBreakEveryMin: 60, longBreakMin: 5,
    blink: true, blinkEveryMin: 10, posture: true, postureEveryMin: 30, stretch: true, stretchEveryMin: 45, water: false, dailyLimit: true, dailyLimitMin: 480,
  },
  intense: {
    mode: 'firm', rule2020: true, lookAwayEveryMin: 20, lookAwaySec: 30, longBreak: true, longBreakEveryMin: 45, longBreakMin: 10,
    blink: true, blinkEveryMin: 8, posture: true, postureEveryMin: 25, stretch: true, stretchEveryMin: 30, water: true, waterEveryMin: 45, dailyLimit: true, dailyLimitMin: 360,
  },
};

export const LIMITS = {
  lookAwayEveryMin: [5, 60], lookAwaySec: [10, 120], longBreakEveryMin: [15, 180], longBreakMin: [1, 30],
  blinkEveryMin: [2, 60], postureEveryMin: [10, 120], stretchEveryMin: [10, 180], waterEveryMin: [15, 180],
  dailyLimitMin: [60, 960], idleSec: [20, 600],
} as const;

const HHMM = /^(\d{1,2}):(\d{2})$/;
export function parseHHMM(s: string): number | null {
  const m = HHMM.exec((s ?? '').trim());
  if (!m) return null;
  const h = Number(m[1]);
  const mi = Number(m[2]);
  return h < 24 && mi < 60 ? h * 60 + mi : null;
}

export function inWindow(start: string, end: string, minuteOfDay: number): boolean {
  const s = parseHHMM(start);
  const e = parseHHMM(end);
  if (s === null || e === null || s === e) return false;
  return s < e ? minuteOfDay >= s && minuteOfDay < e : minuteOfDay >= s || minuteOfDay < e;
}

export function normalizeEyeCare(raw: unknown): EyeCareConfig {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const D = DEFAULT_EYECARE;
  const bool = (k: keyof EyeCareConfig) => (typeof r[k] === 'boolean' ? (r[k] as boolean) : (D[k] as boolean));
  const num = (k: keyof typeof LIMITS) => {
    const [lo, hi] = LIMITS[k];
    const v = r[k];
    return typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, Math.round(v))) : (D[k] as number);
  };
  const time = (k: 'lateStart' | 'lateEnd') => (typeof r[k] === 'string' && parseHHMM(r[k] as string) !== null ? (r[k] as string) : D[k]);
  const cfg: EyeCareConfig = {
    enabled: bool('enabled'),
    mode: ['gentle', 'firm', 'strict'].includes(r.mode as string) ? (r.mode as EyeCareMode) : D.mode,
    rule2020: bool('rule2020'), lookAwayEveryMin: num('lookAwayEveryMin'), lookAwaySec: num('lookAwaySec'),
    longBreak: bool('longBreak'), longBreakEveryMin: num('longBreakEveryMin'), longBreakMin: num('longBreakMin'),
    blink: bool('blink'), blinkEveryMin: num('blinkEveryMin'),
    posture: bool('posture'), postureEveryMin: num('postureEveryMin'),
    stretch: bool('stretch'), stretchEveryMin: num('stretchEveryMin'),
    water: bool('water'), waterEveryMin: num('waterEveryMin'),
    dailyLimit: bool('dailyLimit'), dailyLimitMin: num('dailyLimitMin'),
    lateHours: bool('lateHours'), lateStart: time('lateStart'), lateEnd: time('lateEnd'),
    pauseInDnd: bool('pauseInDnd'), idleSec: num('idleSec'),
  };
  // a look-away has to be shorter than the interval it repeats in
  if (cfg.lookAwaySec >= cfg.lookAwayEveryMin * 60) cfg.lookAwaySec = Math.max(10, cfg.lookAwayEveryMin * 60 - 10);
  return cfg;
}

// ── runtime (what has happened today) ──────────────────────────────────────

export interface EyeCareRuntime {
  /** Local date "YYYY-MM-DD" these counters belong to. */
  day: string;
  activeSec: number;
  sinceLookAway: number;
  sinceLongBreak: number;
  sinceBlink: number;
  sincePosture: number;
  sinceStretch: number;
  sinceWater: number;
  /** Epoch ms; nothing is shown before this (snooze / "later"). */
  snoozeUntil: number;
  /** Seconds of activity over the limit already warned about. */
  limitWarnedAtSec: number;
  lookAways: number;
  longBreaks: number;
  skipped: number;
  /** Consecutive taken breaks (reset when one is skipped). */
  streak: number;
  bestStreak: number;
}

export const isoDay = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

export function freshRuntime(now: Date): EyeCareRuntime {
  return {
    day: isoDay(now), activeSec: 0, sinceLookAway: 0, sinceLongBreak: 0, sinceBlink: 0, sincePosture: 0, sinceStretch: 0, sinceWater: 0,
    snoozeUntil: 0, limitWarnedAtSec: 0, lookAways: 0, longBreaks: 0, skipped: 0, streak: 0, bestStreak: 0,
  };
}

export type EyeCareEvent =
  | { type: 'lookAway'; seconds: number }
  | { type: 'longBreak'; seconds: number }
  | { type: 'blink' }
  | { type: 'posture' }
  | { type: 'stretch' }
  | { type: 'water' }
  | { type: 'limit'; overMin: number };

/** Interval scale: late at night the eyes tire faster, so breaks come ~30 % more often. */
export function lateFactor(cfg: EyeCareConfig, now: Date): number {
  return cfg.lateHours && inWindow(cfg.lateStart, cfg.lateEnd, now.getHours() * 60 + now.getMinutes()) ? 0.7 : 1;
}

/**
 * Advances the counters by `dtSec` seconds. `idleSec` is how long the person
 * has not touched mouse/keyboard. Returns the next runtime and at most ONE
 * event to show (most important first: long break > look-away > limit >
 * the gentle reminders) — several reminders never stack on top of each other.
 */
export function advance(
  rt: EyeCareRuntime, cfg: EyeCareConfig, dtSec: number, idleSec: number, now: Date, nowMs: number = now.getTime(),
): { rt: EyeCareRuntime; event: EyeCareEvent | null } {
  let next: EyeCareRuntime = rt.day === isoDay(now) ? { ...rt } : freshRuntime(now);
  if (!cfg.enabled) return { rt: next, event: null };

  // Natural break: the person stepped away from the computer.
  const away = idleSec >= cfg.idleSec;
  if (away) {
    if (idleSec >= cfg.lookAwaySec) { next.sinceLookAway = 0; next.sinceBlink = 0; }
    if (idleSec >= cfg.longBreakMin * 60) {
      next.sinceLongBreak = 0; next.sinceStretch = 0; next.sincePosture = 0; next.sinceWater = 0;
    }
    return { rt: next, event: null };
  }

  const dt = Math.max(0, dtSec);
  next.activeSec += dt;
  next.sinceLookAway += dt; next.sinceLongBreak += dt; next.sinceBlink += dt;
  next.sincePosture += dt; next.sinceStretch += dt; next.sinceWater += dt;

  if (nowMs < next.snoozeUntil) return { rt: next, event: null };

  const f = lateFactor(cfg, now);
  const due = (since: number, everyMin: number) => since >= everyMin * 60 * f;

  if (cfg.longBreak && due(next.sinceLongBreak, cfg.longBreakEveryMin)) {
    return { rt: next, event: { type: 'longBreak', seconds: cfg.longBreakMin * 60 } };
  }
  if (cfg.rule2020 && due(next.sinceLookAway, cfg.lookAwayEveryMin)) {
    return { rt: next, event: { type: 'lookAway', seconds: cfg.lookAwaySec } };
  }
  if (cfg.dailyLimit) {
    const limitSec = cfg.dailyLimitMin * 60;
    // first warning at the limit, then every 30 minutes beyond it
    if (next.activeSec >= limitSec && (next.limitWarnedAtSec === 0 || next.activeSec - next.limitWarnedAtSec >= 1800)) {
      next.limitWarnedAtSec = next.activeSec;
      return { rt: next, event: { type: 'limit', overMin: Math.floor((next.activeSec - limitSec) / 60) } };
    }
  }
  if (cfg.blink && due(next.sinceBlink, cfg.blinkEveryMin)) { next.sinceBlink = 0; return { rt: next, event: { type: 'blink' } }; }
  if (cfg.posture && due(next.sincePosture, cfg.postureEveryMin)) { next.sincePosture = 0; return { rt: next, event: { type: 'posture' } }; }
  if (cfg.stretch && due(next.sinceStretch, cfg.stretchEveryMin)) { next.sinceStretch = 0; return { rt: next, event: { type: 'stretch' } }; }
  if (cfg.water && due(next.sinceWater, cfg.waterEveryMin)) { next.sinceWater = 0; return { rt: next, event: { type: 'water' } }; }
  return { rt: next, event: null };
}

/** A break was completed: reset the matching timers and the stats. */
export function completeBreak(rt: EyeCareRuntime, kind: 'lookAway' | 'longBreak'): EyeCareRuntime {
  const n = { ...rt, streak: rt.streak + 1 };
  n.bestStreak = Math.max(n.bestStreak, n.streak);
  n.sinceLookAway = 0; n.sinceBlink = 0;
  if (kind === 'longBreak') { n.longBreaks += 1; n.sinceLongBreak = 0; n.sinceStretch = 0; n.sincePosture = 0; n.sinceWater = 0; }
  else n.lookAways += 1;
  return n;
}

/** A break was skipped / postponed: ask again in `retryMin` minutes. */
export function skipBreak(rt: EyeCareRuntime, nowMs: number, retryMin = 5): EyeCareRuntime {
  return { ...rt, skipped: rt.skipped + 1, streak: 0, snoozeUntil: nowMs + retryMin * 60_000 };
}

/** 0–100 "health score" of today: how many due breaks were really taken. */
export function healthScore(rt: EyeCareRuntime, cfg: EyeCareConfig): number {
  const limitPenalty = cfg.dailyLimit && rt.activeSec > cfg.dailyLimitMin * 60 ? Math.min(30, Math.floor((rt.activeSec - cfg.dailyLimitMin * 60) / 600) * 5) : 0;
  const taken = rt.lookAways + rt.longBreaks;
  const total = taken + rt.skipped;
  const base = total === 0 ? 100 : Math.round((taken / total) * 100);
  return Math.max(0, base - limitPenalty);
}

export function formatDuration(totalSec: number): { h: number; m: number } {
  const m = Math.floor(Math.max(0, totalSec) / 60);
  return { h: Math.floor(m / 60), m: m % 60 };
}

/** Emergency exit for strict mode: `count` Escape presses within `windowMs`. */
export function escapeBurst(times: number[], nowMs: number, count = 3, windowMs = 1500): { fired: boolean; times: number[] } {
  const kept = [...times, nowMs].filter((x) => nowMs - x <= windowMs);
  return kept.length >= count ? { fired: true, times: [] } : { fired: false, times: kept };
}

// ── persistence + stores ───────────────────────────────────────────────────

const CFG_KEY = 'blue.eyecare.v1';
const RT_KEY = 'blue.eyecare.stats.v1';

function loadCfg(): EyeCareConfig {
  try {
    const v = typeof localStorage !== 'undefined' ? localStorage.getItem(CFG_KEY) : null;
    return normalizeEyeCare(v ? JSON.parse(v) : null);
  } catch { return { ...DEFAULT_EYECARE }; }
}

function loadRt(): EyeCareRuntime {
  const fresh = freshRuntime(new Date());
  try {
    const v = typeof localStorage !== 'undefined' ? localStorage.getItem(RT_KEY) : null;
    const p = v ? JSON.parse(v) : null;
    if (p && typeof p === 'object' && p.day === fresh.day) return { ...fresh, ...p };
  } catch { /* ignore */ }
  return fresh;
}

export const eyeCareConfig = writable<EyeCareConfig>(loadCfg());
export const eyeCareStats = writable<EyeCareRuntime>(loadRt());

/** The break on screen right now, or null. */
export interface ActiveBreak {
  kind: 'lookAway' | 'longBreak';
  startedAt: number;
  endsAt: number;
  totalSec: number;
  /** Full-screen (long breaks, strict mode) or a small corner card. */
  fullscreen: boolean;
  /** Skip allowed from this moment on (firm mode: after a short delay). */
  skipAfter: number | null;
}
export const activeBreak = writable<ActiveBreak | null>(null);

function saveCfg(c: EyeCareConfig) { try { localStorage.setItem(CFG_KEY, JSON.stringify(c)); } catch { /* ignore */ } }
function saveRt(r: EyeCareRuntime) { try { localStorage.setItem(RT_KEY, JSON.stringify(r)); } catch { /* ignore */ } }

export function setEyeCare(patch: Partial<EyeCareConfig>) {
  eyeCareConfig.update((cur) => { const next = normalizeEyeCare({ ...cur, ...patch }); saveCfg(next); return next; });
}

export function applyPreset(p: EyeCarePreset) {
  setEyeCare({ ...PRESETS[p], enabled: true });
}

export function resetEyeCareStats() {
  const r = freshRuntime(new Date());
  saveRt(r); eyeCareStats.set(r);
}

export function beginBreak(kind: 'lookAway' | 'longBreak', seconds: number, nowMs = Date.now()) {
  const cfg = get(eyeCareConfig);
  const fullscreen = kind === 'longBreak' || cfg.mode === 'strict';
  const skipAfter = cfg.mode === 'strict' ? null : cfg.mode === 'firm' ? nowMs + 5000 : nowMs;
  activeBreak.set({ kind, startedAt: nowMs, endsAt: nowMs + seconds * 1000, totalSec: seconds, fullscreen, skipAfter });
}

export function finishBreak(taken: boolean, nowMs = Date.now(), retryMin = 5) {
  const b = get(activeBreak);
  if (!b) return;
  activeBreak.set(null);
  eyeCareStats.update((r) => { const n = taken ? completeBreak(r, b.kind) : skipBreak(r, nowMs, retryMin); saveRt(n); return n; });
}

/** "Postpone": not counted as skipped, just asked again later. */
export function postponeBreak(nowMs = Date.now(), minutes = 5) {
  activeBreak.set(null);
  eyeCareStats.update((r) => { const n = { ...r, snoozeUntil: nowMs + minutes * 60_000 }; saveRt(n); return n; });
}

function toast(title: string, message: string) {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent('blue:show-toast', { detail: { id: `eyecare-${Date.now()}`, title, message, urgent: true } }));
}

/**
 * Starts the engine; returns a cleanup function. `translate` is injected so
 * the store stays free of UI imports (and testable).
 * `isDndActive` lets the engine honour the "pause in Do Not Disturb" option.
 */
export function startEyeCareEngine(
  translate: (key: string, params?: Record<string, string | number>) => string,
  isDndActive: () => boolean = () => false,
): () => void {
  if (typeof window === 'undefined') return () => {};
  let lastInput = Date.now();
  let lastTick = Date.now();
  let escTimes: number[] = [];

  const onInput = () => { lastInput = Date.now(); };
  const onKey = (e: KeyboardEvent) => {
    lastInput = Date.now();
    if (e.key !== 'Escape' || !get(activeBreak)) return;
    const burst = escapeBurst(escTimes, Date.now());
    escTimes = burst.times;
    if (burst.fired) finishBreak(false, Date.now(), 10);
  };
  const evs: (keyof WindowEventMap)[] = ['mousemove', 'mousedown', 'wheel', 'touchstart', 'pointerdown'];
  evs.forEach((e) => window.addEventListener(e, onInput, { passive: true }));
  window.addEventListener('keydown', onKey);

  const timer = setInterval(() => {
    const nowMs = Date.now();
    // a sleeping laptop / suspended tab must not count as work: cap the step
    const dt = Math.min(10, Math.max(0, (nowMs - lastTick) / 1000));
    const gap = nowMs - lastTick;
    lastTick = nowMs;
    const cfg = get(eyeCareConfig);
    if (!cfg.enabled) { if (get(activeBreak)) activeBreak.set(null); return; }
    const now = new Date(nowMs);

    // a break in progress: count down, finish at zero
    const br = get(activeBreak);
    if (br) {
      if (nowMs >= br.endsAt) {
        finishBreak(true, nowMs);
        toast(translate('eyecare.toast.done_title'), translate(br.kind === 'longBreak' ? 'eyecare.toast.done_long' : 'eyecare.toast.done_look'));
      }
      return;
    }
    if (gap > 60_000) lastInput = 0; // woke from suspend: treat as away
    if (cfg.pauseInDnd && isDndActive()) return;

    const { rt, event } = advance(get(eyeCareStats), cfg, dt, (nowMs - lastInput) / 1000, now, nowMs);
    saveRt(rt);
    eyeCareStats.set(rt);
    if (!event) return;
    switch (event.type) {
      case 'lookAway': beginBreak('lookAway', event.seconds, nowMs); break;
      case 'longBreak': beginBreak('longBreak', event.seconds, nowMs); break;
      case 'blink': toast(translate('eyecare.toast.blink_title'), translate('eyecare.toast.blink')); break;
      case 'posture': toast(translate('eyecare.toast.posture_title'), translate('eyecare.toast.posture')); break;
      case 'stretch': toast(translate('eyecare.toast.stretch_title'), translate('eyecare.toast.stretch')); break;
      case 'water': toast(translate('eyecare.toast.water_title'), translate('eyecare.toast.water')); break;
      case 'limit': toast(translate('eyecare.toast.limit_title'), translate('eyecare.toast.limit', { n: Math.max(1, event.overMin) })); break;
    }
  }, 1000);

  return () => {
    clearInterval(timer);
    evs.forEach((e) => window.removeEventListener(e, onInput));
    window.removeEventListener('keydown', onKey);
  };
}
