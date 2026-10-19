export type ResultKind = 'calc' | 'app' | 'setting' | 'file' | 'event' | 'mail' | 'action';

export interface ResultItem {
  kind: ResultKind;
  id: string;
  title: string;
  subtitle?: string;
  /** Opaque data the host uses to open the result. */
  payload?: unknown;
}

export interface ResultSection { kind: ResultKind; items: ResultItem[] }

/** Lower-cases and strips the diacritics of the shipped languages (same table as the Rust side). */
export function fold(s: string): string {
  const map: Record<string, string> = {
    ą: 'a', ä: 'a', á: 'a', à: 'a', â: 'a', ã: 'a', å: 'a', ć: 'c', č: 'c', ç: 'c',
    ę: 'e', é: 'e', è: 'e', ê: 'e', ë: 'e', ě: 'e', í: 'i', ì: 'i', î: 'i', ï: 'i', ı: 'i',
    ł: 'l', ń: 'n', ñ: 'n', ň: 'n', ó: 'o', ö: 'o', ò: 'o', ô: 'o', õ: 'o', ø: 'o',
    ś: 's', š: 's', ş: 's', ú: 'u', ù: 'u', û: 'u', ü: 'u', ů: 'u', ý: 'y', ÿ: 'y',
    ź: 'z', ż: 'z', ž: 'z',
  };
  let out = '';
  for (const ch of s.toLowerCase()) out += ch === 'ß' ? 'ss' : (map[ch] ?? ch);
  return out;
}

export const tokens = (q: string): string[] => fold(q).split(/\s+/).filter(Boolean);

/** null = no match; higher = better. Every token must match. */
export function scoreText(text: string, toks: string[], extra = ''): number | null {
  if (toks.length === 0) return null;
  const n = fold(text);
  const words = n.split(/[^a-z0-9]+/).filter(Boolean);
  const e = fold(extra);
  let total = 0;
  for (const t of toks) {
    if (n === t) total += 300;
    else if (n.startsWith(t)) total += 100;
    else if (words.some((w) => w.startsWith(t))) total += 80;
    else if (n.includes(t)) total += 60;
    else if (e.includes(t)) total += 15;
    else return null;
  }
  return total;
}

export function rank<T>(items: T[], query: string, text: (t: T) => string, extra: (t: T) => string = () => '', limit = 8): T[] {
  const toks = tokens(query);
  return items
    .map((it) => ({ it, s: scoreText(text(it), toks, extra(it)) }))
    .filter((x): x is { it: T; s: number } => x.s !== null)
    .sort((a, b) => b.s - a.s)
    .slice(0, limit)
    .map((x) => x.it);
}

// ── Calculator ──────────────────────────────────────────────────────────
// Recursive-descent parser: no eval(), no Function(). Supports + - * / % ^ **,
// parentheses, unary minus, constants pi/e and a few functions.

const FUNCS: Record<string, (x: number) => number> = {
  sqrt: Math.sqrt, abs: Math.abs, round: Math.round, floor: Math.floor, ceil: Math.ceil,
  sin: Math.sin, cos: Math.cos, tan: Math.tan, ln: Math.log, log: Math.log10, exp: Math.exp,
};
const CONSTS: Record<string, number> = { pi: Math.PI, e: Math.E };

export function evalMath(input: string): number | null {
  const src = input.trim().replace(/,/g, '.').replace(/×/g, '*').replace(/÷/g, '/').replace(/\*\*/g, '^');
  if (!src || src.length > 120) return null;
  // Only plain expressions: digits, operators, parentheses, identifiers.
  if (!/^[0-9a-z+\-*/%^().\s]+$/i.test(src)) return null;
  let i = 0;
  let sawOperation = false;
  const peek = () => { while (src[i] === ' ') i++; return src[i]; };
  const fail = () => { throw new Error('syntax'); };

  function number(): number {
    const m = /^\d+(\.\d+)?|^\.\d+/.exec(src.slice(i));
    if (!m) return fail() as never;
    i += m[0].length;
    return parseFloat(m[0]);
  }
  function primary(): number {
    const c = peek();
    if (c === '(') { i++; const v = expr(); if (peek() !== ')') fail(); i++; return v; }
    if (c && /[a-z]/i.test(c)) {
      const m = /^[a-z]+/i.exec(src.slice(i))!;
      const name = m[0].toLowerCase();
      i += name.length;
      if (name in CONSTS) { sawOperation = sawOperation || false; return CONSTS[name]; }
      const fn = FUNCS[name];
      if (!fn || peek() !== '(') return fail() as never;
      i++;
      const arg = expr();
      if (peek() !== ')') fail();
      i++;
      sawOperation = true;
      return fn(arg);
    }
    return number();
  }
  // Precedence (like a maths textbook): ^ binds tighter than unary minus, so -2^2 = -4,
  // and the exponent may itself be signed (2^-1 = 0.5). ^ is right-associative.
  function power(): number {
    const base = primary();
    if (peek() === '^') { i++; sawOperation = true; return Math.pow(base, unary()); }
    return base;
  }
  function unary(): number {
    const c = peek();
    if (c === '-') { i++; return -unary(); }
    if (c === '+') { i++; return unary(); }
    return power();
  }
  function term(): number {
    let v = unary();
    for (;;) {
      const c = peek();
      if (c === '*' || c === '/' || c === '%') {
        i++; sawOperation = true;
        const r = unary();
        v = c === '*' ? v * r : c === '/' ? v / r : v % r;
      } else return v;
    }
  }
  function expr(): number {
    let v = term();
    for (;;) {
      const c = peek();
      if (c === '+' || c === '-') { i++; sawOperation = true; const r = term(); v = c === '+' ? v + r : v - r; } else return v;
    }
  }
  try {
    const v = expr();
    if (peek() !== undefined) return null;
    if (!sawOperation || !Number.isFinite(v)) return null;
    return v;
  } catch {
    return null;
  }
}

export function formatNumber(n: number): string {
  if (Number.isInteger(n)) return String(n);
  return String(parseFloat(n.toPrecision(12)));
}

// ── Sections & navigation ──────────────────────────────────────────────

const ORDER: ResultKind[] = ['calc', 'app', 'setting', 'action', 'event', 'mail', 'file'];

/** Drops empty sections and orders the rest; the flat list drives ↑/↓ navigation. */
export function buildSections(parts: Partial<Record<ResultKind, ResultItem[]>>): ResultSection[] {
  return ORDER.filter((k) => (parts[k]?.length ?? 0) > 0).map((kind) => ({ kind, items: parts[kind]! }));
}

export const flatten = (sections: ResultSection[]): ResultItem[] => sections.flatMap((s) => s.items);

export function moveSelection(index: number, delta: number, length: number): number {
  if (length <= 0) return 0;
  return (((index + delta) % length) + length) % length;
}
