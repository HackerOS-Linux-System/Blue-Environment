export interface SearchFields {
  /** What the person sees as the app's name. */
  name: string;
  /** Lower-priority text: description, launch command, keywords. */
  extra?: string;
}

// Letters that Unicode normalisation does not decompose.
const FOLD: Record<string, string> = { ł: 'l', Ł: 'l', ø: 'o', Ø: 'o', đ: 'd', Đ: 'd', ß: 'ss', æ: 'ae', Æ: 'ae', œ: 'oe', Œ: 'oe' };

/** Lower-cases and strips accents: "Zażółć Gęślą" → "zazolc gesla". */
export function fold(input: string): string {
  return input
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[łŁøØđĐßæÆœŒ]/g, (c) => FOLD[c] ?? c)
    .toLowerCase();
}

export function tokenize(query: string): string[] {
  return fold(query).split(/\s+/).filter(Boolean);
}

const WORD_SPLIT = /[\s\-_.()/\\:,;]+/;

/** Score of one token against one app, `0` when it does not match at all. */
function tokenScore(token: string, name: string, nameWords: string[], extra: string): number {
  if (name.startsWith(token)) return 100;
  if (nameWords.some((w) => w.startsWith(token))) return 80;
  if (name.includes(token)) return 60;
  if (extra.includes(token)) return 30;
  return 0;
}

/** `null` when some token matches nothing (the app is filtered out). */
export function scoreApp(fields: SearchFields, tokens: string[]): number | null {
  if (tokens.length === 0) return 0;
  const name = fold(fields.name);
  const nameWords = name.split(WORD_SPLIT).filter(Boolean);
  const extra = fold(fields.extra ?? '');
  let total = 0;
  for (const t of tokens) {
    const s = tokenScore(t, name, nameWords, extra);
    if (s === 0) return null;
    total += s;
  }
  const joined = tokens.join(' ');
  if (name === joined) total += 200;
  else if (name.startsWith(joined)) total += 50;
  return total;
}

/**
 * Filters and ranks `items` for `query`. Ties keep alphabetical order, so the
 * list is stable and predictable while typing. An empty query returns the items
 * unchanged.
 */
export function searchApps<T>(items: T[], query: string, fields: (item: T) => SearchFields): T[] {
  const tokens = tokenize(query);
  if (tokens.length === 0) return items;
  const scored: { item: T; score: number; name: string }[] = [];
  for (const item of items) {
    const f = fields(item);
    const score = scoreApp(f, tokens);
    if (score !== null) scored.push({ item, score, name: f.name });
  }
  scored.sort((a, b) => b.score - a.score || a.name.localeCompare(b.name));
  return scored.map((s) => s.item);
}

/** Last path segment of a command's first word: `/usr/bin/firefox --new` → `firefox`. */
export function execBasename(exec: string | undefined): string {
  if (!exec) return '';
  const first = exec.trim().split(/\s+/)[0] ?? '';
  return first.split('/').pop() ?? '';
}
