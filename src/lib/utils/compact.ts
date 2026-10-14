export const COMPACT_MAX_WIDTH = 640;

/** `width` is the app's measured width in px; `0` means "not measured yet" (→ not compact). */
export function isCompactWidth(width: number, max = COMPACT_MAX_WIDTH): boolean {
  return width > 0 && width < max;
}
