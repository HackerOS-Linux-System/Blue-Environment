import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { english } from './english';
import { pl_check } from './_placeholders';

// Guards two things the parity test can't see:
//  1. every literal key used in the UI — `$t('x')`, `t('x')`, `translate('x')` —
//     really exists in english.ts (a typo silently prints the raw key to users);
//  2. every `{placeholder}` in an English string is present in all translations
//     (a translator dropping `{name}` would silently lose the dynamic part).

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(svelte|ts)$/.test(name) && !/\.test\.ts$/.test(name) && !p.includes('/translations/')) out.push(p);
  }
  return out;
}

describe('translation key usage', () => {
  const root = join(__dirname, '..');
  const files = walk(root);
  const used = new Map<string, string>();
  const re = /(?:\$t|\btranslate|\btr)\(\s*'([a-z0-9_]+(?:\.[a-z0-9_]+)+)'/g;
  for (const f of files) {
    const src = readFileSync(f, 'utf-8');
    for (const m of src.matchAll(re)) used.set(m[1], f);
  }

  it('finds a reasonable number of used keys (sanity check for the scanner)', () => {
    expect(used.size).toBeGreaterThan(20);
  });

  it('every literal key used in code exists in english.ts', () => {
    const missing = [...used.entries()].filter(([k]) => !(k in english)).map(([k, f]) => `${k}  (${f.replace(root, '')})`);
    expect(missing, `keys used in code but missing from english.ts:\n${missing.join('\n')}`).toEqual([]);
  });

  it('dynamic key families are complete', () => {
    const families: Record<string, string[]> = {
      'blueweb.panel.': ['bookmarks', 'history', 'downloads'],
      'blueweb.state.': ['downloading', 'done', 'error', 'cancelled'],
      'blueconnect.device.': ['phone', 'tablet', 'desktop', 'laptop', 'tv', 'unknown'],
      'startmenu.cat.': ['internet', 'multimedia', 'graphics', 'office', 'development', 'games', 'system', 'other'],
      'cal.view.': ['month', 'week', 'day'],
      'music.repeat_': ['off', 'all', 'one'],
      'cal.unit.': ['daily', 'weekly', 'monthly', 'yearly'],
    };
    const missing = Object.entries(families).flatMap(([prefix, names]) => names.map((n) => prefix + n)).filter((k) => !(k in english));
    expect(missing).toEqual([]);
  });

  it('placeholders are preserved in every translation', () => {
    expect(pl_check()).toEqual([]);
  });
});
