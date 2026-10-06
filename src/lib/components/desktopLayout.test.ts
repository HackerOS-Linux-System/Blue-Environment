import { describe, it, expect } from 'vitest';
import { gridSize, cellAt, cellOrigin, placeAll, nearestFree, moveGroup, renameKey, prune, parseLayout, uniqueName, key, CELL_W, CELL_H, ORIGIN_X, ORIGIN_Y } from './desktopLayout';

const COLS = 6, ROWS = 4;
const unique = (l: Record<string, { col: number; row: number }>) => new Set(Object.values(l).map(key)).size === Object.keys(l).length;

describe('desktop grid', () => {
  it('computes grid size and never returns < 1', () => {
    expect(gridSize(1920, 1080)).toEqual({ cols: Math.floor((1920 - ORIGIN_X) / CELL_W), rows: Math.floor((1080 - ORIGIN_Y - 48) / CELL_H) });
    expect(gridSize(10, 10)).toEqual({ cols: 1, rows: 1 });
  });
  it('maps points to cells (clamped) and back', () => {
    expect(cellAt(ORIGIN_X + CELL_W * 2 + 5, ORIGIN_Y + CELL_H * 1 + 5, COLS, ROWS)).toEqual({ col: 2, row: 1 });
    expect(cellAt(-50, -50, COLS, ROWS)).toEqual({ col: 0, row: 0 });
    expect(cellAt(99999, 99999, COLS, ROWS)).toEqual({ col: COLS - 1, row: ROWS - 1 });
    expect(cellOrigin({ col: 2, row: 1 })).toEqual({ left: ORIGIN_X + 2 * CELL_W, top: ORIGIN_Y + CELL_H });
  });
});

describe('placeAll', () => {
  it('flows unplaced icons column-major, alphabetically', () => {
    const l = placeAll(['b', 'a', 'c', 'd', 'e'], {}, COLS, 3);
    expect(l['a']).toEqual({ col: 0, row: 0 });
    expect(l['b']).toEqual({ col: 0, row: 1 });
    expect(l['c']).toEqual({ col: 0, row: 2 });
    expect(l['d']).toEqual({ col: 1, row: 0 });
  });
  it('keeps remembered cells and flows the rest around them', () => {
    const l = placeAll(['a', 'b'], { b: { col: 0, row: 0 } }, COLS, ROWS);
    expect(l['b']).toEqual({ col: 0, row: 0 });
    expect(l['a']).toEqual({ col: 0, row: 1 });
  });
  it('relocates saved cells that are out of bounds or duplicated; never overlaps', () => {
    const l = placeAll(['a', 'b', 'c'], { a: { col: 99, row: 0 }, b: { col: 1, row: 1 }, c: { col: 1, row: 1 } }, COLS, ROWS);
    expect(unique(l)).toBe(true);
    expect(l['a'].col).toBeLessThan(COLS);
  });
  it('never loses an icon when the grid is full', () => {
    const names = Array.from({ length: 30 }, (_, i) => `f${i}`);
    const l = placeAll(names, {}, 3, 3);
    expect(Object.keys(l)).toHaveLength(30);
    expect(unique(l)).toBe(true);
  });
  it('does not mutate saved', () => {
    const saved = { a: { col: 99, row: 99 } };
    placeAll(['a'], saved, COLS, ROWS);
    expect(saved).toEqual({ a: { col: 99, row: 99 } });
  });
});

describe('nearestFree / moveGroup', () => {
  it('returns the target when free, else the closest free cell', () => {
    expect(nearestFree({ col: 2, row: 2 }, new Set(), COLS, ROWS)).toEqual({ col: 2, row: 2 });
    const c = nearestFree({ col: 2, row: 2 }, new Set([key({ col: 2, row: 2 })]), COLS, ROWS);
    expect(Math.abs(c.col - 2) + Math.abs(c.row - 2)).toBe(1);
  });
  it('moves a single icon', () => {
    const l = moveGroup({ a: { col: 0, row: 0 } }, ['a'], 'a', { col: 3, row: 2 }, COLS, ROWS);
    expect(l['a']).toEqual({ col: 3, row: 2 });
  });
  it('keeps relative offsets for a group', () => {
    const base = { a: { col: 0, row: 0 }, b: { col: 0, row: 1 }, c: { col: 5, row: 3 } };
    const l = moveGroup(base, ['a', 'b'], 'a', { col: 2, row: 1 }, COLS, ROWS);
    expect(l['a']).toEqual({ col: 2, row: 1 });
    expect(l['b']).toEqual({ col: 2, row: 2 });
    expect(l['c']).toEqual({ col: 5, row: 3 });
  });
  it('falls back to free cells when dropped on another icon, without overlap', () => {
    const base = { a: { col: 0, row: 0 }, b: { col: 2, row: 2 } };
    const l = moveGroup(base, ['a'], 'a', { col: 2, row: 2 }, COLS, ROWS);
    expect(l['b']).toEqual({ col: 2, row: 2 });
    expect(l['a']).not.toEqual({ col: 2, row: 2 });
    expect(unique(l)).toBe(true);
  });
  it('keeps a group on-screen when dropped near the edge', () => {
    const base = { a: { col: 0, row: 0 }, b: { col: 1, row: 0 }, c: { col: 2, row: 0 } };
    const l = moveGroup(base, ['a', 'b', 'c'], 'a', { col: COLS - 1, row: ROWS - 1 }, COLS, ROWS);
    expect(unique(l)).toBe(true);
    for (const c of Object.values(l)) { expect(c.col).toBeLessThan(COLS); expect(c.row).toBeLessThan(ROWS); }
  });
});

describe('helpers', () => {
  it('renameKey / prune', () => {
    expect(renameKey({ a: { col: 1, row: 1 } }, 'a', 'b')).toEqual({ b: { col: 1, row: 1 } });
    expect(renameKey({ a: { col: 1, row: 1 } }, 'x', 'y')).toEqual({ a: { col: 1, row: 1 } });
    expect(prune({ a: { col: 0, row: 0 }, b: { col: 1, row: 0 } }, ['b'])).toEqual({ b: { col: 1, row: 0 } });
  });
  it('parseLayout tolerates garbage', () => {
    expect(parseLayout('not json')).toEqual({});
    expect(parseLayout('[]')).toEqual({});
    expect(parseLayout('{"a":{"col":1,"row":2},"b":{"col":-1,"row":0},"c":"x"}')).toEqual({ a: { col: 1, row: 2 } });
  });
  it('uniqueName', () => {
    expect(uniqueName('a.txt', new Set())).toBe('a.txt');
    expect(uniqueName('a.txt', new Set(['a.txt']))).toBe('a (1).txt');
    expect(uniqueName('a.txt', new Set(['a.txt', 'a (1).txt']))).toBe('a (2).txt');
    expect(uniqueName('dir', new Set(['dir']))).toBe('dir (1)');
    expect(uniqueName('.bashrc', new Set(['.bashrc']))).toBe('.bashrc (1)');
  });
});
