import { describe, it, expect } from 'vitest';
import { filterApps, listLauncherApps, paginate, resolvePinned } from './apps';

const reg = {
  b: { id: 'b', title: 'Calculator', icon: 1 },
  a: { id: 'a', title: 'Żółw', icon: 2 },
  c: { id: 'c', title: 'Camera', icon: 3 },
  d: { id: 'd', title: 'About', icon: 4 },
};

describe('altShell apps', () => {
  it('lists apps alphabetically and honours appsEnabled / exclude', () => {
    expect(listLauncherApps(reg).map((a) => a.id)).toEqual(['d', 'b', 'c', 'a']);
    expect(listLauncherApps(reg, { b: false }).map((a) => a.id)).not.toContain('b');
    expect(listLauncherApps(reg, {}, ['d']).map((a) => a.id)).not.toContain('d');
    expect(listLauncherApps(reg, { b: true }).map((a) => a.id)).toContain('b');
  });

  it('filters ignoring case and diacritics', () => {
    const apps = listLauncherApps(reg);
    expect(filterApps(apps, 'zolw').map((a) => a.id)).toEqual(['a']);
    expect(filterApps(apps, 'CA').map((a) => a.id)).toEqual(['b', 'c']);
    expect(filterApps(apps, '  ')).toBe(apps);
    expect(filterApps(apps, 'nothing')).toEqual([]);
  });

  it('paginates', () => {
    expect(paginate([1, 2, 3, 4, 5], 2)).toEqual([[1, 2], [3, 4], [5]]);
    expect(paginate([], 4)).toEqual([]);
    expect(paginate([1, 2], 0)).toEqual([[1, 2]]);
  });

  it('resolves pinned apps, skipping missing/duplicate ids, falling back to the first apps', () => {
    const apps = listLauncherApps(reg);
    expect(resolvePinned(['c', 'zzz', 'c', 'b'], apps, 4).map((a) => a.id)).toEqual(['c', 'b']);
    expect(resolvePinned(['c', 'b', 'a'], apps, 2).map((a) => a.id)).toEqual(['c', 'b']);
    expect(resolvePinned(undefined, apps, 2).map((a) => a.id)).toEqual(['d', 'b']);
    expect(resolvePinned(['zzz'], apps, 2).map((a) => a.id)).toEqual(['d', 'b']);
  });
});
