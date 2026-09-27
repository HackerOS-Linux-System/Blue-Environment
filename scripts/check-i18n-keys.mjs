#!/usr/bin/env node
/**
 * Checks that every locale in src/lib/translations/ has exactly the same
 * set of keys as english.ts (the source of truth every other locale is
 * derived from).
 *
 * WHY THIS EXISTS
 * ----------------
 * english.ts has 574 lines, german.ts 568, and the other 13 locales
 * (arabic, chinese, czech, dutch, french, italian, japanese, portuguese,
 * russian, spanish, swedish, turkish, ukrainian) are all exactly 543 —
 * a strong signal they were all forked from one shared, now-outdated
 * base and never re-synced as english.ts grew. polish.ts, at 642 lines,
 * likely has the opposite problem: extra/renamed keys that no longer
 * match english.ts either.
 *
 * A missing key isn't cosmetic: `t('some.key')` (see stores/language.ts)
 * falls back to showing the raw key string in the UI for any locale
 * missing it — e.g. a Turkish user seeing `settings.parental.locked_out`
 * printed literally instead of a translated sentence.
 *
 * Usage:
 *   node scripts/check-i18n-keys.mjs            # report + auto-fix missing keys
 *   node scripts/check-i18n-keys.mjs --check    # report only, exit 1 if any locale is missing keys (CI)
 *   node scripts/check-i18n-keys.mjs --report-only  # report only, exit 0 (no fix, no failure)
 */
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const dir = path.join(__dirname, '../src/lib/translations');

const checkOnly = process.argv.includes('--check');
const reportOnly = process.argv.includes('--report-only');

// One 'key': 'value' pair per line is this codebase's actual format
// (verified: no template literals, no multi-line values across all 17
// locale files). A translated value is single-quoted UNLESS the string
// itself contains an apostrophe that would need escaping — many do
// (English "Let's", French "l'image", Italian "l'HDR", …) — in which
// case this codebase writes it double-quoted instead, unescaped, which
// reads better than a wall of `\'`. Both forms must be matched: an
// earlier version of this regex only matched single-quoted values,
// silently treating every double-quoted line as if the key didn't
// exist at all — which caused this exact script to append duplicate,
// English-fallback copies of keys that were already correctly
// translated (just double-quoted). See git history / CHANGELOG for the
// cleanup; `decodeQuoted`/`encodeSingleQuoted` below exist so this
// can't happen again: every value is decoded to its real logical string
// regardless of which quote style it was written in, and re-encoded
// (single-quoted, properly escaped) only when this script itself needs
// to *write* a new line.
const KV_LINE = /^(\s*)'((?:[^'\\]|\\.)+)':\s*(?:'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)")\s*,\s*$/;

/** Undoes the minimal escaping either quote style needs (`\'`/`\\` for a
 * single-quoted value, `\"`/`\\` for a double-quoted one) to recover the
 * real string, independent of which quote character the source line
 * happened to use. */
function decodeQuoted(raw, quoteChar) {
  let out = '';
  for (let i = 0; i < raw.length; i++) {
    if (raw[i] === '\\' && (raw[i + 1] === quoteChar || raw[i + 1] === '\\')) {
      out += raw[i + 1];
      i++;
    } else {
      out += raw[i];
    }
  }
  return out;
}

/** The inverse of `decodeQuoted('...', "'")` — always produces a value
 * safe to place inside a NEW single-quoted string literal, regardless
 * of what the value contains (backslashes escaped first, so a
 * subsequent `'` isn't double-escaped into `\\'`). */
function encodeSingleQuoted(value) {
  return value.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
}

function parseFile(filePath) {
  const text = readFileSync(filePath, 'utf8');
  const lines = text.split('\n');
  /** @type {Map<string, {value: string, lineIdx: number}>} */
  const map = new Map();
  lines.forEach((line, idx) => {
    const m = line.match(KV_LINE);
    if (!m) return;
    const [, indent, key, singleQuoted, doubleQuoted] = m;
    const value = singleQuoted !== undefined ? decodeQuoted(singleQuoted, "'") : decodeQuoted(doubleQuoted, '"');
    map.set(key, { value, lineIdx: idx, indent });
  });
  return { text, lines, map };
}

// IMPORTANT: only real locale dictionaries belong here — NOT types.ts
// (no key/value pairs to parse) and NOT *.test.ts (a vitest spec file,
// e.g. i18n-keys.test.ts, which has its own `};`-terminated object
// literal — the `locales` lookup map — that this script's "find the
// closing brace and insert before it" auto-fix logic would happily
// mistake for a locale file's closing brace and inject 580+ bogus
// key/value lines into the middle of a test file. Learned the hard
// way: an earlier version of this filter was just `f.endsWith('.ts')
// && f !== 'types.ts'`, which matched i18n-keys.test.ts too.
const files = readdirSync(dir).filter((f) => f.endsWith('.ts') && !f.endsWith('.test.ts') && f !== 'types.ts');
if (!files.includes('english.ts')) {
  console.error('english.ts not found in', dir);
  process.exit(1);
}

const english = parseFile(path.join(dir, 'english.ts'));
const englishKeys = [...english.map.keys()];

console.log(`english.ts: ${englishKeys.length} keys (source of truth)\n`);

let anyMissing = false;
const report = [];

for (const file of files) {
  if (file === 'english.ts') continue;
  const localePath = path.join(dir, file);
  const locale = parseFile(localePath);
  const missing = englishKeys.filter((k) => !locale.map.has(k));
  const extra = [...locale.map.keys()].filter((k) => !english.map.has(k));

  report.push({ file, missingCount: missing.length, extraCount: extra.length });
  if (missing.length > 0) anyMissing = true;

  if (missing.length === 0 && extra.length === 0) {
    console.log(`✔ ${file}: in sync (${locale.map.size} keys)`);
    continue;
  }
  console.log(`✘ ${file}: ${locale.map.size} keys — ${missing.length} missing, ${extra.length} extra`);
  if (missing.length > 0) console.log(`    missing: ${missing.slice(0, 8).join(', ')}${missing.length > 8 ? `, … (+${missing.length - 8} more)` : ''}`);
  if (extra.length > 0) console.log(`    extra:   ${extra.slice(0, 8).join(', ')}${extra.length > 8 ? `, … (+${extra.length - 8} more)` : ''}`);

  if (!checkOnly && !reportOnly && missing.length > 0) {
    // Auto-fix: append every missing key, using english.ts's own value
    // as a fallback, clearly marked with a leading comment block so a
    // human translator can grep `FIXME i18n:` across the repo and know
    // exactly what still needs real translation — this makes the UI
    // show *something* correct-but-untranslated instead of a raw key,
    // while never silently passing off an English string as if it were
    // a real translation.
    const lines = locale.text.split('\n');
    // Insert right before the final closing `};`.
    let insertAt = lines.length - 1;
    while (insertAt > 0 && lines[insertAt].trim() === '') insertAt--;
    while (insertAt > 0 && lines[insertAt].trim() !== '};') insertAt--;
    if (lines[insertAt].trim() !== '};') {
      console.error(`    could not find closing "};" in ${file} — skipping auto-fix for this file`);
      continue;
    }
    const block = [
      '',
      `    // FIXME i18n: the ${missing.length} keys below are missing a real translation.`,
      '    // Auto-added by scripts/check-i18n-keys.mjs with english.ts\'s own value as a',
      '    // placeholder so the UI shows a correct English sentence instead of a raw key',
      '    // like "settings.parental.locked_out" — replace each with a real translation.',
      ...missing.map((k) => `    '${k}': '${encodeSingleQuoted(english.map.get(k).value)}',`),
    ];
    lines.splice(insertAt, 0, ...block);
    writeFileSync(localePath, lines.join('\n'));
    console.log(`    → appended ${missing.length} placeholder translations to ${file}`);
  }
}

console.log('');
if (checkOnly) {
  if (anyMissing) {
    console.error('DRIFT DETECTED: one or more locales are missing keys present in english.ts.');
    process.exit(1);
  }
  console.log('OK: every locale has every english.ts key.');
  process.exit(0);
}
