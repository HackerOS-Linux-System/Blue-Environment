import { describe, it, expect } from 'vitest';
import { pickNext, type Box } from './spatialNav';

const box = (x: number, y: number, w = 100, h = 100): Box => ({ x, y, w, h });

describe('pickNext (grid of 3x2 tiles)', () => {
  //  0 1 2
  //  3 4 5
  const grid = [box(0, 0), box(120, 0), box(240, 0), box(0, 120), box(120, 120), box(240, 120)];
  const from = (i: number) => grid[i];
  const others = (i: number) => grid.filter((_, j) => j !== i);
  const idOf = (i: number, dir: 'up' | 'down' | 'left' | 'right') => {
    const list = others(i);
    const k = pickNext(from(i), list, dir);
    return k < 0 ? -1 : grid.indexOf(list[k]);
  };

  it('moves along rows and columns', () => {
    expect(idOf(0, 'right')).toBe(1);
    expect(idOf(1, 'right')).toBe(2);
    expect(idOf(2, 'left')).toBe(1);
    expect(idOf(0, 'down')).toBe(3);
    expect(idOf(4, 'up')).toBe(1);
  });

  it('returns -1 at the edges', () => {
    expect(idOf(2, 'right')).toBe(-1);
    expect(idOf(0, 'left')).toBe(-1);
    expect(idOf(0, 'up')).toBe(-1);
    expect(idOf(5, 'down')).toBe(-1);
  });

  it('prefers a tile in the same row over a diagonally closer one', () => {
    const f = box(0, 0);
    const sameRowFar = box(400, 0);
    const diagonalNear = box(110, 150);
    expect(pickNext(f, [diagonalNear, sameRowFar], 'right')).toBe(1);
  });

  it('works with an uneven layout and ignores itself', () => {
    const f = box(0, 0, 300, 200);
    const list = [f, box(0, 300, 100, 100), box(150, 300, 100, 100)];
    // from the wide tile, "down" should pick the one whose centre is closer to its own centre
    expect(pickNext(f, list, 'down')).toBe(2);
  });
});
