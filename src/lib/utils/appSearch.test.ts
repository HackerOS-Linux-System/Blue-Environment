import { describe, it, expect } from 'vitest';
import { fold, tokenize, scoreApp, searchApps, execBasename } from './appSearch';

interface A { name: string; extra?: string }
const f = (a: A) => a;

describe('fold', () => {
  it('lower-cases and strips diacritics, including letters NFD cannot decompose', () => {
    expect(fold('Zażółć Gęślą Jaźń')).toBe('zazolc gesla jazn');
    expect(fold('Łódź')).toBe('lodz');
    expect(fold('Über')).toBe('uber');
  });
});

describe('tokenize', () => {
  it('splits on any whitespace and drops empties — a trailing space must not change the query', () => {
    expect(tokenize('  system   monitor ')).toEqual(['system', 'monitor']);
    expect(tokenize('system ')).toEqual(['system']);
    expect(tokenize('   ')).toEqual([]);
  });
});

describe('searchApps', () => {
  const apps: A[] = [
    { name: 'System Monitor', extra: 'Monitor CPU and memory' },
    { name: 'Settings', extra: 'Configure the system' },
    { name: 'Blue Web', extra: 'Web browser firefox' },
    { name: 'Terminal' },
    { name: 'Zrzut ekranu' },
    { name: 'File Explorer', extra: 'Browse files' },
  ];

  it('returns everything unchanged for an empty query', () => {
    expect(searchApps(apps, '', f)).toBe(apps);
    expect(searchApps(apps, '   ', f)).toBe(apps);
  });

  it('a query containing a space still matches (multi-word search)', () => {
    expect(searchApps(apps, 'system monitor', f).map((a) => a.name)).toEqual(['System Monitor']);
    expect(searchApps(apps, 'system ', f).map((a) => a.name)[0]).toBe('System Monitor');
  });

  it('every token must match, in any order', () => {
    expect(searchApps(apps, 'monitor system', f).map((a) => a.name)).toEqual(['System Monitor']);
    expect(searchApps(apps, 'web zzz', f)).toEqual([]);
  });

  it('ranks name-prefix hits above word-start hits above substring/extra hits', () => {
    const r = searchApps(apps, 'sys', f).map((a) => a.name);
    // "System Monitor" starts with it; "Settings" only mentions it in the description.
    expect(r).toEqual(['System Monitor', 'Settings']);
    const w = searchApps(apps, 'mon', f).map((a) => a.name);
    expect(w[0]).toBe('System Monitor');
  });

  it('exact name beats prefix matches', () => {
    const items: A[] = [{ name: 'Terminal Emulator' }, { name: 'Terminal' }];
    expect(searchApps(items, 'terminal', f)[0].name).toBe('Terminal');
  });

  it('finds apps through their description/command and ignores diacritics', () => {
    expect(searchApps(apps, 'firefox', f).map((a) => a.name)).toEqual(['Blue Web']);
    expect(searchApps(apps, 'zrzut', f).map((a) => a.name)).toEqual(['Zrzut ekranu']);
    expect(searchApps([{ name: 'Łódzka mapa' }], 'lodz', f)).toHaveLength(1);
  });

  it('ties are alphabetical so results do not jump around', () => {
    const items: A[] = [{ name: 'Beta tool' }, { name: 'Alpha tool' }];
    expect(searchApps(items, 'tool', f).map((a) => a.name)).toEqual(['Alpha tool', 'Beta tool']);
  });
});

describe('scoreApp', () => {
  it('is null when a token matches nothing', () => {
    expect(scoreApp({ name: 'Terminal' }, ['xyz'])).toBeNull();
  });
});

describe('execBasename', () => {
  it('extracts the program name', () => {
    expect(execBasename('/usr/bin/firefox --new-window %u')).toBe('firefox');
    expect(execBasename('code')).toBe('code');
    expect(execBasename(undefined)).toBe('');
  });
});
