import { writable } from 'svelte/store';

/**
 * "Application is not responding" — the KDE-style watchdog.
 *
 * Backend commands now run off the UI thread, so a slow call no longer
 * freezes the whole shell; but the *window that is waiting* should still
 * tell the user. An app wraps its long-running calls in `trackBusy()`; if one
 * is still pending after `thresholdMs`, Window.svelte shows an overlay
 * ("X is not responding — Wait / Close") for that window only. The overlay
 * disappears by itself the moment the call settles.
 */
export interface BusyInfo { label: string; since: number; }
export const appHealth = writable<Record<string, BusyInfo>>({});

const pending = new Map<string, Map<symbol, BusyInfo>>();

function publish(windowId: string) {
  const calls = pending.get(windowId);
  appHealth.update((h) => {
    const next = { ...h };
    const oldest = calls && calls.size ? [...calls.values()].sort((a, b) => a.since - b.since)[0] : null;
    if (oldest && Date.now() - oldest.since >= 0 && oldest.since > 0) next[windowId] = oldest; else delete next[windowId];
    return next;
  });
}

export const NOT_RESPONDING_AFTER_MS = 4000;

export function trackBusy<T>(windowId: string | undefined, label: string, p: Promise<T>, thresholdMs = NOT_RESPONDING_AFTER_MS): Promise<T> {
  if (!windowId) return p;
  const key = Symbol(label);
  const info: BusyInfo = { label, since: Date.now() };
  const timer = setTimeout(() => {
    if (!pending.has(windowId)) pending.set(windowId, new Map());
    pending.get(windowId)!.set(key, info);
    publish(windowId);
  }, thresholdMs);
  const done = () => {
    clearTimeout(timer);
    const m = pending.get(windowId);
    if (m?.delete(key)) { if (!m.size) pending.delete(windowId); publish(windowId); }
  };
  p.then(done, done);
  return p;
}

/** `invoke`-shaped helper bound to one window. */
export function watchedInvoke(windowId: string | undefined, invoke: <T>(cmd: string, args?: any) => Promise<T>) {
  return <T>(cmd: string, args?: any) => trackBusy(windowId, cmd, invoke<T>(cmd, args));
}
