import { describe, it, expect } from 'vitest';
import {
  BAR_LEFT_ITEMS, BAR_RIGHT_ITEMS, DEFAULT_TOP_BAR, moveItemTo, moveRightItem, normalizeHexColor, normalizeLeftOrder,
  normalizeRightOrder, normalizeTopBar, rightItemOrder,
} from './topBarConfig';
import { BAR_PRESETS, applyBarPreset } from './barPresets';

describe('topBarConfig (extended options)', () => {
  it('an empty/old saved config gets the new defaults and keeps old values', () => {
    const bar = normalizeTopBar({ showClock: false, pinnedIconSize: 24 });
    expect(bar.showClock).toBe(false);
    expect(bar.pinnedIconSize).toBe(24);
    expect(bar.backgroundMode).toBe('theme');
    expect(bar.rightOrder).toEqual([...BAR_RIGHT_ITEMS]);
    expect(normalizeTopBar(null)).toEqual(normalizeTopBar(undefined));
    expect(normalizeTopBar(DEFAULT_TOP_BAR)).toEqual(DEFAULT_TOP_BAR);
  });

  it('clamps numbers and rejects unknown enum values', () => {
    const bar = normalizeTopBar({
      fontSize: 99, itemSpacing: -4, backgroundOpacity: 1,
      startButtonStyle: 'weird' as any, pinnedStyle: 'x' as any, indicatorStyle: 1 as any, workspaceStyle: null as any,
      backgroundMode: 'rainbow' as any,
    });
    expect(bar.fontSize).toBe(18);
    expect(bar.itemSpacing).toBe(0);
    expect(bar.backgroundOpacity).toBe(10);
    expect(bar.startButtonStyle).toBe('rounded');
    expect(bar.pinnedStyle).toBe('dock');
    expect(bar.indicatorStyle).toBe('line');
    expect(bar.workspaceStyle).toBe('dots');
    expect(bar.backgroundMode).toBe('theme');
  });

  it('normalizes the widget order: drops junk and duplicates, appends missing ones', () => {
    expect(normalizeRightOrder(['clipboard', 'bogus', 'clipboard', 'weather']))
      .toEqual(['clipboard', 'weather', 'workspaces', 'battery', 'network', 'notifications', 'showDesktop', 'power']);
    expect(normalizeRightOrder('nope')).toEqual([...BAR_RIGHT_ITEMS]);
    expect(normalizeRightOrder([])).toEqual([...BAR_RIGHT_ITEMS]);
  });

  it('moves widgets within bounds only', () => {
    const o = [...BAR_RIGHT_ITEMS];
    expect(moveRightItem(o, 1, -1)[0]).toBe('weather');
    expect(moveRightItem(o, 0, -1)).toBe(o);
    expect(moveRightItem(o, o.length - 1, 1)).toBe(o);
    expect(o).toEqual([...BAR_RIGHT_ITEMS]); // never mutates
  });

  it('gives each widget a 1-based CSS order', () => {
    const o = normalizeRightOrder(['notifications']);
    expect(rightItemOrder(o, 'notifications')).toBe(1);
    expect(rightItemOrder(o, 'workspaces')).toBe(2);
  });

  it('normalizes hex colours', () => {
    expect(normalizeHexColor('#ABCDEF', '#000000')).toBe('#abcdef');
    expect(normalizeHexColor('#abc', '#000000')).toBe('#aabbcc');
    expect(normalizeHexColor('red', '#123456')).toBe('#123456');
    expect(normalizeHexColor(5, '#123456')).toBe('#123456');
  });

  it('hides the two optional widgets by default and keeps them off for old configs', () => {
    expect(DEFAULT_TOP_BAR.showDesktopButton).toBe(false);
    expect(DEFAULT_TOP_BAR.showPowerButton).toBe(false);
    const old = normalizeTopBar({ rightOrder: ['clipboard', 'weather'] as any });
    expect(old.showPowerButton).toBe(false);
    expect(old.rightOrder.slice(-2)).toEqual(['showDesktop', 'power']);
  });

  it('normalizes the left order and survives garbage', () => {
    expect(normalizeLeftOrder(['running'])).toEqual(['running', 'start']);
    expect(normalizeLeftOrder(['x', 'start', 'start'])).toEqual(['start', 'running']);
    expect(normalizeTopBar({ leftOrder: 5 as any }).leftOrder).toEqual([...BAR_LEFT_ITEMS]);
  });

  it('moves an item to an absolute position (drag and drop)', () => {
    const o = ['a', 'b', 'c', 'd'];
    expect(moveItemTo(o, 0, 2)).toEqual(['b', 'c', 'a', 'd']);
    expect(moveItemTo(o, 3, 0)).toEqual(['d', 'a', 'b', 'c']);
    expect(moveItemTo(o, 1, 1)).toBe(o);
    expect(moveItemTo(o, -1, 2)).toBe(o);
    expect(moveItemTo(o, 0, 9)).toBe(o);
    expect(o).toEqual(['a', 'b', 'c', 'd']);
  });
});

describe('bar presets', () => {
  it('every preset yields a valid, normalized config', () => {
    for (const p of BAR_PRESETS) {
      const out = applyBarPreset(DEFAULT_TOP_BAR, p.id);
      expect(normalizeTopBar(out)).toEqual(out);
    }
  });

  it('is based on the defaults, not on the current look', () => {
    const messy = normalizeTopBar({ fontSize: 17, shadow: true, floating: true });
    const out = applyBarPreset(messy, 'minimal');
    expect(out.shadow).toBe(false);
    expect(out.floating).toBe(false);
    expect(out.fontSize).toBe(11);
    expect(out.showWeather).toBe(false);
  });

  it('keeps personal choices: start label and clock format', () => {
    const mine = normalizeTopBar({ startLabel: 'Ala', clock24h: false, showSeconds: true, clockStyle: 'time-date' });
    const out = applyBarPreset(mine, 'dock');
    expect(out.startLabel).toBe('Ala');
    expect(out.clock24h).toBe(false);
    expect(out.showSeconds).toBe(true);
    expect(out.clockStyle).toBe('time-date');
    expect(out.floating).toBe(true);
  });

  it('unknown ids change nothing; "default" restores the default look', () => {
    const cur = normalizeTopBar({ fontSize: 15 });
    expect(applyBarPreset(cur, 'nope')).toBe(cur);
    expect(applyBarPreset(cur, 'default').fontSize).toBe(DEFAULT_TOP_BAR.fontSize);
  });
});
