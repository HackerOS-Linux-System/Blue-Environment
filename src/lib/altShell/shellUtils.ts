import { get } from 'svelte/store';
import { windows, activeWindowId, minimizeWindow } from '../stores/windowManager';

/**
 * "Back" for shells without a window frame: gives the foreground app (and any
 * overlay listening) a chance to handle it by cancelling the `blue:back`
 * event; returns `true` if something handled it. Apps can opt in with
 * `window.addEventListener('blue:back', (e) => { …; e.preventDefault(); })`.
 */
export function dispatchBack(): boolean {
  const ev = new CustomEvent('blue:back', { cancelable: true });
  window.dispatchEvent(ev);
  return ev.defaultPrevented;
}

/** Minimizes every open app so the home screen shows — the phone/TV "Home" button. */
export function goHome(): void {
  for (const w of get(windows)) if (!w.isMinimized) minimizeWindow(w.id);
  activeWindowId.set(null);
}

/** `HH:MM` for the status bars (respects the person's 12/24 h setting). */
export function formatClock(d: Date, use24h = true): string {
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: !use24h });
}

export function formatDate(d: Date): string {
  return d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' });
}
