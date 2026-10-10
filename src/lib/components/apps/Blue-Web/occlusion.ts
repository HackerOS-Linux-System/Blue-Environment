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
