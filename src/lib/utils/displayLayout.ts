export interface Rect { name: string; x: number; y: number; w: number; h: number }

export interface OutputGeometry { width: number; height: number; scale: number; transform: string }

/** Logical (post-scale, post-rotation) size in compositor pixels — what the layout is laid out in. */
export function logicalSize(o: OutputGeometry): { w: number; h: number } {
  const rotated = /(^|-)(90|270)$/.test(o.transform);
  const [pw, ph] = rotated ? [o.height, o.width] : [o.width, o.height];
  const s = o.scale > 0 ? o.scale : 1;
  return { w: Math.round(pw / s), h: Math.round(ph / s) };
}

export function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

export const anyOverlap = (rs: Rect[]): boolean => rs.some((a, i) => rs.slice(i + 1).some((b) => overlaps(a, b)));

/** Edges touch (share a border segment) without overlapping. */
export function touches(a: Rect, b: Rect): boolean {
  if (overlaps(a, b)) return false;
  const vOverlap = a.y < b.y + b.h && b.y < a.y + a.h;
  const hOverlap = a.x < b.x + b.w && b.x < a.x + a.w;
  return (vOverlap && (a.x + a.w === b.x || b.x + b.w === a.x)) || (hOverlap && (a.y + a.h === b.y || b.y + b.h === a.y));
}

/** Every monitor is reachable from the first one through touching edges. */
export function isConnected(rs: Rect[]): boolean {
  if (rs.length <= 1) return true;
  const seen = new Set([0]);
  const queue = [0];
  while (queue.length) {
    const i = queue.shift()!;
    rs.forEach((r, j) => { if (!seen.has(j) && touches(rs[i], r)) { seen.add(j); queue.push(j); } });
  }
  return seen.size === rs.length;
}

/**
 * Snaps `moving` so its edges align with (or butt up against) the other
 * monitors' edges when within `threshold` px; returns the new top-left.
 */
export function snapMove(moving: Rect, others: Rect[], threshold: number): { x: number; y: number } {
  let { x, y } = moving;
  let bestDx = threshold + 1;
  let bestDy = threshold + 1;
  for (const o of others) {
    const xs = [
      o.x - moving.w,          // left of o, touching
      o.x + o.w,               // right of o, touching
      o.x,                     // left edges aligned
      o.x + o.w - moving.w,    // right edges aligned
    ];
    const ys = [o.y - moving.h, o.y + o.h, o.y, o.y + o.h - moving.h];
    for (const cx of xs) if (Math.abs(cx - moving.x) < bestDx) { bestDx = Math.abs(cx - moving.x); x = cx; }
    for (const cy of ys) if (Math.abs(cy - moving.y) < bestDy) { bestDy = Math.abs(cy - moving.y); y = cy; }
  }
  if (bestDx > threshold) x = moving.x;
  if (bestDy > threshold) y = moving.y;
  return { x: Math.round(x), y: Math.round(y) };
}

/** Shifts the layout so the top-left-most monitor is at 0,0 (compositors expect non-negative origins). */
export function normalize(rs: Rect[]): Rect[] {
  if (!rs.length) return rs;
  const minX = Math.min(...rs.map((r) => r.x));
  const minY = Math.min(...rs.map((r) => r.y));
  return rs.map((r) => ({ ...r, x: r.x - minX, y: r.y - minY }));
}

/** Left-to-right row, no gaps, keeping the given order. */
export function arrangeRow(rs: Rect[]): Rect[] {
  let x = 0;
  return rs.map((r) => { const out = { ...r, x, y: 0 }; x += r.w; return out; });
}

/** Moves `moving` to the nearest free position that touches another monitor if it currently overlaps. */
export function resolveOverlap(moving: Rect, others: Rect[]): Rect {
  if (!others.some((o) => overlaps(moving, o))) return moving;
  let best: { x: number; y: number } | null = null;
  let bestDist = Infinity;
  for (const o of others) {
    const candidates = [
      { x: o.x + o.w, y: o.y }, { x: o.x - moving.w, y: o.y },
      { x: o.x, y: o.y + o.h }, { x: o.x, y: o.y - moving.h },
    ];
    for (const c of candidates) {
      const test = { ...moving, ...c };
      if (others.some((q) => overlaps(test, q))) continue;
      const d = Math.hypot(c.x - moving.x, c.y - moving.y);
      if (d < bestDist) { bestDist = d; best = c; }
    }
  }
  return best ? { ...moving, ...best } : moving;
}
