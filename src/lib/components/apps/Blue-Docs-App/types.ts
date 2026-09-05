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
 * spreadsheet editor already uses for its own `content`. Deliberately
 * minimal (title + bullet lines per slide, optional images, one
 * background color) rather than trying to match every layout/media
 * capability a format like .pptx has — see PresentationEditor.svelte's
 * module doc for exactly what this does and doesn't cover, and
 * pptxFile.ts for the real .pptx import/export built on top of it. */
export interface Slide {
  id: string;
  title: string;
  /** One bullet per array entry — rendered as a plain bulleted list on
   * the slide. Supports a small inline markdown-lite subset for
   * formatting: `**bold**` and `*italic*` (see
   * PresentationEditor.svelte's `renderInline` helper) — not full rich
   * text (no per-character color, no links, no nested styles), but
   * real, visible formatting rather than plain text only. */
  bullets: string[];
  background: string;
  /** Data URLs (`data:image/...;base64,...`) for images placed on this
   * slide — kept as a flat list rather than positioned/sized objects;
   * every image renders at a fixed size in a simple flow layout below
   * the bullets, both in the editor and in exported .pptx files. See
   * pptxFile.ts's module doc for why this doesn't attempt arbitrary
   * image positioning. */
  images: string[];
}

export interface Presentation {
  slides: Slide[];
}

export function emptyPresentation(): Presentation {
  return { slides: [{ id: `slide-${Date.now()}`, title: 'Title Slide', bullets: [], background: '#0f172a', images: [] }] };
}
