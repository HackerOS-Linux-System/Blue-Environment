export type DocFormat = 'rich' | 'markdown' | 'spreadsheet' | 'presentation';

export interface DocFile {
  id: string; name: string; format: DocFormat; content: string;
  path?: string; modified: boolean; created: Date; updated: Date;
}

export interface TextStyle {
  bold?: boolean; italic?: boolean; underline?: boolean; strike?: boolean;
  color?: string; bgColor?: string; fontSize?: number; fontFamily?: string;
  align?: 'left' | 'center' | 'right' | 'justify';
  heading?: 0 | 1 | 2 | 3; list?: 'bullet' | 'ordered' | 'none'; link?: string;
}

/** A presentation doc's `content` field is this shape, JSON-stringified
 * — same "plain-string-field, structured JSON inside" convention the
 * spreadsheet editor already uses for its own `content`.
 *
 * Every field added after the first, minimal version (`layout`, `elements`,
 * `rects`, `titleStyle` …, `transition`) is optional, so decks saved by
 * older builds still open unchanged — `normalizeSlide` (slideModel.ts) fills
 * in the defaults. See PresentationEditor.svelte for the editor and
 * pptxFile.ts for the real .pptx import/export built on this model. */

/** Position and size in percent of the slide (0–100, 16:9 canvas). */
export interface Rect { x: number; y: number; w: number; h: number }

/** Slide layouts: which placeholders a slide has and where they sit. */
export type SlideLayoutId = 'title-content' | 'title' | 'section' | 'two-content' | 'content-image' | 'title-only' | 'blank';

/** Look of one text block. Anything left out falls back to the layout's default. */
export interface SlideTextStyle {
  fontFamily?: string;
  /** Points on a 960×540pt slide (the same scale PowerPoint uses). */
  fontSize?: number;
  /** `#rrggbb`. Default: readable against the slide background. */
  color?: string;
  align?: 'left' | 'center' | 'right' | 'justify';
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  /** Show a bullet in front of every line. */
  bullets?: boolean;
}

/** A free text box. Lines are separated by `\n`; `**bold**` / `*italic*` work inline. */
export interface SlideTextBox {
  kind: 'text';
  id: string;
  x: number; y: number; w: number; h: number;
  text: string;
  style: SlideTextStyle;
  /** Optional box fill (`#rrggbb`). */
  fill?: string;
}

/** A positioned, resizable picture. */
export interface SlideImageBox {
  kind: 'image';
  id: string;
  x: number; y: number; w: number; h: number;
  /** `data:image/...;base64,...` */
  src: string;
}

export type SlideElement = SlideTextBox | SlideImageBox;

export type TransitionType = 'none' | 'fade' | 'push' | 'wipe' | 'zoom' | 'split' | 'cover';

export interface SlideTransition {
  type: TransitionType;
  /** Direction the motion travels, OOXML letters: l / r / u / d. */
  direction?: 'l' | 'r' | 'u' | 'd';
  /** 100–3000 ms. */
  durationMs?: number;
  /** Move on by itself after this many ms (presentation mode); omitted = on click. */
  advanceAfterMs?: number;
}

export interface Slide {
  id: string;
  /** Title placeholder text. */
  title: string;
  /** Body placeholder: one line per array entry (bulleted unless the layout/style says otherwise).
   * Supports `**bold**` and `*italic*` inline. */
  bullets: string[];
  background: string;
  /** Legacy flat list of pictures (data URLs). `normalizeSlide` moves them into
   * `elements` as positioned pictures, so new code only needs `elements`. */
  images: string[];
  /** Speaker notes (plain text, one paragraph per line). Round-trips through .pptx notes slides. */
  notes?: string;
  /** One optional table: rows of cell texts (first row is the header). Round-trips as a native <a:tbl>. */
  table?: string[][];

  /** Defaults to `title-content`. */
  layout?: SlideLayoutId;
  /** Second column of the `two-content` layout. */
  bullets2?: string[];
  /** Text boxes and pictures placed freely on the slide (drawn in array order). */
  elements?: SlideElement[];
  /** Per-slide overrides of where the placeholders / the table sit. */
  rects?: { title?: Rect; body?: Rect; body2?: Rect; table?: Rect };
  titleStyle?: SlideTextStyle;
  bodyStyle?: SlideTextStyle;
  body2Style?: SlideTextStyle;
  transition?: SlideTransition;
}

export interface Presentation {
  slides: Slide[];
}

export function emptyPresentation(): Presentation {
  return { slides: [{ id: `slide-${Date.now()}`, title: 'Title Slide', bullets: [], background: '#0f172a', images: [], layout: 'title' }] };
}
