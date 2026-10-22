import { migrate } from 'svelte/compiler';
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = process.argv[2] || 'src';
const files = [];
(function walk(d) {
  for (const n of readdirSync(d)) {
    const p = join(d, n);
    if (statSync(p).isDirectory()) walk(p);
    else if (p.endsWith('.svelte')) files.push(p);
  }
})(root);

let changed = 0, failed = [];
for (const f of files) {
  const src = readFileSync(f, 'utf8');
  try {
    const out = migrate(src, { filename: f, use_ts: /<script[^>]*lang=["']ts["']/.test(src) });
    if (out.code !== src) { writeFileSync(f, out.code); changed++; }
  } catch (e) {
    failed.push([f, String(e.message || e).split('\n')[0]]);
  }
}
console.log(`migrated ${changed}/${files.length}`);
for (const [f, m] of failed) console.log('FAILED', f, m);
