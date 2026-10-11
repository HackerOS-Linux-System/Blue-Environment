export type SwipeDir = 'left' | 'right' | 'up' | 'down';

export interface SwipeOptions {
  /** Minimum travel in px to count as a swipe. */
  minDistance?: number;
  /** The dominant axis must be at least this many times the other. */
  axisRatio?: number;
}

/** Direction of a drag from (0,0) to (dx,dy), or `null` if it is too short or too diagonal. */
export function classifySwipe(dx: number, dy: number, opts: SwipeOptions = {}): SwipeDir | null {
  const min = opts.minDistance ?? 48;
  const ratio = opts.axisRatio ?? 1.5;
  const ax = Math.abs(dx); const ay = Math.abs(dy);
  if (Math.max(ax, ay) < min) return null;
  if (ax >= ay * ratio) return dx > 0 ? 'right' : 'left';
  if (ay >= ax * ratio) return dy > 0 ? 'down' : 'up';
  return null;
}

/**
 * Page index after a horizontal drag on a pager. A drag past `threshold` of
 * the page width — or a quick flick — changes the page; the result is clamped.
 */
export function pageAfterDrag(
  page: number, pageCount: number, dx: number, pageWidth: number, durationMs: number,
  threshold = 0.25,
): number {
  if (pageCount <= 0) return 0;
  const flick = durationMs > 0 && durationMs < 250 && Math.abs(dx) > 40;
  const far = pageWidth > 0 && Math.abs(dx) > pageWidth * threshold;
  let next = page;
  if (flick || far) next = dx < 0 ? page + 1 : page - 1;
  return Math.min(pageCount - 1, Math.max(0, next));
}

/** Which zone of the screen's top edge a pull-down started in, so the shade can split notifications / quick settings. */
export function shadeTab(startX: number, screenWidth: number): 'notifications' | 'quick' {
  return screenWidth > 0 && startX > screenWidth / 2 ? 'quick' : 'notifications';
}
