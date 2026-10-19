import { describe, it, expect } from 'vitest';
import { parseSnapshotName, joinRel, breadcrumbs, parentRel, originalPath, isInside, destinationProblem, toggleSource } from './backupView';

describe('backupView', () => {
  it('parses snapshot names', () => {
    const d = parseSnapshotName('snap-20261016-093015')!;
    expect([d.getFullYear(), d.getMonth(), d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds()]).toEqual([2026, 9, 16, 9, 30, 15]);
    expect(parseSnapshotName('snap-20261340-000000')).toBeNull();
    expect(parseSnapshotName('x')).toBeNull();
  });

  it('builds paths', () => {
    expect(joinRel('', 'home')).toBe('home');
    expect(joinRel('home/u', 'Docs')).toBe('home/u/Docs');
    expect(breadcrumbs('home/u/Docs')).toEqual([
      { label: 'home', path: 'home' }, { label: 'u', path: 'home/u' }, { label: 'Docs', path: 'home/u/Docs' },
    ]);
    expect(parentRel('home/u/Docs')).toBe('home/u');
    expect(parentRel('home')).toBe('');
    expect(originalPath('home/u/a.txt')).toBe('/home/u/a.txt');
  });

  it('detects a destination inside a source', () => {
    expect(isInside('/home/u/backups', '/home/u')).toBe(true);
    expect(isInside('/home/user2', '/home/u')).toBe(false);
    expect(isInside('/home/u', '/home/u/')).toBe(true);
    expect(destinationProblem({ destination: '', sources: ['/home/u'] })).toBe('no_destination');
    expect(destinationProblem({ destination: '/mnt/b', sources: [] })).toBe('no_sources');
    expect(destinationProblem({ destination: '/home/u/b', sources: ['/home/u'] })).toBe('inside_source');
    expect(destinationProblem({ destination: '/mnt/b', sources: ['/home/u'] })).toBeNull();
  });

  it('toggles sources', () => {
    expect(toggleSource(['/a'], '/b')).toEqual(['/a', '/b']);
    expect(toggleSource(['/a', '/b'], '/a')).toEqual(['/b']);
  });
});
