import type {
  Presentation, Rect, Slide, SlideElement, SlideImageBox, SlideLayoutId, SlideTextBox,
  SlideTextStyle, SlideTransition, TransitionType,
} from './types';

/** Logical canvas the editor and the exporter agree on (points; 16:9). */
export const SLIDE_W = 960;
export const SLIDE_H = 540;

export const FONT_FAMILIES = [
  'Calibri', 'Arial', 'Georgia', 'Times New Roman', 'Verdana', 'Trebuchet MS', 'Courier New', 'Impact',
] as const;

/** CSS font stack for a family name stored in the model. */
export function cssFontStack(family: string | undefined): string {
  const f = family || 'Calibri';
  const generic = /courier|mono/i.test(f) ? 'monospace' : /georgia|times|serif/i.test(f) && !/sans/i.test(f) ? 'serif' : 'sans-serif';
  return `"${f.replace(/"/g, '')}", ${generic}`;
}

// ── colours ─────────────────────────────────────────────────────────────

/** `#rrggbb` → bare `RRGGBB` for OOXML. */
export function hexColor(cssColor: string): string {
  return cssColor.replace('#', '').toUpperCase().padEnd(6, '0').slice(0, 6);
}

/** Readable text colour (bare hex) for a slide background. */
export function textColorFor(background: string): string {
  const hex = hexColor(background);
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum > 0.55 ? '0F172A' : 'F8FAFC';
}

export function isHexColor(v: unknown): v is string {
  return typeof v === 'string' && /^#[0-9a-fA-F]{6}$/.test(v);
}

// ── layouts ─────────────────────────────────────────────────────────────

export type PlaceholderKey = 'title' | 'body' | 'body2';

export interface LayoutDef {
  id: SlideLayoutId;
  label: string;
  title?: Rect;
  body?: Rect;
  body2?: Rect;
  titleStyle: SlideTextStyle;
  bodyStyle: SlideTextStyle;
}

const TITLE_ONLY_STYLE: SlideTextStyle = { fontSize: 36, bold: true, align: 'left' };
const BODY_STYLE: SlideTextStyle = { fontSize: 24, bullets: true, align: 'left' };

export const LAYOUTS: Record<SlideLayoutId, LayoutDef> = {
  'title-content': {
    id: 'title-content', label: 'Title and content',
    title: { x: 6, y: 5, w: 88, h: 17 }, body: { x: 6, y: 24, w: 88, h: 66 },
    titleStyle: TITLE_ONLY_STYLE, bodyStyle: BODY_STYLE,
  },
  title: {
    id: 'title', label: 'Title slide',
    title: { x: 10, y: 28, w: 80, h: 24 }, body: { x: 15, y: 55, w: 70, h: 20 },
    titleStyle: { fontSize: 54, bold: true, align: 'center' }, bodyStyle: { fontSize: 26, bullets: false, align: 'center' },
  },
  section: {
    id: 'section', label: 'Section header',
    title: { x: 8, y: 36, w: 84, h: 24 }, body: { x: 8, y: 62, w: 84, h: 14 },
    titleStyle: { fontSize: 46, bold: true, align: 'left' }, bodyStyle: { fontSize: 24, bullets: false, align: 'left' },
  },
  'two-content': {
    id: 'two-content', label: 'Two columns',
    title: { x: 6, y: 5, w: 88, h: 17 }, body: { x: 6, y: 24, w: 42, h: 66 }, body2: { x: 52, y: 24, w: 42, h: 66 },
    titleStyle: TITLE_ONLY_STYLE, bodyStyle: { ...BODY_STYLE, fontSize: 22 },
  },
  'content-image': {
    id: 'content-image', label: 'Content and picture',
    title: { x: 6, y: 5, w: 88, h: 17 }, body: { x: 6, y: 24, w: 46, h: 66 },
    titleStyle: TITLE_ONLY_STYLE, bodyStyle: { ...BODY_STYLE, fontSize: 22 },
  },
  'title-only': {
    id: 'title-only', label: 'Title only',
    title: { x: 6, y: 5, w: 88, h: 17 },
    titleStyle: TITLE_ONLY_STYLE, bodyStyle: BODY_STYLE,
  },
  blank: { id: 'blank', label: 'Blank', titleStyle: TITLE_ONLY_STYLE, bodyStyle: BODY_STYLE },
};

export const LAYOUT_IDS = Object.keys(LAYOUTS) as SlideLayoutId[];

/** Where a picture goes when it is added to a slide with this layout. */
export function pictureSlot(layout: SlideLayoutId): Rect {
  if (layout === 'content-image') return { x: 56, y: 24, w: 38, h: 66 };
  if (layout === 'blank' || layout === 'title-only') return { x: 25, y: 28, w: 50, h: 60 };
  return { x: 60, y: 54, w: 32, h: 36 };
}

export function layoutOf(slide: Pick<Slide, 'layout'>): LayoutDef {
  return LAYOUTS[slide.layout ?? 'title-content'] ?? LAYOUTS['title-content'];
}

/** Placeholder / table position after per-slide overrides. `null` = the slide has no such block. */
export function resolveRect(slide: Slide, key: PlaceholderKey | 'table'): Rect | null {
  const override = slide.rects?.[key];
  if (override) return clampRect(override);
  if (key === 'table') {
    if (!slide.table?.length) return null;
    const body = layoutOf(slide).body;
    const bodyHasText = slide.bullets.some((b) => b.trim());
    // below the body text when there is some, otherwise where the body would be
    return bodyHasText ? { x: 6, y: 58, w: 88, h: Math.min(34, Math.max(10, slide.table.length * 7)) } : { ...(body ?? { x: 6, y: 24, w: 88, h: 66 }), h: Math.min(60, Math.max(10, slide.table.length * 8)) };
  }
  return layoutOf(slide)[key] ?? null;
}

/** Fully resolved text style of a placeholder (every field set). */
export function resolveStyle(slide: Slide, key: PlaceholderKey): Required<Omit<SlideTextStyle, 'bullets'>> & { bullets: boolean } {
  const layout = layoutOf(slide);
  const base = key === 'title' ? layout.titleStyle : layout.bodyStyle;
  const own = key === 'title' ? slide.titleStyle : key === 'body' ? slide.bodyStyle : slide.body2Style ?? slide.bodyStyle;
  return fillStyle({ ...base, ...stripUndefined(own) }, slide.background);
}

/** Resolved style of a free text box. */
export function resolveBoxStyle(slide: Slide, box: SlideTextBox): ReturnType<typeof resolveStyle> {
  return fillStyle({ fontSize: 22, bullets: false, align: 'left', ...stripUndefined(box.style) }, slide.background);
}

function fillStyle(s: SlideTextStyle, background: string) {
  return {
    fontFamily: s.fontFamily ?? 'Calibri',
    fontSize: s.fontSize ?? 24,
    color: s.color ?? `#${textColorFor(background)}`,
    align: s.align ?? 'left',
    bold: s.bold ?? false,
    italic: s.italic ?? false,
    underline: s.underline ?? false,
    bullets: s.bullets ?? false,
  } as Required<Omit<SlideTextStyle, 'bullets'>> & { bullets: boolean };
}

function stripUndefined<T extends object>(o: T | undefined): Partial<T> {
  if (!o) return {};
  return Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as Partial<T>;
}

// ── geometry ────────────────────────────────────────────────────────────

const MIN_SIZE = 3;

/** Keeps a rectangle on the slide and at least a few percent large. */
export function clampRect(r: Rect): Rect {
  const w = Math.min(100, Math.max(MIN_SIZE, Number.isFinite(r.w) ? r.w : 30));
  const h = Math.min(100, Math.max(MIN_SIZE, Number.isFinite(r.h) ? r.h : 20));
  const x = Math.min(100 - w, Math.max(0, Number.isFinite(r.x) ? r.x : 0));
  const y = Math.min(100 - h, Math.max(0, Number.isFinite(r.y) ? r.y : 0));
  return { x, y, w, h };
}

export function rectsDiffer(a: Rect, b: Rect, tolerance = 1): boolean {
  return (['x', 'y', 'w', 'h'] as const).some((k) => Math.abs(a[k] - b[k]) > tolerance);
}

export function round1(n: number): number {
  return Math.round(n * 10) / 10;
}

// ── ids / factories ─────────────────────────────────────────────────────

let idCounter = 0;
export function newId(prefix: string): string {
  idCounter = (idCounter + 1) % 1_000_000;
  return `${prefix}-${Date.now().toString(36)}-${idCounter.toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
}

export const SLIDE_BACKGROUNDS = ['#0f172a', '#1e1b4b', '#052e2b', '#3f1d1d', '#1c1917', '#0c4a6e', '#ffffff', '#f1f5f9', '#fef3c7'];

export function newSlide(layout: SlideLayoutId = 'title-content', index = 0): Slide {
  return {
    id: newId('slide'),
    title: layout === 'blank' ? '' : `Slide ${index + 1}`,
    bullets: [],
    background: SLIDE_BACKGROUNDS[index % 6],
    images: [],
    layout,
    elements: [],
  };
}

export function newTextBox(slide: Slide): SlideTextBox {
  const n = (slide.elements ?? []).filter((e) => e.kind === 'text').length;
  const off = Math.min(20, n * 4);
  return {
    kind: 'text',
    id: newId('text'),
    ...clampRect({ x: 20 + off, y: 40 + off, w: 40, h: 14 }),
    text: 'Text',
    style: { fontSize: 24, align: 'left', bullets: false },
  };
}

/** Picture box that keeps `aspect` (width / height of the picture) inside the layout's picture slot. */
export function newImageBox(slide: Slide, src: string, aspect = 4 / 3): SlideImageBox {
  const slot = pictureSlot(slide.layout ?? 'title-content');
  // slot is in percent of a 16:9 slide, so convert to a common unit before comparing aspect ratios
  const slotAspect = (slot.w * SLIDE_W) / (slot.h * SLIDE_H);
  let w = slot.w, h = slot.h;
  if (aspect > slotAspect) h = (slot.w * SLIDE_W) / aspect / SLIDE_H;
  else w = (slot.h * SLIDE_H * aspect) / SLIDE_W;
  const n = (slide.elements ?? []).filter((e) => e.kind === 'image').length;
  const off = Math.min(12, n * 3);
  return {
    kind: 'image',
    id: newId('img'),
    ...clampRect({ x: slot.x + (slot.w - w) / 2 + off, y: slot.y + (slot.h - h) / 2 + off, w, h }),
    src,
  };
}

// ── normalisation (also migrates decks saved by older builds) ───────────

const TRANSITION_TYPES: TransitionType[] = ['none', 'fade', 'push', 'wipe', 'zoom', 'split', 'cover'];

export function normalizeTransition(t: unknown): SlideTransition | undefined {
  if (!t || typeof t !== 'object') return undefined;
  const o = t as Partial<SlideTransition>;
  if (!o.type || !TRANSITION_TYPES.includes(o.type)) return undefined;
  const hasAdvance = typeof o.advanceAfterMs === 'number' && Number.isFinite(o.advanceAfterMs) && o.advanceAfterMs > 0;
  if (o.type === 'none' && !hasAdvance) return undefined; // nothing to remember
  const out: SlideTransition = { type: o.type };
  if (o.direction && ['l', 'r', 'u', 'd'].includes(o.direction)) out.direction = o.direction;
  if (typeof o.durationMs === 'number' && Number.isFinite(o.durationMs)) out.durationMs = Math.min(3000, Math.max(100, Math.round(o.durationMs)));
  if (typeof o.advanceAfterMs === 'number' && Number.isFinite(o.advanceAfterMs) && o.advanceAfterMs > 0) out.advanceAfterMs = Math.min(600_000, Math.round(o.advanceAfterMs));
  return out;
}

/**
 * Returns a slide with every optional field filled in and legacy pictures
 * (`images: string[]`, once drawn in a fixed row) converted into positioned
 * picture elements laid out in that same row. Idempotent.
 */
export function normalizeSlide(input: Slide): Slide {
  const slide: Slide = { ...input };
  slide.id = slide.id || newId('slide');
  slide.title = typeof slide.title === 'string' ? slide.title : '';
  slide.bullets = Array.isArray(slide.bullets) ? slide.bullets.filter((b) => typeof b === 'string') : [];
  slide.background = isHexColor(slide.background) ? slide.background : '#0f172a';
  slide.layout = slide.layout && LAYOUTS[slide.layout] ? slide.layout : 'title-content';
  slide.elements = Array.isArray(slide.elements)
    ? slide.elements
        .filter((e): e is SlideElement => !!e && (e.kind === 'text' || e.kind === 'image'))
        .map((e) => ({ ...e, id: e.id || newId(e.kind), ...clampRect(e) }))
    : [];

  const legacy = Array.isArray(slide.images) ? slide.images.filter((s) => typeof s === 'string' && s) : [];
  if (legacy.length) {
    const gap = 2;
    const w = Math.min(30, (80 - gap * (legacy.length - 1)) / legacy.length);
    const total = legacy.length * w + gap * (legacy.length - 1);
    const startX = (100 - total) / 2;
    const y = slide.table?.length ? 70 : 52;
    legacy.forEach((src, i) => {
      slide.elements!.push({ kind: 'image', id: newId('img'), ...clampRect({ x: startX + i * (w + gap), y, w, h: Math.min(40, 96 - y) }), src });
    });
  }
  slide.images = [];

  if (slide.bullets2 && !Array.isArray(slide.bullets2)) delete slide.bullets2;
  const tr = normalizeTransition(slide.transition);
  if (tr) slide.transition = tr; else delete slide.transition;
  return slide;
}

export function normalizePresentation(p: Presentation): Presentation {
  return { slides: p.slides.map(normalizeSlide) };
}

/** Text lines of a body placeholder that actually contain something. */
export function nonEmptyLines(lines: string[] | undefined): string[] {
  return (lines ?? []).filter((l) => l.trim().length > 0);
}

// ── transitions ─────────────────────────────────────────────────────────

export const TRANSITION_LABELS: Record<TransitionType, string> = {
  none: 'None', fade: 'Fade', push: 'Push', wipe: 'Wipe', zoom: 'Zoom', split: 'Split', cover: 'Cover',
};

export const TRANSITIONS_WITH_DIRECTION: TransitionType[] = ['push', 'wipe', 'cover'];

/** Web Animations keyframes that play when a slide with this transition appears. */
export function transitionKeyframes(t: SlideTransition | undefined): Keyframe[] | null {
  if (!t || t.type === 'none') return null;
  const dir = t.direction ?? 'l';
  // the new slide travels in `dir`, so it starts on the opposite side
  const from = { l: 'translateX(100%)', r: 'translateX(-100%)', u: 'translateY(100%)', d: 'translateY(-100%)' }[dir];
  const wipeFrom = { l: 'inset(0 0 0 100%)', r: 'inset(0 100% 0 0)', u: 'inset(100% 0 0 0)', d: 'inset(0 0 100% 0)' }[dir];
  switch (t.type) {
    case 'fade': return [{ opacity: 0 }, { opacity: 1 }];
    case 'push':
    case 'cover': return [{ transform: from }, { transform: 'none' }];
    case 'wipe': return [{ clipPath: wipeFrom }, { clipPath: 'inset(0 0 0 0)' }];
    case 'zoom': return [{ transform: 'scale(0.4)', opacity: 0 }, { transform: 'none', opacity: 1 }];
    case 'split': return [{ clipPath: 'inset(0 50% 0 50%)' }, { clipPath: 'inset(0 0 0 0)' }];
    default: return null;
  }
}

export function transitionDuration(t: SlideTransition | undefined): number {
  return Math.min(3000, Math.max(100, t?.durationMs ?? 600));
}
