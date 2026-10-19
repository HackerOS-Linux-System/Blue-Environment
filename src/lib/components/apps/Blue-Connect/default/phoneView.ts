export interface Box { left: number; top: number; width: number; height: number }

/** The rectangle an image of `aspect` (w/h) occupies inside a `cw`×`ch` box with `object-fit: contain`. */
export function fitRect(cw: number, ch: number, aspect: number): { x: number; y: number; w: number; h: number } {
  if (cw <= 0 || ch <= 0 || !(aspect > 0)) return { x: 0, y: 0, w: 0, h: 0 };
  const boxAspect = cw / ch;
  if (boxAspect > aspect) { const w = ch * aspect; return { x: (cw - w) / 2, y: 0, w, h: ch }; }
  const h = cw / aspect;
  return { x: 0, y: (ch - h) / 2, w: cw, h };
}

/**
 * Maps a pointer position (client coordinates) over the displayed screen to
 * device pixels. Returns null when the pointer is in the letterbox bars.
 */
export function mapPointToDevice(clientX: number, clientY: number, box: Box, deviceW: number, deviceH: number): { x: number; y: number } | null {
  if (deviceW <= 0 || deviceH <= 0) return null;
  const r = fitRect(box.width, box.height, deviceW / deviceH);
  const px = clientX - box.left - r.x;
  const py = clientY - box.top - r.y;
  if (px < 0 || py < 0 || px > r.w || py > r.h || r.w === 0) return null;
  return { x: Math.min(deviceW - 1, Math.round((px / r.w) * deviceW)), y: Math.min(deviceH - 1, Math.round((py / r.h) * deviceH)) };
}

export type Gesture =
  | { kind: 'tap'; x: number; y: number }
  | { kind: 'longPress'; x: number; y: number }
  | { kind: 'swipe'; x1: number; y1: number; x2: number; y2: number; ms: number };

/** Turns pointer-down/up (device coordinates + elapsed ms) into a tap, long press or swipe. */
export function classifyGesture(down: { x: number; y: number }, up: { x: number; y: number }, elapsedMs: number, slop = 14): Gesture {
  const dist = Math.hypot(up.x - down.x, up.y - down.y);
  if (dist <= slop) return elapsedMs >= 500 ? { kind: 'longPress', x: down.x, y: down.y } : { kind: 'tap', x: down.x, y: down.y };
  return { kind: 'swipe', x1: down.x, y1: down.y, x2: up.x, y2: up.y, ms: Math.max(80, Math.min(1200, Math.round(elapsedMs))) };
}

/** Mouse wheel → a scroll swipe centred on the pointer (wheel down = finger moves up). */
export function wheelToSwipe(deltaY: number, at: { x: number; y: number }, deviceH: number): Gesture | null {
  if (!deltaY) return null;
  const dist = Math.min(deviceH * 0.45, Math.max(deviceH * 0.12, Math.abs(deltaY) * 3));
  const dir = deltaY > 0 ? -1 : 1;
  const y2 = Math.round(Math.max(1, Math.min(deviceH - 1, at.y + dir * dist)));
  return { kind: 'swipe', x1: at.x, y1: at.y, x2: at.x, y2, ms: 180 };
}

export type KeyAction = { kind: 'key'; name: string } | { kind: 'text'; text: string };

const KEY_MAP: Record<string, string> = {
  Enter: 'enter', Backspace: 'delete', Delete: 'forward_delete', Tab: 'tab', Escape: 'back',
  ArrowUp: 'dpad_up', ArrowDown: 'dpad_down', ArrowLeft: 'dpad_left', ArrowRight: 'dpad_right',
  PageUp: 'page_up', PageDown: 'page_down', Home: 'move_home', End: 'move_end',
};

/** Keyboard event → phone input, or null when it should be ignored (shortcuts, non-ASCII, dead keys). */
export function keyFromEvent(e: { key: string; ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean }): KeyAction | null {
  if (e.ctrlKey || e.metaKey || e.altKey) return null;
  if (KEY_MAP[e.key]) return { kind: 'key', name: KEY_MAP[e.key] };
  if (e.key.length === 1 && e.key >= ' ' && e.key <= '~') return { kind: 'text', text: e.key };
  return null;
}

/** Merges consecutive text actions so fast typing becomes one `input text` call. */
export function coalesce(actions: KeyAction[]): KeyAction[] {
  const out: KeyAction[] = [];
  for (const a of actions) {
    const last = out[out.length - 1];
    if (a.kind === 'text' && last?.kind === 'text') last.text += a.text; else out.push({ ...a });
  }
  return out;
}

export interface PhoneApp { package: string; label: string }

export function filterApps(apps: PhoneApp[], q: string): PhoneApp[] {
  const s = q.trim().toLowerCase();
  const list = s ? apps.filter((a) => a.label.toLowerCase().includes(s) || a.package.toLowerCase().includes(s)) : apps;
  return [...list].sort((a, b) => a.label.localeCompare(b.label));
}

export function formatKb(kb: number): string {
  if (!kb) return '–';
  const gb = kb / 1024 / 1024;
  return gb >= 1 ? `${gb.toFixed(gb >= 100 ? 0 : 1)} GB` : `${Math.round(kb / 1024)} MB`;
}

export const parentRemote = (p: string): string => {
  const parts = p.split('/').filter(Boolean);
  parts.pop();
  return '/' + parts.join('/');
};
export const joinRemote = (base: string, name: string): string => (base.replace(/\/+$/, '') + '/' + name).replace(/\/+/g, '/');

/** `host:port` check mirroring the backend (clear message before a round-trip). */
export function validHostPort(s: string): boolean {
  const m = /^(.+):(\d{1,5})$/.exec(s.trim());
  if (!m) return false;
  const port = Number(m[2]);
  return port > 0 && port < 65536 && /^[A-Za-z0-9.\-[\]:]+$/.test(m[1]);
}
