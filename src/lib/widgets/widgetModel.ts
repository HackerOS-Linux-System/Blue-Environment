export type BuiltinWidgetType = 'clock' | 'weather' | 'notes' | 'system' | 'calendar';
/** Widgets that are installed from the Widget Store in Blue Software. */
export type StoreWidgetType = 'worldclock' | 'countdown' | 'quote' | 'links' | 'todo' | 'timer';
export type WidgetType = BuiltinWidgetType | StoreWidgetType;

export interface WidgetInstance {
  id: string;
  type: WidgetType;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Id of the Widget Store entry (preset) this instance was created from, if any. */
  storeId?: string;
  /** Per-widget settings (clock: analog/hour12, weather: city, notes: text, calendar: monday-first). */
  config: Record<string, string | number | boolean>;
}

export const BUILTIN_WIDGET_TYPES: BuiltinWidgetType[] = ['clock', 'weather', 'notes', 'system', 'calendar'];
export const STORE_WIDGET_TYPES: StoreWidgetType[] = ['worldclock', 'countdown', 'quote', 'links', 'todo', 'timer'];
export const WIDGET_TYPES: WidgetType[] = [...BUILTIN_WIDGET_TYPES, ...STORE_WIDGET_TYPES];

export const WIDGET_LABEL: Record<StoreWidgetType, string> = {
  worldclock: 'World clock', countdown: 'Countdown', quote: 'Quote', links: 'Quick links', todo: 'To-do list', timer: 'Timer',
};

export const DEFAULT_SIZE: Record<WidgetType, { w: number; h: number }> = {
  clock: { w: 220, h: 120 },
  weather: { w: 220, h: 130 },
  notes: { w: 240, h: 180 },
  system: { w: 240, h: 150 },
  calendar: { w: 240, h: 240 },
  worldclock: { w: 240, h: 170 },
  countdown: { w: 240, h: 140 },
  quote: { w: 280, h: 150 },
  links: { w: 220, h: 190 },
  todo: { w: 250, h: 230 },
  timer: { w: 220, h: 150 },
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
export interface WidgetPreset {
  storeId?: string;
  size?: { w: number; h: number };
  config?: WidgetInstance['config'];
}

export function createWidget(type: WidgetType, existing: WidgetInstance[], vw: number, vh: number, preset?: WidgetPreset): WidgetInstance {
  const { w, h } = preset?.size ? { w: Math.max(MIN_SIZE.w, preset.size.w), h: Math.max(MIN_SIZE.h, preset.size.h) } : DEFAULT_SIZE[type];
  const margin = 24;
  const x = snap(Math.max(0, vw - w - margin));
  let y = margin;
  const overlaps = (yy: number) => existing.some((o) => Math.abs(o.x - x) < w && yy < o.y + o.h && yy + h > o.y);
  while (overlaps(y) && y + h < vh) y += GRID;
  if (y + h > vh) y = margin; // screen is full — stack on top rather than disappearing off-screen
  return clampToViewport({ id: newId(type), type, x, y: snap(y), w, h, config: { ...defaultConfig(type), ...(preset?.config ?? {}) }, ...(preset?.storeId ? { storeId: preset.storeId } : {}) }, vw, vh);
}

export function defaultConfig(type: WidgetType): WidgetInstance['config'] {
  switch (type) {
    case 'clock': return { analog: false, hour12: false, seconds: false };
    case 'weather': return { city: '' };
    case 'notes': return { text: '' };
    case 'calendar': return { mondayFirst: true };
    case 'worldclock': return { zones: 'Europe/London,America/New_York,Asia/Tokyo', hour12: false };
    case 'countdown': return { label: 'New Year', date: `${new Date().getFullYear() + 1}-01-01T00:00` };
    case 'quote': return { quotes: DEFAULT_QUOTES.join('\n'), mode: 'daily' };
    case 'links': return { links: 'Blue Environment|https://github.com/HackerOS-Linux-System/Blue-Environment' };
    case 'todo': return { items: '[]' };
    case 'timer': return { mode: 'pomodoro', minutes: 25 };
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
      ...(typeof o.storeId === 'string' && o.storeId ? { storeId: o.storeId.slice(0, 64) } : {}),
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


// ── helpers for the Widget Store widgets ─────────────────────────────────

export const DEFAULT_QUOTES = [
  'Simplicity is the ultimate sophistication. — Leonardo da Vinci',
  'Programs must be written for people to read. — Harold Abelson',
  'Make it work, make it right, make it fast. — Kent Beck',
  'Talk is cheap. Show me the code. — Linus Torvalds',
  'Premature optimization is the root of all evil. — Donald Knuth',
];

/** Time left until `targetMs` as days/hours/minutes/seconds (never negative). */
export function countdownParts(targetMs: number, nowMs: number): { done: boolean; days: number; hours: number; minutes: number; seconds: number } {
  const diff = targetMs - nowMs;
  if (!Number.isFinite(diff) || diff <= 0) return { done: true, days: 0, hours: 0, minutes: 0, seconds: 0 };
  const s = Math.floor(diff / 1000);
  return { done: false, days: Math.floor(s / 86400), hours: Math.floor((s % 86400) / 3600), minutes: Math.floor((s % 3600) / 60), seconds: s % 60 };
}

/** Parses `YYYY-MM-DD` / `YYYY-MM-DDTHH:mm` (local time). `NaN` when invalid. */
export function parseTargetDate(text: string): number {
  const m = /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2}))?$/.exec(text.trim());
  if (!m) return NaN;
  const [y, mo, d, h, mi] = [m[1], m[2], m[3], m[4] ?? '0', m[5] ?? '0'].map(Number);
  const date = new Date(y, mo - 1, d, h, mi, 0, 0);
  // reject rollovers such as 2025-02-31
  return date.getFullYear() === y && date.getMonth() === mo - 1 && date.getDate() === d ? date.getTime() : NaN;
}

/** `Europe/Warsaw` → `Warsaw`. */
export function zoneLabel(zone: string): string {
  const last = zone.split('/').pop() ?? zone;
  return last.replace(/_/g, ' ');
}

/** Valid IANA zone names only (invalid ones are dropped; at most `max`). */
export function parseZones(text: string, max = 6): string[] {
  const out: string[] = [];
  for (const raw of text.split(/[,\n]/)) {
    const z = raw.trim();
    if (!z || out.includes(z)) continue;
    try { new Intl.DateTimeFormat('en', { timeZone: z }); out.push(z); } catch { /* unknown zone */ }
    if (out.length >= max) break;
  }
  return out;
}

export interface Quote { text: string; author: string }

/** One quote per line; `text — author` (em dash or ` - `) splits the author off. */
export function parseQuotes(text: string): Quote[] {
  return text.split('\n').map((l) => l.trim()).filter(Boolean).map((line) => {
    const m = /^(.*?)\s+(?:—|–|-{1,2})\s+([^—–]+)$/.exec(line);
    return m ? { text: m[1].trim(), author: m[2].trim() } : { text: line, author: '' };
  });
}

/** Same quote all day, a different one tomorrow. */
export function dailyIndex(length: number, date = new Date()): number {
  if (length <= 0) return 0;
  const day = Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000);
  return ((day % length) + length) % length;
}

export interface QuickLink { label: string; url: string }

/** Only plain web / mail links may be opened from a widget. */
export function isSafeLinkUrl(url: string): boolean {
  return /^(https?:\/\/[^\s]+|mailto:[^\s]+)$/i.test(url.trim()) && url.length <= 2048;
}

/** `Label|url` per line; lines with an unsafe url are skipped. */
export function parseLinks(text: string, max = 12): QuickLink[] {
  const out: QuickLink[] = [];
  for (const line of text.split('\n')) {
    const i = line.indexOf('|');
    if (i < 0) continue;
    const label = line.slice(0, i).trim();
    const url = line.slice(i + 1).trim();
    if (label && isSafeLinkUrl(url)) out.push({ label: label.slice(0, 60), url });
    if (out.length >= max) break;
  }
  return out;
}

export interface TodoItem { id: string; text: string; done: boolean }

export function parseTodos(raw: unknown): TodoItem[] {
  if (typeof raw !== 'string') return [];
  try {
    const data = JSON.parse(raw);
    if (!Array.isArray(data)) return [];
    return data
      .filter((i): i is { id?: unknown; text: string; done?: unknown } => !!i && typeof i === 'object' && typeof (i as { text?: unknown }).text === 'string')
      .slice(0, 100)
      .map((i, n) => ({ id: typeof i.id === 'string' && i.id ? i.id : `t${n}`, text: i.text.slice(0, 200), done: i.done === true }));
  } catch { return []; }
}

export function serializeTodos(items: TodoItem[]): string {
  return JSON.stringify(items.map(({ id, text, done }) => ({ id, text, done })));
}

/** `mm:ss` (or `h:mm:ss` from an hour up). */
export function formatTimer(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600), m = Math.floor((total % 3600) / 60), s = total % 60;
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`;
}
