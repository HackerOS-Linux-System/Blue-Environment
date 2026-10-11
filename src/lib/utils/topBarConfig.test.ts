import { describe, it, expect } from 'vitest';
import {
  BAR_RIGHT_ITEMS, DEFAULT_TOP_BAR, moveRightItem, normalizeHexColor, normalizeRightOrder, normalizeTopBar, rightItemOrder,
} from './topBarConfig';

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
      .toEqual(['clipboard', 'weather', 'workspaces', 'battery', 'network', 'notifications']);
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
});
