import { describe, it, expect } from 'vitest';
import {
  parseWidgets, serializeWidgets, createWidget, clampToViewport, snap, pushHistory, sparklinePoints,
  weatherGlyph, monthGrid, clockAngles, bringToFront, updateWidget, DEFAULT_SIZE, type WidgetInstance,
} from './widgetModel';

const w = (id: string, over: Partial<WidgetInstance> = {}): WidgetInstance =>
  ({ id, type: 'clock', x: 0, y: 0, w: 200, h: 100, config: {}, ...over });

describe('widget persistence', () => {
  it('round-trips', () => {
    const list = [createWidget('clock', [], 1920, 1080), createWidget('notes', [], 1920, 1080)];
    const back = parseWidgets(serializeWidgets(list));
    expect(back.map((x) => x.type)).toEqual(['clock', 'notes']);
    expect(back[0].config).toMatchObject({ analog: false });
  });
  it('drops garbage and unknown types, repairs missing fields', () => {
    const raw = JSON.stringify({ widgets: [null, 5, { type: 'nope' }, { type: 'weather' }, { type: 'weather', id: 'a' }, { type: 'notes', id: 'a', x: 'zz', w: 1 }] });
    const out = parseWidgets(raw);
    expect(out).toHaveLength(3);
    expect(new Set(out.map((o) => o.id)).size).toBe(3); // duplicate ids repaired
    expect(out[2].w).toBeGreaterThanOrEqual(140);       // below-minimum width clamped up
    expect(out[2].x).toBe(24);                          // non-numeric x → default
    expect(parseWidgets('not json')).toEqual([]);
    expect(parseWidgets(null)).toEqual([]);
  });
  it('accepts a bare array too', () => {
    expect(parseWidgets(JSON.stringify([{ type: 'system' }]))).toHaveLength(1);
  });
});

describe('placement', () => {
  it('stays inside the viewport', () => {
    const c = clampToViewport(w('a', { x: 5000, y: -40, w: 400, h: 300 }), 1000, 700);
    expect(c.x).toBe(600);
    expect(c.y).toBe(0);
  });
  it('snaps to the 10px grid', () => {
    expect(snap(14)).toBe(10);
    expect(snap(16)).toBe(20);
  });
  it('new widgets do not overlap existing ones when there is room', () => {
    const a = createWidget('clock', [], 1920, 1080);
    const b = createWidget('clock', [a], 1920, 1080);
    expect(b.y).toBeGreaterThanOrEqual(a.y + a.h);
  });
  it('uses the default size per type', () => {
    expect(createWidget('calendar', [], 1920, 1080)).toMatchObject(DEFAULT_SIZE.calendar);
  });
  it('bringToFront moves to the end; updateWidget merges config', () => {
    const l = [w('a'), w('b'), w('c')];
    expect(bringToFront(l, 'a').map((x) => x.id)).toEqual(['b', 'c', 'a']);
    const u = updateWidget([w('a', { config: { x: 1 } })], 'a', { config: { y: 2 } });
    expect(u[0].config).toEqual({ x: 1, y: 2 });
  });
});

describe('widget helpers', () => {
  it('history keeps the newest N samples', () => {
    let h: number[] = [];
    for (let i = 0; i < 50; i++) h = pushHistory(h, i, 10);
    expect(h).toHaveLength(10);
    expect(h[9]).toBe(49);
    expect(pushHistory([], NaN)).toEqual([0]);
  });
  it('sparkline scales to the box', () => {
    expect(sparklinePoints([0, 100], 100, 50)).toBe('0.0,50.0 100.0,0.0');
    expect(sparklinePoints([], 10, 10)).toBe('');
    expect(sparklinePoints([250], 10, 10)).toBe('0.0,0.0'); // clamped
  });
  it('maps WMO codes', () => {
    expect(weatherGlyph(0)).toBe('☀️');
    expect(weatherGlyph(63)).toBe('🌧️');
    expect(weatherGlyph(95)).toBe('⛈️');
  });
  it('builds a 42-cell month grid with the right offset and today', () => {
    // Oct 2026 starts on a Thursday
    const mon = monthGrid(2026, 9, true, new Date(2026, 9, 21));
    expect(mon).toHaveLength(42);
    expect(mon[3]).toMatchObject({ day: 1, inMonth: true });
    expect(mon.find((c) => c.isToday)?.day).toBe(21);
    const sun = monthGrid(2026, 9, false, new Date(2026, 9, 21));
    expect(sun[4]).toMatchObject({ day: 1, inMonth: true });
    expect(mon.filter((c) => c.inMonth)).toHaveLength(31);
  });
  it('clock angles', () => {
    expect(clockAngles(new Date(2026, 0, 1, 3, 0, 0))).toEqual({ hour: 90, minute: 0, second: 0 });
    expect(clockAngles(new Date(2026, 0, 1, 12, 30, 30)).second).toBe(180);
  });
});
