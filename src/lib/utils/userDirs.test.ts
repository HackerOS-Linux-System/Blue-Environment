import { describe, it, expect } from 'vitest';
import { buildPlaces, defaultUserDirs, normalizeUserDirs, userDirPath, userDirTilde } from './userDirs';

const raw = [
  { key: 'desktop', path: '/home/ala/Pulpit', home_path: 'HOME/Pulpit', name: 'Pulpit', exists: true },
  { key: 'documents', path: '/home/ala/Dokumenty', home_path: 'HOME/Dokumenty', name: 'Dokumenty', exists: true },
  { key: 'downloads', path: '/home/ala/Pobrane', home_path: 'HOME/Pobrane', name: 'Pobrane', exists: true },
  { key: 'music', path: '/mnt/dane/Muzyka', home_path: null, name: 'Muzyka', exists: true },
  { key: 'videos', path: '/home/ala/Wideo', home_path: 'HOME/Wideo', name: 'Wideo', exists: false },
];

describe('userDirs', () => {
  it('defaults to the English names', () => {
    const d = defaultUserDirs();
    expect(userDirPath('downloads', d)).toBe('HOME/Downloads');
    expect(userDirTilde('desktop', d)).toBe('~/Desktop');
  });

  it('uses the localized folders reported by the backend', () => {
    const d = normalizeUserDirs(raw);
    expect(userDirPath('downloads', d)).toBe('HOME/Pobrane');
    expect(userDirTilde('documents', d)).toBe('~/Dokumenty');
  });

  it('uses the absolute path for folders outside the home directory', () => {
    const d = normalizeUserDirs(raw);
    expect(userDirPath('music', d)).toBe('/mnt/dane/Muzyka');
  });

  it('fills missing keys with English defaults and ignores garbage', () => {
    const d = normalizeUserDirs([{ nope: 1 }, null, raw[0]]);
    expect(userDirPath('desktop', d)).toBe('HOME/Pulpit');
    expect(userDirPath('pictures', d)).toBe('HOME/Pictures');
    expect(normalizeUserDirs('garbage')).toEqual(defaultUserDirs());
  });

  it('builds sidebar places with real names and skips missing folders', () => {
    const places = buildPlaces(normalizeUserDirs(raw), 'Dom');
    expect(places[0]).toEqual({ name: 'Dom', path: 'HOME', iconName: 'Home' });
    const names = places.map((p) => p.name);
    expect(names).toContain('Pobrane');
    expect(names).toContain('Pulpit');
    expect(names).not.toContain('Wideo'); // exists: false
    expect(places.find((p) => p.name === 'Pobrane')?.iconName).toBe('Download');
  });
});
