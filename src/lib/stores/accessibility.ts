import { writable, get } from 'svelte/store';

/**
 * Shell-level accessibility settings. Everything except the screen reader is
 * implemented here, in the webview (CSS classes + a root font size + a pointer
 * magnifier); the screen reader itself (Orca) is started through the
 * `a11y_screen_reader_set` backend command from Settings → Accessibility.
 */
export interface A11ySettings {
  highContrast: boolean;
  /** Multiplier of the root font size: 1 = normal. */
  textScale: number;
  reduceMotion: boolean;
  magnifierEnabled: boolean;
  /** Magnification factor 1.5 … 8. */
  magnifierZoom: number;
  /** Always show a strong focus outline on the focused control. */
  strongFocus: boolean;
  /** Speak shell announcements (toasts, workspace changes) through speech-dispatcher when Orca is off. */
  speakAnnouncements: boolean;
}

export const DEFAULT_A11Y: A11ySettings = {
  highContrast: false,
  textScale: 1,
  reduceMotion: false,
  magnifierEnabled: false,
  magnifierZoom: 2,
  strongFocus: false,
  speakAnnouncements: false,
};

export const TEXT_SCALE_MIN = 0.85;
export const TEXT_SCALE_MAX = 2;
export const MAGNIFIER_MIN = 1.5;
export const MAGNIFIER_MAX = 8;

const STORAGE_KEY = 'blue.a11y.v1';

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));
const num = (v: unknown, d: number) => (typeof v === 'number' && Number.isFinite(v) ? v : d);
const bool = (v: unknown, d: boolean) => (typeof v === 'boolean' ? v : d);

/** Accepts anything (corrupt localStorage, older versions) and returns valid settings. */
export function normalizeA11y(raw: unknown): A11ySettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  return {
    highContrast: bool(r.highContrast, DEFAULT_A11Y.highContrast),
    textScale: clamp(num(r.textScale, 1), TEXT_SCALE_MIN, TEXT_SCALE_MAX),
    reduceMotion: bool(r.reduceMotion, DEFAULT_A11Y.reduceMotion),
    magnifierEnabled: bool(r.magnifierEnabled, DEFAULT_A11Y.magnifierEnabled),
    magnifierZoom: clamp(num(r.magnifierZoom, 2), MAGNIFIER_MIN, MAGNIFIER_MAX),
    strongFocus: bool(r.strongFocus, DEFAULT_A11Y.strongFocus),
    speakAnnouncements: bool(r.speakAnnouncements, DEFAULT_A11Y.speakAnnouncements),
  };
}

/** Classes toggled on <html>; the matching rules live in src/app.css. */
export function rootClasses(s: A11ySettings): string[] {
  const c: string[] = [];
  if (s.highContrast) c.push('a11y-high-contrast');
  if (s.reduceMotion) c.push('a11y-reduce-motion');
  if (s.strongFocus) c.push('a11y-strong-focus');
  return c;
}
export const ALL_ROOT_CLASSES = ['a11y-high-contrast', 'a11y-reduce-motion', 'a11y-strong-focus'] as const;

export const rootFontSizePercent = (s: A11ySettings): number => Math.round(s.textScale * 100);

/**
 * CSS transform that magnifies the page around the pointer: the point under
 * the pointer stays where it is while everything else scales away from it.
 */
export function magnifierTransform(zoom: number, px: number, py: number): { transform: string; origin: string } {
  return { transform: `scale(${zoom})`, origin: `${Math.round(px)}px ${Math.round(py)}px` };
}

/** Clamp the pointer so the zoomed view never shows beyond the screen edge. */
export function clampMagnifierOrigin(px: number, py: number, vw: number, vh: number): { x: number; y: number } {
  return { x: clamp(px, 0, vw), y: clamp(py, 0, vh) };
}

function load(): A11ySettings {
  try {
    const s = typeof localStorage !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null;
    return normalizeA11y(s ? JSON.parse(s) : null);
  } catch {
    return { ...DEFAULT_A11Y };
  }
}

function persist(s: A11ySettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch { /* private mode / quota: settings just won't survive a restart */ }
}

export const a11y = writable<A11ySettings>(load());

export function updateA11y(patch: Partial<A11ySettings>) {
  a11y.update((cur) => {
    const next = normalizeA11y({ ...cur, ...patch });
    persist(next);
    return next;
  });
}

export function resetA11y() {
  persist(DEFAULT_A11Y);
  a11y.set({ ...DEFAULT_A11Y });
}

/** Applies the settings to the document. Safe to call repeatedly. */
export function applyA11y(s: A11ySettings, doc: Document | undefined = typeof document !== 'undefined' ? document : undefined) {
  if (!doc) return;
  const root = doc.documentElement;
  for (const c of ALL_ROOT_CLASSES) root.classList.remove(c);
  for (const c of rootClasses(s)) root.classList.add(c);
  root.style.fontSize = s.textScale === 1 ? '' : `${rootFontSizePercent(s)}%`;
}

// ── Live-region announcer ───────────────────────────────────────────────

let region: HTMLElement | null = null;

/** Politely announces `text` to assistive technology (Orca reads aria-live regions). */
export function announce(text: string, doc: Document | undefined = typeof document !== 'undefined' ? document : undefined) {
  if (!doc || !text) return;
  if (!region || !doc.body.contains(region)) {
    region = doc.createElement('div');
    region.setAttribute('role', 'status');
    region.setAttribute('aria-live', 'polite');
    region.setAttribute('aria-atomic', 'true');
    region.style.cssText = 'position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0';
    doc.body.appendChild(region);
  }
  region.textContent = '';
  // A change is needed for some readers to re-announce identical text.
  setTimeout(() => { if (region) region.textContent = text; }, 30);
}

export const getA11y = () => get(a11y);
