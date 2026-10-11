export type Dir = 'up' | 'down' | 'left' | 'right';

export interface Box { x: number; y: number; w: number; h: number }

const cx = (b: Box) => b.x + b.w / 2;
const cy = (b: Box) => b.y + b.h / 2;

/**
 * Index of the best candidate in direction `dir` from `from`, or `-1`.
 * `from` itself should not be in `candidates` (it is ignored if it is).
 */
export function pickNext(from: Box, candidates: Box[], dir: Dir): number {
  let best = -1;
  let bestScore = Infinity;
  candidates.forEach((c, i) => {
    if (c === from) return;
    // Signed distance along the travel axis between centres, and the
    // misalignment across it.
    let along: number; let across: number; let overlap: boolean;
    switch (dir) {
      case 'right': along = cx(c) - cx(from); across = Math.abs(cy(c) - cy(from));
        overlap = c.y < from.y + from.h && c.y + c.h > from.y; break;
      case 'left': along = cx(from) - cx(c); across = Math.abs(cy(c) - cy(from));
        overlap = c.y < from.y + from.h && c.y + c.h > from.y; break;
      case 'down': along = cy(c) - cy(from); across = Math.abs(cx(c) - cx(from));
        overlap = c.x < from.x + from.w && c.x + c.w > from.x; break;
      default: along = cy(from) - cy(c); across = Math.abs(cx(c) - cx(from));
        overlap = c.x < from.x + from.w && c.x + c.w > from.x; break;
    }
    if (along <= 0.5) return; // not in that direction
    // Misaligned candidates are penalised twice as hard as they are far;
    // candidates that don't overlap the row/column at all pay a flat penalty
    // so a tile in the same row always beats one diagonally closer.
    const score = along + across * 2 + (overlap ? 0 : 1000);
    if (score < bestScore) { bestScore = score; best = i; }
  });
  return best;
}

export const FOCUSABLE_SELECTOR =
  '[data-tv-focusable], button:not([disabled]), a[href], input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function isVisible(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  if (r.width < 1 || r.height < 1) return false;
  const style = getComputedStyle(el);
  return style.visibility !== 'hidden' && style.display !== 'none';
}

/**
 * Moves focus within `root` in direction `dir`. If nothing inside `root` has
 * focus yet, the first visible focusable element gets it. Returns the newly
 * focused element or `null` when there is no neighbour in that direction.
 */
export function moveFocus(root: HTMLElement, dir: Dir): HTMLElement | null {
  const all = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(isVisible);
  if (!all.length) return null;
  const active = document.activeElement as HTMLElement | null;
  const current = active && root.contains(active) ? all.find((el) => el === active || el.contains(active)) : undefined;
  if (!current) { all[0].focus(); return all[0]; }
  const rect = (el: HTMLElement): Box => { const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; };
  const others = all.filter((el) => el !== current);
  const idx = pickNext(rect(current), others.map(rect), dir);
  if (idx < 0) return null;
  const next = others[idx];
  next.focus();
  next.scrollIntoView?.({ block: 'nearest', inline: 'nearest', behavior: 'smooth' });
  return next;
}
