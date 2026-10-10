import { describe, it, expect } from 'vitest';
import type { ExternalWindow, WindowState } from '../types';
import {
  buildRunningItems, pickVisible, clampRunningMax, groupByWorkspace,
  RUNNING_APPS_DEFAULT_MAX, RUNNING_APPS_MAX_LIMIT, type RunningItem,
} from './runningApps';

const blue = (id: string, extra: Partial<WindowState> = {}): WindowState =>
  ({ id, appId: 'terminal', title: `T ${id}`, isMinimized: false, workspace: 0, ...extra }) as WindowState;
const native = (id: string, extra: Partial<ExternalWindow> = {}): ExternalWindow =>
  ({ id, pid: 1, title: `N ${id}`, class: 'cls', iconPath: '', isMinimized: false, desktop: 0, ...extra });
const item = (id: string, extra: Partial<RunningItem> = {}): RunningItem =>
  ({ id, title: id, isExternal: false, isMinimized: false, isActive: false, workspace: 0, ...extra });

describe('buildRunningItems', () => {
  it('lists Blue windows first, then native ones, and marks the active window', () => {
    const items = buildRunningItems([blue('a'), blue('b')], [native('x')], 'b');
    expect(items.map((i) => i.id)).toEqual(['a', 'b', 'x']);
    expect(items.find((i) => i.id === 'b')?.isActive).toBe(true);
    expect(items.find((i) => i.id === 'x')?.isExternal).toBe(true);
  });

  it('lists a native window embedded in a Blue window only once', () => {
    const items = buildRunningItems([blue('w', { externalWindowId: 'x', isExternal: true })], [native('x'), native('y')], null);
    expect(items.map((i) => i.id)).toEqual(['w', 'y']);
  });

  it('falls back to the window class when a native window has no title', () => {
    const [it0] = buildRunningItems([], [native('x', { title: '', class: 'firefox' })], null);
    expect(it0.title).toBe('firefox');
  });

  it('keeps a native window\'s workspace and minimized state', () => {
    const [it0] = buildRunningItems([], [native('x', { desktop: 2, isMinimized: true })], null);
    expect(it0.workspace).toBe(2);
    expect(it0.isMinimized).toBe(true);
  });
});

describe('clampRunningMax', () => {
  it('defaults to 4 and clamps to 1..12', () => {
    expect(clampRunningMax(undefined)).toBe(RUNNING_APPS_DEFAULT_MAX);
    expect(clampRunningMax(NaN)).toBe(RUNNING_APPS_DEFAULT_MAX);
    expect(clampRunningMax(0)).toBe(1);
    expect(clampRunningMax(99)).toBe(RUNNING_APPS_MAX_LIMIT);
    expect(clampRunningMax(6.4)).toBe(6);
  });
});

describe('pickVisible', () => {
  const six = ['a', 'b', 'c', 'd', 'e', 'f'].map((id) => item(id));

  it('shows everything when it fits', () => {
    expect(pickVisible(six.slice(0, 3), 4)).toEqual({ visible: six.slice(0, 3), hidden: 0 });
  });

  it('shows the first N and counts the rest', () => {
    const r = pickVisible(six, 4);
    expect(r.visible.map((i) => i.id)).toEqual(['a', 'b', 'c', 'd']);
    expect(r.hidden).toBe(2);
  });

  it('never hides the focused window behind the +N chip', () => {
    const items = six.map((i) => (i.id === 'f' ? { ...i, isActive: true } : i));
    const r = pickVisible(items, 4);
    expect(r.visible.map((i) => i.id)).toEqual(['a', 'b', 'c', 'f']);
    expect(r.hidden).toBe(2);
  });

  it('leaves order untouched when the focused window is already visible', () => {
    const items = six.map((i) => (i.id === 'b' ? { ...i, isActive: true } : i));
    expect(pickVisible(items, 4).visible.map((i) => i.id)).toEqual(['a', 'b', 'c', 'd']);
  });

  it('respects a custom limit', () => {
    expect(pickVisible(six, 2).visible).toHaveLength(2);
    expect(pickVisible(six, 2).hidden).toBe(4);
  });
});

describe('groupByWorkspace', () => {
  it('puts the current workspace first, then the others in order', () => {
    const groups = groupByWorkspace([item('a', { workspace: 2 }), item('b', { workspace: 0 }), item('c', { workspace: 1 }), item('d', { workspace: 1 })], 1);
    expect(groups.map((g) => g.workspace)).toEqual([1, 0, 2]);
    expect(groups[0].items.map((i) => i.id)).toEqual(['c', 'd']);
  });

  it('returns nothing for no windows', () => {
    expect(groupByWorkspace([], 0)).toEqual([]);
  });
});
