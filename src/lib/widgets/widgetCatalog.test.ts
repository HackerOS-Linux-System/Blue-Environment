import { describe, it, expect } from 'vitest';
import {
  bundledCatalog, sanitizeCatalog, sanitizeEntry, searchCatalog, installable,
} from './widgetCatalog';
import {
  countdownParts, parseTargetDate, parseZones, zoneLabel, parseQuotes, dailyIndex, parseLinks, isSafeLinkUrl,
  parseTodos, serializeTodos, formatTimer, createWidget, parseWidgets, serializeWidgets, WIDGET_TYPES,
} from './widgetModel';

describe('bundled Widget Store catalogue', () => {
  const { entries, warnings } = bundledCatalog();

  it('is not empty and has no invalid entries', () => {
    expect(entries.length).toBeGreaterThanOrEqual(10);
    expect(warnings).toEqual([]);
  });
  it('keeps the five built-in widgets and adds installable ones', () => {
    expect(entries.filter((e) => e.builtin).map((e) => e.type).sort()).toEqual(['calendar', 'clock', 'notes', 'system', 'weather']);
    expect(installable(entries).length).toBeGreaterThanOrEqual(8);
  });
  it('uses unique ids and known widget engines only', () => {
    expect(new Set(entries.map((e) => e.id)).size).toBe(entries.length);
    expect(entries.every((e) => WIDGET_TYPES.includes(e.type))).toBe(true);
  });
  it('has presets whose own settings are valid', () => {
    for (const e of entries) {
      const w = createWidget(e.type, [], 1920, 1080, { storeId: e.id, size: e.size, config: e.config });
      expect(w.storeId).toBe(e.id);
      expect(w.w).toBeGreaterThanOrEqual(140);
    }
    const world = entries.find((e) => e.id === 'world-clock')!;
    expect(parseZones(String(world.config!.zones)).length).toBe(3);
    for (const e of entries.filter((x) => x.type === 'countdown')) expect(Number.isNaN(parseTargetDate(String(e.config!.date)))).toBe(false);
    for (const e of entries.filter((x) => x.type === 'links')) expect(parseLinks(String(e.config!.links)).length).toBeGreaterThan(0);
  });
});

describe('sanitizeCatalog', () => {
  it('skips junk, duplicates and unknown engines, clamps sizes and config', () => {
    const r = sanitizeCatalog({ widgets: [
      null, 5, { id: 'Bad Id', name: 'x', type: 'clock' }, { id: 'ok-one', name: 'One', type: 'todo', size: { w: 5000, h: 1 }, config: { a: 'x', 'b c': 1, d: { e: 1 } } },
      { id: 'ok-one', name: 'Dup', type: 'todo' }, { id: 'evil', name: 'Evil', type: 'shell' },
    ] });
    expect(r.entries.map((e) => e.id)).toEqual(['ok-one']);
    expect(r.entries[0].size).toEqual({ w: 800, h: 90 });
    expect(r.entries[0].config).toEqual({ a: 'x' });
    expect(r.warnings).toHaveLength(5);
    expect(sanitizeCatalog('nope').entries).toEqual([]);
    expect(sanitizeCatalog([{ id: 'ab', name: 'A', type: 'timer' }]).entries).toHaveLength(1);
  });
  it('only lets built-in engines claim to be built in', () => {
    expect(sanitizeEntry({ id: 'cc', name: 'C', type: 'todo', builtin: true }).entry?.builtin).toBeUndefined();
    expect(sanitizeEntry({ id: 'cc', name: 'C', type: 'clock', builtin: true }).entry?.builtin).toBe(true);
  });
  it('searches by name, tag and category', () => {
    const { entries } = bundledCatalog();
    expect(searchCatalog(entries, 'pomodoro').map((e) => e.id)).toContain('pomodoro');
    expect(searchCatalog(entries, 'quotes').length).toBeGreaterThanOrEqual(2);
    expect(searchCatalog(entries, '', 'Inspiration').every((e) => e.category === 'Inspiration')).toBe(true);
    expect(searchCatalog(entries, 'zzzz-nothing')).toEqual([]);
  });
});

describe('store widget helpers', () => {
  it('counts down and never goes negative', () => {
    const now = Date.UTC(2026, 0, 1);
    expect(countdownParts(now + 90_061_000, now)).toMatchObject({ done: false, days: 1, hours: 1, minutes: 1, seconds: 1 });
    expect(countdownParts(now - 5, now).done).toBe(true);
    expect(countdownParts(NaN, now).done).toBe(true);
  });
  it('parses dates strictly', () => {
    expect(parseTargetDate('2027-01-01T00:00')).toBe(new Date(2027, 0, 1, 0, 0).getTime());
    expect(parseTargetDate('2027-02-31')).toBeNaN();
    expect(parseTargetDate('tomorrow')).toBeNaN();
  });
  it('keeps only real time zones', () => {
    expect(parseZones('Europe/Warsaw, Nope/Zone, Europe/Warsaw\nAsia/Tokyo')).toEqual(['Europe/Warsaw', 'Asia/Tokyo']);
    expect(parseZones('Europe/Warsaw,Europe/Berlin,Europe/Paris', 2)).toHaveLength(2);
    expect(zoneLabel('America/New_York')).toBe('New York');
  });
  it('splits quotes from authors and rotates daily', () => {
    const q = parseQuotes('Plain line\nBe kind — Someone\nOther - Person');
    expect(q).toEqual([{ text: 'Plain line', author: '' }, { text: 'Be kind', author: 'Someone' }, { text: 'Other', author: 'Person' }]);
    expect(dailyIndex(5, new Date(2026, 5, 1))).not.toBe(dailyIndex(5, new Date(2026, 5, 2)));
    expect(dailyIndex(0)).toBe(0);
  });
  it('accepts only web and mail links', () => {
    expect(isSafeLinkUrl('https://example.org/a?b=1')).toBe(true);
    expect(isSafeLinkUrl('javascript:alert(1)')).toBe(false);
    expect(isSafeLinkUrl('file:///etc/passwd')).toBe(false);
    expect(parseLinks('A|https://a.org\nB|javascript:x\nno separator\n|https://c.org')).toEqual([{ label: 'A', url: 'https://a.org' }]);
  });
  it('round-trips to-do items and survives garbage', () => {
    const items = [{ id: 'a', text: 'One', done: true }, { id: 'b', text: 'Two', done: false }];
    expect(parseTodos(serializeTodos(items))).toEqual(items);
    expect(parseTodos('not json')).toEqual([]);
    expect(parseTodos(JSON.stringify([1, { text: 5 }, { text: 'ok' }]))).toEqual([{ id: 't0', text: 'ok', done: false }]);
    expect(parseTodos(undefined)).toEqual([]);
  });
  it('formats timers', () => {
    expect(formatTimer(25 * 60_000)).toBe('25:00');
    expect(formatTimer(3_661_000)).toBe('1:01:01');
    expect(formatTimer(-5)).toBe('00:00');
  });
  it('persists the store id of a widget', () => {
    const w = createWidget('timer', [], 1920, 1080, { storeId: 'pomodoro', config: { minutes: 50 } });
    const back = parseWidgets(serializeWidgets([w]));
    expect(back[0].storeId).toBe('pomodoro');
    expect(back[0].config.minutes).toBe(50);
  });
});
