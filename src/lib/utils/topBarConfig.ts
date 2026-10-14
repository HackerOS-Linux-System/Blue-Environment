import { RUNNING_APPS_DEFAULT_MAX, clampRunningMax } from './runningApps';

export type BarBlur = 'none' | 'sm' | 'md' | 'xl';
export type BarClockStyle = 'time' | 'time-date' | 'date-time';
/** Where the bar's background colour comes from. */
export type BarBackgroundMode = 'theme' | 'custom';
export type BarStartStyle = 'rounded' | 'pill' | 'square';
export type BarPinnedStyle = 'dock' | 'flat';
export type BarIndicatorStyle = 'line' | 'dot' | 'glow';
export type BarWorkspaceStyle = 'dots' | 'numbers';
/**
 * Right-hand widgets whose order can be changed (the IME badge always stays
 * first). `showDesktop` and `power` are the two widgets that are OFF until the
 * person adds them (`showDesktopButton` / `showPowerButton`).
 */
export type BarRightItem =
  | 'workspaces' | 'weather' | 'battery' | 'clipboard' | 'network' | 'notifications' | 'showDesktop' | 'power';

export const BAR_RIGHT_ITEMS: readonly BarRightItem[] = [
  'workspaces', 'weather', 'battery', 'clipboard', 'network', 'notifications', 'showDesktop', 'power',
];

/** Left-hand items whose order can be changed. */
export type BarLeftItem = 'start' | 'running';
export const BAR_LEFT_ITEMS: readonly BarLeftItem[] = ['start', 'running'];

export interface TopBarConfig {
  // ── Elementy ───────────────────────────────────────────────
  showStartButton: boolean;
  /** Strip of running apps next to the Start button (replaced the old search pill). */
  showRunningApps: boolean;
  showPinned: boolean;
  showWorkspaces: boolean;
  showWeather: boolean;
  showBattery: boolean;
  showClipboard: boolean;
  showNotifications: boolean;
  showNetworkIcon: boolean;
  showClock: boolean;
  /** "Show desktop" button (minimizes everything). Off by default — add it in Settings → Bar. */
  showDesktopButton: boolean;
  /** Power button (opens the power menu). Off by default — add it in Settings → Bar. */
  showPowerButton: boolean;
  // ── Zegar ──────────────────────────────────────────────────
  clockStyle: BarClockStyle;
  clock24h: boolean;
  showSeconds: boolean;
  // ── Wygląd ─────────────────────────────────────────────────
  startLabel: string;       // własny tekst obok ikony Start (pusty = "Blue")
  pinnedIconSize: number;   // px, 14–32
  runningAppsMax: number;   // ile uruchomionych aplikacji widać naraz (1–12); resztę pokazuje „+N” / podwójne kliknięcie
  runningAppsLabels: boolean; // nazwa obok ikony (na szerokich ekranach)
  floating: boolean;        // pływający pasek z marginesem i zaokrągleniem
  floatingMargin: number;   // px, 0–24
  cornerRadius: number;     // px, 0–28 (działa przy floating)
  blur: BarBlur;
  showBorder: boolean;
  accentIndicators: boolean;
  autoHide: boolean;        // chowa pasek, dopóki kursor nie dotknie krawędzi
  // ── Rozszerzone dostosowanie ───────────────────────────────
  backgroundMode: BarBackgroundMode; // 'theme' = kolor z motywu, 'custom' = własny kolor poniżej
  backgroundColor: string;  // #rrggbb (tylko gdy backgroundMode = 'custom')
  backgroundOpacity: number; // %, 10–100 (tylko gdy backgroundMode = 'custom')
  fontSize: number;         // px, 10–18 — zegar, pogoda, bateria
  itemSpacing: number;      // px, 0–16 — odstęp między widżetami po prawej
  startButtonStyle: BarStartStyle;
  pinnedStyle: BarPinnedStyle;
  indicatorStyle: BarIndicatorStyle;
  workspaceStyle: BarWorkspaceStyle;
  showBatteryPercent: boolean;
  shadow: boolean;          // cień pod paskiem
  rightOrder: BarRightItem[]; // kolejność widżetów po prawej stronie
  leftOrder: BarLeftItem[];   // kolejność elementów po lewej stronie (Start, uruchomione aplikacje)
}

export const DEFAULT_TOP_BAR: TopBarConfig = {
  showStartButton: true,
  showRunningApps: true,
  showPinned: true,
  showWorkspaces: true,
  showWeather: true,
  showBattery: true,
  showClipboard: true,
  showNotifications: true,
  showNetworkIcon: true,
  showClock: true,
  showDesktopButton: false,
  showPowerButton: false,
  clockStyle: 'time',
  clock24h: true,
  showSeconds: false,
  startLabel: '',
  pinnedIconSize: 20,
  runningAppsMax: RUNNING_APPS_DEFAULT_MAX,
  runningAppsLabels: true,
  floating: false,
  floatingMargin: 8,
  cornerRadius: 16,
  blur: 'sm',
  showBorder: true,
  accentIndicators: true,
  autoHide: false,
  backgroundMode: 'theme',
  backgroundColor: '#0f172a',
  backgroundOpacity: 95,
  fontSize: 12,
  itemSpacing: 8,
  startButtonStyle: 'rounded',
  pinnedStyle: 'dock',
  indicatorStyle: 'line',
  workspaceStyle: 'dots',
  showBatteryPercent: true,
  shadow: false,
  rightOrder: [...BAR_RIGHT_ITEMS],
  leftOrder: [...BAR_LEFT_ITEMS],
};

const clamp = (v: unknown, lo: number, hi: number, d: number) =>
  typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : d;
const bool = (v: unknown, d: boolean) => (typeof v === 'boolean' ? v : d);
const oneOf = <T extends string>(v: unknown, allowed: readonly T[], d: T): T =>
  (allowed as readonly unknown[]).includes(v) ? (v as T) : d;

/**
 * Cleans a saved order: drops unknown/duplicate entries and appends any item
 * that is missing (e.g. one added in a later version), in default order.
 */
export function normalizeOrder<T extends string>(raw: unknown, all: readonly T[]): T[] {
  const out: T[] = [];
  if (Array.isArray(raw)) {
    for (const x of raw) {
      if ((all as readonly unknown[]).includes(x) && !out.includes(x as T)) out.push(x as T);
    }
  }
  for (const item of all) if (!out.includes(item)) out.push(item);
  return out;
}

export const normalizeRightOrder = (raw: unknown): BarRightItem[] => normalizeOrder(raw, BAR_RIGHT_ITEMS);
export const normalizeLeftOrder = (raw: unknown): BarLeftItem[] => normalizeOrder(raw, BAR_LEFT_ITEMS);

/** Returns a copy of `order` with the item at `index` moved by `delta` positions (clamped). */
export function moveItem<T>(order: T[], index: number, delta: number): T[] {
  return moveItemTo(order, index, index + delta);
}

/** Returns a copy of `order` with the item at `from` moved to position `to` (the drag-and-drop case). */
export function moveItemTo<T>(order: T[], from: number, to: number): T[] {
  if (from < 0 || from >= order.length || to < 0 || to >= order.length || from === to) return order;
  const next = [...order];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}
export const moveRightItem = moveItem<BarRightItem>;

/** CSS `order` value for an item (1-based; the IME badge has no order = 0, so it stays first). */
export function itemOrder<T extends string>(order: readonly T[], item: T): number {
  return order.indexOf(item) + 1;
}
export const rightItemOrder = (order: BarRightItem[], item: BarRightItem) => itemOrder(order, item);

/** `#rgb`/`#rrggbb` only — the colour input of the settings page always yields this. */
export function normalizeHexColor(v: unknown, d: string): string {
  if (typeof v !== 'string') return d;
  const s = v.trim();
  if (/^#[0-9a-fA-F]{6}$/.test(s)) return s.toLowerCase();
  const short = /^#([0-9a-fA-F])([0-9a-fA-F])([0-9a-fA-F])$/.exec(s);
  return short ? `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`.toLowerCase() : d;
}

export function normalizeTopBar(raw: Partial<TopBarConfig> | undefined | null): TopBarConfig {
  const r: Partial<TopBarConfig> = raw && typeof raw === 'object' ? raw : {};
  const d = DEFAULT_TOP_BAR;
  return {
    showStartButton: bool(r.showStartButton, d.showStartButton),
    // Migration: whoever had hidden the old search pill (`showSearch: false`)
    // gets the running-apps strip hidden too, instead of it popping up unasked.
    showRunningApps: bool(r.showRunningApps, bool((r as { showSearch?: unknown }).showSearch, d.showRunningApps)),
    showPinned: bool(r.showPinned, d.showPinned),
    showWorkspaces: bool(r.showWorkspaces, d.showWorkspaces),
    showWeather: bool(r.showWeather, d.showWeather),
    showBattery: bool(r.showBattery, d.showBattery),
    showClipboard: bool(r.showClipboard, d.showClipboard),
    showNotifications: bool(r.showNotifications, d.showNotifications),
    showNetworkIcon: bool(r.showNetworkIcon, d.showNetworkIcon),
    showClock: bool(r.showClock, d.showClock),
    showDesktopButton: bool(r.showDesktopButton, d.showDesktopButton),
    showPowerButton: bool(r.showPowerButton, d.showPowerButton),
    clockStyle: (['time', 'time-date', 'date-time'] as const).includes(r.clockStyle as any)
      ? (r.clockStyle as BarClockStyle) : d.clockStyle,
    clock24h: bool(r.clock24h, d.clock24h),
    showSeconds: bool(r.showSeconds, d.showSeconds),
    startLabel: typeof r.startLabel === 'string' ? r.startLabel.slice(0, 24) : d.startLabel,
    pinnedIconSize: clamp(r.pinnedIconSize, 14, 32, d.pinnedIconSize),
    runningAppsMax: clampRunningMax(r.runningAppsMax),
    runningAppsLabels: bool(r.runningAppsLabels, d.runningAppsLabels),
    floating: bool(r.floating, d.floating),
    floatingMargin: clamp(r.floatingMargin, 0, 24, d.floatingMargin),
    cornerRadius: clamp(r.cornerRadius, 0, 28, d.cornerRadius),
    blur: (['none', 'sm', 'md', 'xl'] as const).includes(r.blur as any) ? (r.blur as BarBlur) : d.blur,
    showBorder: bool(r.showBorder, d.showBorder),
    accentIndicators: bool(r.accentIndicators, d.accentIndicators),
    autoHide: bool(r.autoHide, d.autoHide),
    backgroundMode: oneOf(r.backgroundMode, ['theme', 'custom'] as const, d.backgroundMode),
    backgroundColor: normalizeHexColor(r.backgroundColor, d.backgroundColor),
    backgroundOpacity: clamp(r.backgroundOpacity, 10, 100, d.backgroundOpacity),
    fontSize: clamp(r.fontSize, 10, 18, d.fontSize),
    itemSpacing: clamp(r.itemSpacing, 0, 16, d.itemSpacing),
    startButtonStyle: oneOf(r.startButtonStyle, ['rounded', 'pill', 'square'] as const, d.startButtonStyle),
    pinnedStyle: oneOf(r.pinnedStyle, ['dock', 'flat'] as const, d.pinnedStyle),
    indicatorStyle: oneOf(r.indicatorStyle, ['line', 'dot', 'glow'] as const, d.indicatorStyle),
    workspaceStyle: oneOf(r.workspaceStyle, ['dots', 'numbers'] as const, d.workspaceStyle),
    showBatteryPercent: bool(r.showBatteryPercent, d.showBatteryPercent),
    shadow: bool(r.shadow, d.shadow),
    rightOrder: normalizeRightOrder(r.rightOrder),
    leftOrder: normalizeLeftOrder(r.leftOrder),
  };
}

export function formatBarClock(d: Date, bar: TopBarConfig): string {
  const time = d.toLocaleTimeString([], {
    hour: '2-digit', minute: '2-digit',
    second: bar.showSeconds ? '2-digit' : undefined,
    hour12: !bar.clock24h,
  });
  const date = d.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' });
  if (bar.clockStyle === 'time-date') return `${time} · ${date}`;
  if (bar.clockStyle === 'date-time') return `${date} · ${time}`;
  return time;
}

export const BLUR_CLASS: Record<BarBlur, string> = {
  none: '', sm: 'backdrop-blur-sm', md: 'backdrop-blur-md', xl: 'backdrop-blur-xl',
};
