export type HotCornerId = 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight';

export type HotCornerAction =
  | 'none' | 'start-menu' | 'fullscreen-menu' | 'show-desktop'
  | 'workspace-next' | 'workspace-prev' | 'window-switcher'
  | 'control-center' | 'notifications' | 'lock' | 'screenshot'
  | 'terminal' | 'open-app';

export interface HotCornerSlot {
  action: HotCornerAction;
  /** Używane tylko przy `open-app`. */
  appId?: string;
}

export interface HotCornersConfig {
  enabled: boolean;
  /** Rozmiar strefy w rogu, px. */
  triggerSize: number;
  /** Jak długo kursor musi być w rogu, ms (0 = natychmiast). */
  dwellMs: number;
  /** Przerwa po wyzwoleniu, zanim ten sam róg zadziała ponownie, ms. */
  cooldownMs: number;
  corners: Record<HotCornerId, HotCornerSlot>;
}

export const HOT_CORNER_IDS: HotCornerId[] = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight'];

/** `labelKey` to klucz tłumaczenia (i18n) — etykiety nie są wpisane na sztywno. */
export const HOT_CORNER_ACTIONS: { id: HotCornerAction; labelKey: string }[] = ([
  'none', 'start-menu', 'fullscreen-menu', 'show-desktop', 'workspace-next', 'workspace-prev',
  'window-switcher', 'control-center', 'notifications', 'lock', 'screenshot', 'terminal', 'open-app',
] as HotCornerAction[]).map((id) => ({ id, labelKey: `scorner.a.${id.replace(/-/g, '_')}` }));

export const DEFAULT_HOT_CORNERS: HotCornersConfig = {
  enabled: false, // domyślnie wyłączone — włącza się w Ustawieniach
  triggerSize: 6,
  dwellMs: 150,
  cooldownMs: 700,
  corners: {
    topLeft: { action: 'fullscreen-menu' },
    topRight: { action: 'notifications' },
    bottomLeft: { action: 'show-desktop' },
    bottomRight: { action: 'workspace-next' },
  },
};

const ACTION_IDS = new Set(HOT_CORNER_ACTIONS.map((a) => a.id));
const clamp = (v: unknown, lo: number, hi: number, d: number) =>
  typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : d;

export function normalizeHotCorners(raw: Partial<HotCornersConfig> | undefined | null): HotCornersConfig {
  const r: Partial<HotCornersConfig> = raw && typeof raw === 'object' ? raw : {};
  const d = DEFAULT_HOT_CORNERS;
  const corners = {} as Record<HotCornerId, HotCornerSlot>;
  for (const id of HOT_CORNER_IDS) {
    const slot = (r.corners as any)?.[id];
    const action = slot && ACTION_IDS.has(slot.action) ? (slot.action as HotCornerAction) : d.corners[id].action;
    corners[id] = { action, appId: typeof slot?.appId === 'string' ? slot.appId : undefined };
  }
  return {
    enabled: typeof r.enabled === 'boolean' ? r.enabled : d.enabled,
    triggerSize: clamp(r.triggerSize, 2, 40, d.triggerSize),
    dwellMs: clamp(r.dwellMs, 0, 1500, d.dwellMs),
    cooldownMs: clamp(r.cooldownMs, 200, 5000, d.cooldownMs),
    corners,
  };
}

/** Który róg obejmuje punkt (x, y) w oknie o wymiarach w×h — albo null. */
export function cornerAt(x: number, y: number, w: number, h: number, size: number): HotCornerId | null {
  const left = x <= size, right = x >= w - size - 1;
  const top = y <= size, bottom = y >= h - size - 1;
  if (top && left) return 'topLeft';
  if (top && right) return 'topRight';
  if (bottom && left) return 'bottomLeft';
  if (bottom && right) return 'bottomRight';
  return null;
}
