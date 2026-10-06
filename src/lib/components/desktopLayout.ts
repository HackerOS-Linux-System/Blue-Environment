export interface Cell { col: number; row: number }
export type Layout = Record<string, Cell>;

export const CELL_W = 96;
export const CELL_H = 96;
export const ORIGIN_X = 8;
/** 3rem (top panel) + 8px margin, measured from the top of the full-screen desktop container. */
export const ORIGIN_Y = 56;
/** Space kept free at the bottom for a bottom-docked panel. */
export const BOTTOM_RESERVE = 48;

export const key = (c: Cell) => `${c.col}:${c.row}`;

export function gridSize(width: number, height: number): { cols: number; rows: number } {
  const cols = Math.max(1, Math.floor((width - ORIGIN_X) / CELL_W));
  const rows = Math.max(1, Math.floor((height - ORIGIN_Y - BOTTOM_RESERVE) / CELL_H));
  return { cols, rows };
}

/** Cell under a point given in container coordinates (clamped to the grid). */
export function cellAt(x: number, y: number, cols: number, rows: number): Cell {
  return {
    col: Math.min(cols - 1, Math.max(0, Math.floor((x - ORIGIN_X) / CELL_W))),
    row: Math.min(rows - 1, Math.max(0, Math.floor((y - ORIGIN_Y) / CELL_H))),
  };
}

export function cellOrigin(c: Cell): { left: number; top: number } {
  return { left: ORIGIN_X + c.col * CELL_W, top: ORIGIN_Y + c.row * CELL_H };
}

const inBounds = (c: Cell, cols: number, rows: number) => c.col >= 0 && c.row >= 0 && c.col < cols && c.row < rows;

/** First free cell in column-major order (top→bottom, then next column). Grows past `cols` if the grid is full. */
function firstFree(taken: Set<string>, cols: number, rows: number): Cell {
  for (let col = 0; ; col++) {
    for (let row = 0; row < rows; row++) {
      const c = { col, row };
      if (!taken.has(key(c))) return c;
    }
    if (col > cols + 1000) return { col, row: 0 }; // unreachable safety net
  }
}

/**
 * Effective positions for `names`: remembered cells are kept when still valid
 * (in bounds, not shared); everything else is flowed into free cells in
 * alphabetical order. Pure — never mutates `saved`.
 */
export function placeAll(names: string[], saved: Layout, cols: number, rows: number): Layout {
  const out: Layout = {};
  const taken = new Set<string>();
  const sorted = [...names].sort((a, b) => a.localeCompare(b));
  for (const n of sorted) {
    const s = saved[n];
    if (s && inBounds(s, cols, rows) && !taken.has(key(s))) { out[n] = { col: s.col, row: s.row }; taken.add(key(s)); }
  }
  for (const n of sorted) {
    if (out[n]) continue;
    const c = firstFree(taken, cols, rows);
    out[n] = c; taken.add(key(c));
  }
  return out;
}

/** Nearest free in-bounds cell to `target` (ring search; ties broken top-left first). */
export function nearestFree(target: Cell, taken: Set<string>, cols: number, rows: number): Cell {
  const t = { col: Math.min(cols - 1, Math.max(0, target.col)), row: Math.min(rows - 1, Math.max(0, target.row)) };
  if (!taken.has(key(t))) return t;
  const maxR = cols + rows;
  for (let r = 1; r <= maxR; r++) {
    let best: Cell | null = null;
    let bestD = Infinity;
    for (let dc = -r; dc <= r; dc++) {
      for (let dr = -r; dr <= r; dr++) {
        if (Math.max(Math.abs(dc), Math.abs(dr)) !== r) continue;
        const c = { col: t.col + dc, row: t.row + dr };
        if (!inBounds(c, cols, rows) || taken.has(key(c))) continue;
        const d = Math.abs(dc) + Math.abs(dr);
        if (d < bestD || (d === bestD && best && (c.col < best.col || (c.col === best.col && c.row < best.row)))) { best = c; bestD = d; }
      }
    }
    if (best) return best;
  }
  return firstFree(taken, cols, rows);
}

/**
 * Moves a dragged group so `anchor` lands on `target`, keeping relative
 * offsets. If that would leave the grid or sit on a non-dragged icon, each
 * dragged icon falls back to the nearest free cell instead — nothing ever
 * overlaps and nothing is ever lost off-screen.
 */
export function moveGroup(layout: Layout, dragged: string[], anchor: string, target: Cell, cols: number, rows: number): Layout {
  const a = layout[anchor];
  if (!a || !dragged.length) return layout;
  const draggedSet = new Set(dragged);
  const others = Object.entries(layout).filter(([n]) => !draggedSet.has(n));
  const taken = new Set(others.map(([, c]) => key(c)));
  const dc = target.col - a.col, dr = target.row - a.row;

  const straight = dragged.map((n) => ({ n, c: { col: layout[n].col + dc, row: layout[n].row + dr } }));
  const fits = straight.every(({ c }) => inBounds(c, cols, rows) && !taken.has(key(c)));
  const next: Layout = Object.fromEntries(others.map(([n, c]) => [n, c]));
  if (fits) {
    for (const { n, c } of straight) next[n] = c;
    return next;
  }
  const order = [anchor, ...dragged.filter((n) => n !== anchor)];
  const used = new Set(taken);
  for (const n of order) {
    const want = { col: layout[n].col + dc, row: layout[n].row + dr };
    const c = nearestFree(want, used, cols, rows);
    next[n] = c; used.add(key(c));
  }
  return next;
}

/** Moves a remembered position to a new file name (after a rename). */
export function renameKey(layout: Layout, from: string, to: string): Layout {
  if (!layout[from] || from === to) return layout;
  const { [from]: cell, ...rest } = layout;
  return { ...rest, [to]: cell };
}

/** Drops entries for files that no longer exist. */
export function prune(layout: Layout, names: string[]): Layout {
  const keep = new Set(names);
  return Object.fromEntries(Object.entries(layout).filter(([n]) => keep.has(n)));
}

/** Tolerant parser for the persisted JSON (corrupt/foreign data → empty layout). */
export function parseLayout(raw: string): Layout {
  try {
    const v = JSON.parse(raw);
    if (!v || typeof v !== 'object' || Array.isArray(v)) return {};
    const out: Layout = {};
    for (const [n, c] of Object.entries(v as Record<string, any>)) {
      if (c && Number.isInteger(c.col) && Number.isInteger(c.row) && c.col >= 0 && c.row >= 0) out[n] = { col: c.col, row: c.row };
    }
    return out;
  } catch { return {}; }
}

/** Unique file name inside `existing` ("a.txt" → "a (1).txt"). */
export function uniqueName(name: string, existing: Set<string>): string {
  if (!existing.has(name)) return name;
  const dot = name.lastIndexOf('.');
  const [stem, ext] = dot > 0 ? [name.slice(0, dot), name.slice(dot)] : [name, ''];
  for (let i = 1; ; i++) {
    const cand = `${stem} (${i})${ext}`;
    if (!existing.has(cand)) return cand;
  }
}
