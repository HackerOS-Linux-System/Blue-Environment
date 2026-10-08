export type BarBlur = 'none' | 'sm' | 'md' | 'xl';
export type BarClockStyle = 'time' | 'time-date' | 'date-time';

export interface TopBarConfig {
  // ── Elementy ───────────────────────────────────────────────
  showStartButton: boolean;
  showSearch: boolean;
  showPinned: boolean;
  showWorkspaces: boolean;
  showWeather: boolean;
  showBattery: boolean;
  showClipboard: boolean;
  showNotifications: boolean;
  showNetworkIcon: boolean;
  showClock: boolean;
  // ── Zegar ──────────────────────────────────────────────────
  clockStyle: BarClockStyle;
  clock24h: boolean;
  showSeconds: boolean;
  // ── Wygląd ─────────────────────────────────────────────────
  startLabel: string;       // własny tekst obok ikony Start (pusty = "Blue")
  pinnedIconSize: number;   // px, 14–32
  searchWidth: number;      // px, 120–320
  floating: boolean;        // pływający pasek z marginesem i zaokrągleniem
  floatingMargin: number;   // px, 0–24
  cornerRadius: number;     // px, 0–28 (działa przy floating)
  blur: BarBlur;
  showBorder: boolean;
  accentIndicators: boolean;
  autoHide: boolean;        // chowa pasek, dopóki kursor nie dotknie krawędzi
}

export const DEFAULT_TOP_BAR: TopBarConfig = {
  showStartButton: true,
  showSearch: true,
  showPinned: true,
  showWorkspaces: true,
  showWeather: true,
  showBattery: true,
  showClipboard: true,
  showNotifications: true,
  showNetworkIcon: true,
  showClock: true,
  clockStyle: 'time',
  clock24h: true,
  showSeconds: false,
  startLabel: '',
  pinnedIconSize: 20,
  searchWidth: 176,
  floating: false,
  floatingMargin: 8,
  cornerRadius: 16,
  blur: 'sm',
  showBorder: true,
  accentIndicators: true,
  autoHide: false,
};

const clamp = (v: unknown, lo: number, hi: number, d: number) =>
  typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : d;
const bool = (v: unknown, d: boolean) => (typeof v === 'boolean' ? v : d);

export function normalizeTopBar(raw: Partial<TopBarConfig> | undefined | null): TopBarConfig {
  const r: Partial<TopBarConfig> = raw && typeof raw === 'object' ? raw : {};
  const d = DEFAULT_TOP_BAR;
  return {
    showStartButton: bool(r.showStartButton, d.showStartButton),
    showSearch: bool(r.showSearch, d.showSearch),
    showPinned: bool(r.showPinned, d.showPinned),
    showWorkspaces: bool(r.showWorkspaces, d.showWorkspaces),
    showWeather: bool(r.showWeather, d.showWeather),
    showBattery: bool(r.showBattery, d.showBattery),
    showClipboard: bool(r.showClipboard, d.showClipboard),
    showNotifications: bool(r.showNotifications, d.showNotifications),
    showNetworkIcon: bool(r.showNetworkIcon, d.showNetworkIcon),
    showClock: bool(r.showClock, d.showClock),
    clockStyle: (['time', 'time-date', 'date-time'] as const).includes(r.clockStyle as any)
      ? (r.clockStyle as BarClockStyle) : d.clockStyle,
    clock24h: bool(r.clock24h, d.clock24h),
    showSeconds: bool(r.showSeconds, d.showSeconds),
    startLabel: typeof r.startLabel === 'string' ? r.startLabel.slice(0, 24) : d.startLabel,
    pinnedIconSize: clamp(r.pinnedIconSize, 14, 32, d.pinnedIconSize),
    searchWidth: clamp(r.searchWidth, 120, 320, d.searchWidth),
    floating: bool(r.floating, d.floating),
    floatingMargin: clamp(r.floatingMargin, 0, 24, d.floatingMargin),
    cornerRadius: clamp(r.cornerRadius, 0, 28, d.cornerRadius),
    blur: (['none', 'sm', 'md', 'xl'] as const).includes(r.blur as any) ? (r.blur as BarBlur) : d.blur,
    showBorder: bool(r.showBorder, d.showBorder),
    accentIndicators: bool(r.accentIndicators, d.accentIndicators),
    autoHide: bool(r.autoHide, d.autoHide),
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
