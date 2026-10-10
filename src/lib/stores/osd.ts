import { writable, get } from 'svelte/store';

/**
 * State of the on-screen display (OSD) shown when the volume or the brightness
 * changes — the same little overlay KDE Plasma pops up.
 *
 * Changes arrive as backend events (`osd:volume`, `osd:brightness`, see
 * src-tauri/src/shell_osd.rs and App.svelte), so the OSD shows no matter who
 * changed the level: media keys, a headset, another app… The one exception is
 * the shell's own sliders (Control Center, Settings): while the person is
 * dragging one, the slider IS the feedback, so `SystemBridge` calls
 * `noteLocalChange()` and the echo that comes back from the system is ignored.
 */

export type OsdKind = 'volume' | 'brightness';

export interface OsdState {
  kind: OsdKind;
  /** Percent. Volume may exceed 100 (amplified output, up to 150). */
  value: number;
  muted: boolean;
  visible: boolean;
  /** Bumped on every show, so the view can restart its hide animation. */
  seq: number;
}

export const OSD_VISIBLE_MS = 1800;
/** How long after a shell-initiated change the echo from the system is ignored. */
export const LOCAL_ECHO_MS = 900;

export const osd = writable<OsdState>({ kind: 'volume', value: 0, muted: false, visible: false, seq: 0 });

let enabled = true;
let hideTimer: ReturnType<typeof setTimeout> | undefined;
const localUntil: Record<OsdKind, number> = { volume: 0, brightness: 0 };

/** Settings → Panel → "On-screen display". Turning it off also hides a visible one. */
export function setOsdEnabled(value: boolean) {
  enabled = value;
  if (!value) hideOsd();
}

export function isOsdEnabled(): boolean {
  return enabled;
}

/** The shell itself is about to change this level (a slider, not a key). */
export function noteLocalChange(kind: OsdKind, now = Date.now()) {
  localUntil[kind] = now + LOCAL_ECHO_MS;
}

export function hideOsd() {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = undefined;
  osd.update((s) => (s.visible ? { ...s, visible: false } : s));
}

/**
 * Shows the OSD. `force` skips the local-echo filter (used when the shell
 * handled a media key itself and therefore knows the change was intentional).
 */
export function showOsd(kind: OsdKind, value: number, muted = false, force = false, now = Date.now()) {
  if (!enabled) return;
  if (!force && now < localUntil[kind]) return;
  const max = kind === 'volume' ? 150 : 100;
  const v = Math.round(Math.min(max, Math.max(0, Number.isFinite(value) ? value : 0)));
  osd.update((s) => ({ kind, value: v, muted: kind === 'volume' && muted, visible: true, seq: s.seq + 1 }));
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(hideOsd, OSD_VISIBLE_MS);
}

export type VolumeIconKind = 'muted' | 'low' | 'medium' | 'high';

/** Which speaker glyph fits the level (KDE: muted, low, medium, high). */
export function volumeIconKind(value: number, muted: boolean): VolumeIconKind {
  if (muted || value <= 0) return 'muted';
  if (value < 34) return 'low';
  if (value < 67) return 'medium';
  return 'high';
}

export type BrightnessIconKind = 'dim' | 'medium' | 'full';

export function brightnessIconKind(value: number): BrightnessIconKind {
  if (value < 34) return 'dim';
  if (value < 67) return 'medium';
  return 'full';
}

/** Width of the filled part of the bar, 0–100 (an amplified volume fills it entirely). */
export function barFill(value: number, muted: boolean): number {
  if (muted) return 0;
  return Math.min(100, Math.max(0, value));
}

export function currentOsd(): OsdState {
  return get(osd);
}
