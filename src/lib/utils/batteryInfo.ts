import type { BatteryStatus } from './systemBridge';

export type BatteryPhase = 'charging' | 'discharging' | 'full' | 'plugged' | 'unknown';

/**
 * The one state the hover card headlines.
 *  - `plugged`: on the charger but NOT charging (battery-care limit, "Not charging").
 */
export function batteryPhase(b: Pick<BatteryStatus, 'charging' | 'status' | 'acOnline'>): BatteryPhase {
  if (b.charging || b.status === 'Charging') return 'charging';
  if (b.status === 'Full') return 'full';
  if (b.status === 'Discharging') return 'discharging';
  if (b.status === 'Not charging') return 'plugged';
  // "Unknown" (some drivers): the adapter tells us at least whether we're plugged in.
  return b.acOnline ? 'plugged' : 'unknown';
}

/** `{ h, m }` for a minute count; hours omitted below one hour. */
export function splitMinutes(total: number): { h: number; m: number } {
  const t = Math.max(0, Math.round(total));
  return { h: Math.floor(t / 60), m: t % 60 };
}

/** Watts with one decimal below 10 W, whole above — "9.4" / "32". */
export function formatWatts(w: number): string {
  return w < 10 ? w.toFixed(1) : String(Math.round(w));
}

/** Health worth mentioning: anything real, rounded. */
export function formatPercent(p: number): number {
  return Math.round(p);
}

/** Tailwind text colour class for the headline state. */
export function phaseColor(phase: BatteryPhase, percentage: number): string {
  if (phase === 'charging') return 'text-green-400';
  if (phase === 'full') return 'text-green-400';
  if (phase === 'plugged') return 'text-blue-300';
  if (percentage <= 15) return 'text-red-400';
  if (percentage <= 30) return 'text-amber-400';
  return 'text-slate-200';
}
