export type WidgetType = 'clock' | 'weather' | 'notes' | 'system' | 'calendar';

export interface WidgetInstance {
  id: string;
  type: WidgetType;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Per-widget settings (clock: analog/hour12, weather: city, notes: text, calendar: monday-first). */
  config: Record<string, string | number | boolean>;
}

export const WIDGET_TYPES: WidgetType[] = ['clock', 'weather', 'notes', 'system', 'calendar'];

export const DEFAULT_SIZE: Record<WidgetType, { w: number; h: number }> = {
  clock: { w: 220, h: 120 },
  weather: { w: 220, h: 130 },
  notes: { w: 240, h: 180 },
  system: { w: 240, h: 150 },
  calendar: { w: 240, h: 240 },
};

export const MIN_SIZE = { w: 140, h: 90 };
export const GRID = 10;

export function snap(v: number, grid = GRID): number {
  return Math.round(v / grid) * grid;
}

export function clamp(v: number, lo: number, hi: number): number {
  return Math.min(Math.max(v, lo), Math.max(lo, hi));
}

/** Keeps a widget fully inside the viewport (and at least MIN_SIZE big). */
export function clampToViewport(w: WidgetInstance, vw: number, vh: number): WidgetInstance {
  const width = clamp(w.w, MIN_SIZE.w, Math.max(MIN_SIZE.w, vw));
  const height = clamp(w.h, MIN_SIZE.h, Math.max(MIN_SIZE.h, vh));
  return { ...w, w: width, h: height, x: clamp(w.x, 0, vw - width), y: clamp(w.y, 0, vh - height) };
}

let counter = 0;
export function newId(type: WidgetType): string {
  counter += 1;
  return `${type}-${Date.now().toString(36)}-${counter}`;
}

/** Creates a widget at the first free slot along the right edge (cascading downwards). */
export function createWidget(type: WidgetType, existing: WidgetInstance[], vw: number, vh: number): WidgetInstance {
  const { w, h } = DEFAULT_SIZE[type];
  const margin = 24;
  const x = snap(Math.max(0, vw - w - margin));
  let y = margin;
  const overlaps = (yy: number) => existing.some((o) => Math.abs(o.x - x) < w && yy < o.y + o.h && yy + h > o.y);
  while (overlaps(y) && y + h < vh) y += GRID;
  if (y + h > vh) y = margin; // screen is full — stack on top rather than disappearing off-screen
  return clampToViewport({ id: newId(type), type, x, y: snap(y), w, h, config: defaultConfig(type) }, vw, vh);
}

export function defaultConfig(type: WidgetType): WidgetInstance['config'] {
  switch (type) {
    case 'clock': return { analog: false, hour12: false, seconds: false };
    case 'weather': return { city: '' };
    case 'notes': return { text: '' };
    case 'calendar': return { mondayFirst: true };
    default: return {};
  }
}

/** Tolerant parser: unknown types/garbage are dropped instead of breaking the desktop. */
export function parseWidgets(raw: string | null | undefined): WidgetInstance[] {
  if (!raw) return [];
  let data: unknown;
  try { data = JSON.parse(raw); } catch { return []; }
  const list = Array.isArray(data) ? data : (data as { widgets?: unknown })?.widgets;
  if (!Array.isArray(list)) return [];
  const seen = new Set<string>();
  const out: WidgetInstance[] = [];
  for (const it of list) {
    if (!it || typeof it !== 'object') continue;
    const o = it as Partial<WidgetInstance>;
    if (!o.type || !WIDGET_TYPES.includes(o.type)) continue;
    const id = typeof o.id === 'string' && o.id && !seen.has(o.id) ? o.id : newId(o.type);
    seen.add(id);
    const def = DEFAULT_SIZE[o.type];
    const num = (v: unknown, d: number) => (typeof v === 'number' && Number.isFinite(v) ? v : d);
    out.push({
      id, type: o.type,
      x: num(o.x, 24), y: num(o.y, 24),
      w: Math.max(MIN_SIZE.w, num(o.w, def.w)), h: Math.max(MIN_SIZE.h, num(o.h, def.h)),
      config: { ...defaultConfig(o.type), ...(o.config && typeof o.config === 'object' ? o.config : {}) },
    });
  }
  return out;
}

export function serializeWidgets(list: WidgetInstance[]): string {
  return JSON.stringify({ version: 1, widgets: list }, null, 2);
}

export function updateWidget(list: WidgetInstance[], id: string, patch: Partial<WidgetInstance>): WidgetInstance[] {
  return list.map((w) => (w.id === id ? { ...w, ...patch, config: patch.config ? { ...w.config, ...patch.config } : w.config } : w));
}

/** Raises a widget to the top of the stacking order (end of the array). */
export function bringToFront(list: WidgetInstance[], id: string): WidgetInstance[] {
  const i = list.findIndex((w) => w.id === id);
  if (i < 0 || i === list.length - 1) return list;
  return [...list.slice(0, i), ...list.slice(i + 1), list[i]];
}

// ── helpers used by individual widgets ───────────────────────────────────

/** Appends a sample, keeping the last `max`. */
export function pushHistory(history: number[], value: number, max = 40): number[] {
  const next = history.length >= max ? history.slice(history.length - max + 1) : history.slice();
  next.push(Number.isFinite(value) ? value : 0);
  return next;
}

/** SVG polyline points for a sparkline, values scaled against `max` (default 100). */
export function sparklinePoints(values: number[], width: number, height: number, max = 100): string {
  if (values.length === 0) return '';
  const step = values.length > 1 ? width / (values.length - 1) : 0;
  return values
    .map((v, i) => `${(i * step).toFixed(1)},${(height - clamp(v / max, 0, 1) * height).toFixed(1)}`)
    .join(' ');
}

/** WMO weather code (Open-Meteo) → emoji. */
export function weatherGlyph(code: number): string {
  if (code === 0) return '☀️';
  if (code <= 2) return '🌤️';
  if (code === 3) return '☁️';
  if (code === 45 || code === 48) return '🌫️';
  if (code >= 51 && code <= 57) return '🌦️';
  if ((code >= 61 && code <= 67) || (code >= 80 && code <= 82)) return '🌧️';
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return '❄️';
  if (code >= 95) return '⛈️';
  return '🌡️';
}

export interface CalendarCell { day: number; inMonth: boolean; isToday: boolean; }

/** 6-row month grid (42 cells). `month` is 0-based. */
export function monthGrid(year: number, month: number, mondayFirst: boolean, today = new Date()): CalendarCell[] {
  const first = new Date(year, month, 1);
  const offset = mondayFirst ? (first.getDay() + 6) % 7 : first.getDay();
  const cells: CalendarCell[] = [];
  for (let i = 0; i < 42; i++) {
    const d = new Date(year, month, 1 - offset + i);
    cells.push({
      day: d.getDate(),
      inMonth: d.getMonth() === month,
      isToday: d.getFullYear() === today.getFullYear() && d.getMonth() === today.getMonth() && d.getDate() === today.getDate(),
    });
  }
  return cells;
}

/** Angles in degrees for an analog clock face. */
export function clockAngles(d: Date): { hour: number; minute: number; second: number } {
  const s = d.getSeconds(), m = d.getMinutes(), h = d.getHours() % 12;
  return { second: s * 6, minute: m * 6 + s * 0.1, hour: h * 30 + m * 0.5 };
}
