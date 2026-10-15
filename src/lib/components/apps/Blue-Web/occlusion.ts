export interface Rect { x: number; y: number; width: number; height: number }

/** The stacking-relevant part of a shell window. */
export interface WindowLayer {
  id: string;
  zIndex: number;
  isPiP?: boolean;
  isMinimized: boolean;
}

/** Windows' own 1px borders and soft shadows must not count as covering. */
export const OVERLAP_TOLERANCE_PX = 2;

/** Picture-in-picture windows float above every normal window (Window.svelte: z-index 9998). */
const PIP_BASE = 1_000_000;
const layer = (w: WindowLayer) => (w.isPiP ? PIP_BASE : 0) + w.zIndex;

/** Do the rectangles share more than a hair's width in both directions? */
export function rectsOverlap(a: Rect, b: Rect, tolerance = OVERLAP_TOLERANCE_PX): boolean {
  const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
  return w > tolerance && h > tolerance;
}

/** Visible windows stacked above `self` (the ones that can cover it). */
export function windowsAbove(self: WindowLayer, all: WindowLayer[]): WindowLayer[] {
  const mine = layer(self);
  return all.filter((w) => w.id !== self.id && !w.isMinimized && layer(w) > mine);
}

export function isCovered(target: Rect, covers: Rect[]): boolean {
  return covers.some((c) => rectsOverlap(target, c));
}

// ── Partial coverage: keep the page live, cut it to what is visible ─────────

/** Below this (px) a visible remainder is not worth keeping live — the page is hidden instead. */
export const MIN_VISIBLE_PX = 48;

export type Coverage =
  | { kind: 'none' }
  | { kind: 'full' }
  | { kind: 'partial'; clip: Rect };

const inset = (r: Rect, by: number): Rect => ({ x: r.x + by, y: r.y + by, width: r.width - 2 * by, height: r.height - 2 * by });

/**
 * How much of `page` do `covers` hide, and — when only part of it — which single
 * rectangle of it stays visible (the largest uncovered one).
 *
 *  - `none`    nothing overlaps: leave the page alone;
 *  - `partial` the page is cut to `clip` and keeps running underneath what covers it;
 *  - `full`    (almost) nothing is left to see: hiding it costs nothing the person can notice.
 *
 * Covers are shrunk by the overlap tolerance first, so a window's 1px border or soft
 * shadow never counts as covering.
 */
export function classifyCoverage(page: Rect, covers: Rect[], minVisible = MIN_VISIBLE_PX): Coverage {
  const cs = covers
    .map((c) => inset(c, OVERLAP_TOLERANCE_PX / 2))
    .filter((c) => c.width > 0 && c.height > 0 && rectsOverlap(page, c, 0));
  if (cs.length === 0) return { kind: 'none' };

  // Coordinate compression: every edge of a cover (clamped to the page) splits the page.
  const px1 = page.x + page.width;
  const py1 = page.y + page.height;
  const clampX = (v: number) => Math.min(px1, Math.max(page.x, v));
  const clampY = (v: number) => Math.min(py1, Math.max(page.y, v));
  const xs = [...new Set([page.x, px1, ...cs.flatMap((c) => [clampX(c.x), clampX(c.x + c.width)])])].sort((a, b) => a - b);
  const ys = [...new Set([page.y, py1, ...cs.flatMap((c) => [clampY(c.y), clampY(c.y + c.height)])])].sort((a, b) => a - b);

  // free[j][i]: is the cell between xs[i..i+1] and ys[j..j+1] uncovered?
  const free: boolean[][] = [];
  for (let j = 0; j < ys.length - 1; j++) {
    const row: boolean[] = [];
    for (let i = 0; i < xs.length - 1; i++) {
      const cx = (xs[i] + xs[i + 1]) / 2;
      const cy = (ys[j] + ys[j + 1]) / 2;
      row.push(!cs.some((c) => cx > c.x && cx < c.x + c.width && cy > c.y && cy < c.y + c.height));
    }
    free.push(row);
  }

  // Largest all-free block of cells (grids here are tiny: a handful of covers).
  let best: Rect | null = null;
  let bestArea = 0;
  for (let j0 = 0; j0 < free.length; j0++) {
    for (let i0 = 0; i0 < free[j0].length; i0++) {
      for (let j1 = j0; j1 < free.length; j1++) {
        for (let i1 = i0; i1 < free[j1].length; i1++) {
          let ok = true;
          for (let j = j0; j <= j1 && ok; j++) for (let i = i0; i <= i1; i++) if (!free[j][i]) { ok = false; break; }
          if (!ok) continue;
          const r = { x: xs[i0], y: ys[j0], width: xs[i1 + 1] - xs[i0], height: ys[j1 + 1] - ys[j0] };
          const area = r.width * r.height;
          if (area > bestArea) { best = r; bestArea = area; }
        }
      }
    }
  }

  if (!best || best.width < minVisible || best.height < minVisible) return { kind: 'full' };
  return { kind: 'partial', clip: best };
}

export const sameRect = (a: Rect | null, b: Rect | null): boolean =>
  a === b || (!!a && !!b && a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height);
