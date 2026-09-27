import { describe, it, expect } from 'vitest';
import { english } from './english';
import { arabic } from './arabic';
import { chinese } from './chinese';
import { czech } from './czech';
import { dutch } from './dutch';
import { french } from './french';
import { german } from './german';
import { italian } from './italian';
import { japanese } from './japanese';
import { polish } from './polish';
import { portuguese } from './portuguese';
import { russian } from './russian';
import { spanish } from './spanish';
import { swedish } from './swedish';
import { turkish } from './turkish';
import { ukrainian } from './ukrainian';

// Regression test for the drift this codebase actually had: 15 of 16
// non-English locales were missing keys used by real, shipped UI
// (Screen Time, Cloned Apps, the on-screen keyboard settings, Blue
// Security, parts of the Welcome tour) — a missing key means `t(key)`
// falls back to printing the raw key string in the UI. See
// scripts/check-i18n-keys.mjs, which found and fixed this; this test
// exists so the same drift fails CI immediately next time instead of
// quietly shipping to users of every locale but the one someone
// happened to be testing in.
const locales: Record<string, Record<string, string>> = {
  arabic, chinese, czech, dutch, french, german, italian, japanese,
  polish, portuguese, russian, spanish, swedish, turkish, ukrainian,
};

describe('translation key parity', () => {
  const englishKeys = Object.keys(english);

  it('english.ts itself has no duplicate or empty keys', () => {
    expect(new Set(englishKeys).size).toBe(englishKeys.length);
    expect(englishKeys.every((k) => k.trim().length > 0)).toBe(true);
  });

  for (const [name, dict] of Object.entries(locales)) {
    it(`${name}.ts has every key english.ts has`, () => {
      const missing = englishKeys.filter((k) => !(k in dict));
      expect(missing, `missing keys in ${name}.ts: ${missing.join(', ')}`).toEqual([]);
    });

    it(`${name}.ts has no keys english.ts doesn't have (stale/typo keys)`, () => {
      const extra = Object.keys(dict).filter((k) => !(k in english));
      expect(extra, `extra keys in ${name}.ts not present in english.ts: ${extra.join(', ')}`).toEqual([]);
    });
  }
});
