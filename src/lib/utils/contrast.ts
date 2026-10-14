import { parseColor, type Rgba } from './colorUtil';
import type { ShellThemeColors } from '../data/builtinThemes';

/**
 * WCAG 2.x contrast checks for the theme editor. A theme can look fine to its
 * author and still leave text unreadable on someone else's screen; this turns
 * "is it readable?" into the standard numbers (4.5:1 for body text, 3:1 for
 * large text and UI components).
 */

const lin = (c: number) => {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
};

/** WCAG relative luminance (0 = black, 1 = white). */
export function relativeLuminance(c: Rgba): number {
  return 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
}

/** Composites a possibly translucent colour over an opaque backdrop. */
function over(fg: Rgba, bg: Rgba): Rgba {
  const a = fg.a;
  return { r: fg.r * a + bg.r * (1 - a), g: fg.g * a + bg.g * (1 - a), b: fg.b * a + bg.b * (1 - a), a: 1 };
}

/**
 * Contrast ratio between two CSS colours, 1..21, or `null` if either can't be
 * parsed. A translucent `fg` is composited over `bg`; a translucent `bg` over black.
 */
export function contrastRatio(fg: string, bg: string): number | null {
  const f = parseColor(fg); const b0 = parseColor(bg);
  if (!f || !b0) return null;
  const b = over(b0, { r: 0, g: 0, b: 0, a: 1 });
  const l1 = relativeLuminance(over(f, b)); const l2 = relativeLuminance(b);
  const hi = Math.max(l1, l2); const lo = Math.min(l1, l2);
  return (hi + 0.05) / (lo + 0.05);
}

export type ContrastPair = 'text_background' | 'text_surface' | 'muted_background' | 'accent_background';

export interface ContrastIssue {
  pair: ContrastPair;
  /** Measured ratio, rounded to 1 decimal. */
  ratio: number;
  /** Minimum the pair should reach. */
  min: number;
}

const CHECKS: { pair: ContrastPair; fg: keyof ShellThemeColors; bg: keyof ShellThemeColors; min: number }[] = [
  { pair: 'text_background', fg: 'text', bg: 'background', min: 4.5 },
  { pair: 'text_surface', fg: 'text', bg: 'surface', min: 4.5 },
  { pair: 'muted_background', fg: 'textMuted', bg: 'background', min: 3 },
  { pair: 'accent_background', fg: 'accent', bg: 'background', min: 3 },
];

/** The pairs of a palette that fall short of their WCAG minimum (empty = all good). Unparsable colours are skipped. */
export function themeContrastIssues(colors: ShellThemeColors): ContrastIssue[] {
  const issues: ContrastIssue[] = [];
  for (const c of CHECKS) {
    const ratio = contrastRatio(colors[c.fg], colors[c.bg]);
    if (ratio !== null && ratio < c.min) issues.push({ pair: c.pair, ratio: Math.round(ratio * 10) / 10, min: c.min });
  }
  return issues;
}
