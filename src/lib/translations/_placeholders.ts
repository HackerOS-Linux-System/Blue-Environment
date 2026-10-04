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

const locales: Record<string, Record<string, string>> = {
  arabic, chinese, czech, dutch, french, german, italian, japanese,
  polish, portuguese, russian, spanish, swedish, turkish, ukrainian,
};

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');

/** Returns "locale:key" for every translation whose `{placeholders}` differ from English. */
export function pl_check(): string[] {
  const bad: string[] = [];
  for (const [key, en] of Object.entries(english)) {
    const want = placeholders(en);
    for (const [name, dict] of Object.entries(locales)) {
      if (key in dict && placeholders(dict[key]) !== want) bad.push(`${name}:${key}`);
    }
  }
  return bad;
}
