#!/usr/bin/env node
/**
 * Regenerates:
 *   1. src-tauri/build.rs               — the `.commands(&[...])` list
 *   2. src-tauri/capabilities/default.json — one "allow-<kebab-command>"
 *      permission per custom command
 *
 * from the single source of truth: the `tauri::generate_handler![...]`
 * block in src-tauri/src/main.rs.
 *
 * WHY THIS EXISTS
 * ----------------
 * Before this script, all three lists were hand-maintained separately.
 * They drifted: `screen_time_record_usage`, `screen_time_get_summary`,
 * `screen_time_clear_history` (and a dozen others — accounts_*, bc_*,
 * bv_*, downloader_*, messages_*, list_system_themes,
 * settings_bluetooth_get_powered) were registered as real `#[tauri::command]`
 * functions and wired into `generate_handler!`, but were never added to
 * `capabilities/default.json`'s `allow-*` list. Tauri's runtime ACL
 * silently denies any IPC call to a command that isn't explicitly
 * allowed by the window's capability file — so every one of those
 * commands failed on every call, with the failure swallowed by the
 * frontend's `.catch(() => {})` call sites. This is exactly why
 * Settings > Screen Time always showed "no data": the recording calls
 * were being rejected at the IPC layer before they ever reached
 * `screen_time.rs`.
 *
 * Run this any time a command is added to, removed from, or renamed in
 * the `generate_handler!` list in main.rs:
 *
 *   node scripts/gen-tauri-allowlist.mjs
 *   # or: npm run gen:allowlist
 *
 * It is also run in CI (see .github/workflows/test.yml) in "check" mode
 * to fail the build if main.rs and the generated files have drifted —
 * i.e. if someone edits the command list but forgets to re-run this.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(__dirname, '..');
const mainRsPath = path.join(root, 'src-tauri/src/main.rs');
const buildRsPath = path.join(root, 'src-tauri/build.rs');
const capsPath = path.join(root, 'src-tauri/capabilities/default.json');

const checkOnly = process.argv.includes('--check');

// ── 1. Extract the generate_handler![...] block from main.rs ───────────
const mainRs = readFileSync(mainRsPath, 'utf8');
const startMarker = '.invoke_handler(tauri::generate_handler![';
const startIdx = mainRs.indexOf(startMarker);
if (startIdx === -1) {
  console.error('Could not find `.invoke_handler(tauri::generate_handler![` in main.rs');
  process.exit(1);
}
const bodyStart = startIdx + startMarker.length;
// Find the matching `])` that closes the macro call — the list itself
// never contains nested `[`/`]`, so a plain bracket-depth counter over
// this substring is sufficient (no need for a real Rust parser here).
let depth = 1;
let i = bodyStart;
for (; i < mainRs.length; i++) {
  if (mainRs[i] === '[') depth++;
  else if (mainRs[i] === ']') { depth--; if (depth === 0) break; }
}
const body = mainRs.slice(bodyStart, i);

// Strip `//` line comments, then split on commas.
const withoutComments = body
  .split('\n')
  .map((line) => {
    const idx = line.indexOf('//');
    return idx === -1 ? line : line.slice(0, idx);
  })
  .join('\n');

const commands = withoutComments
  .split(',')
  .map((tok) => tok.trim())
  .filter(Boolean)
  // A token may be module-qualified, e.g. `commands::backend::backend_get_info`
  // or `BlueDocs::docs_read_file` — the actual Tauri command name (and the
  // name build.rs/capabilities need) is always just the last path segment.
  .map((tok) => tok.split('::').pop())
  .filter((name) => /^[a-zA-Z_][a-zA-Z0-9_]*$/.test(name));

const uniqueCommands = [...new Set(commands)].sort();

console.log(`Found ${uniqueCommands.length} unique commands in generate_handler![...]`);

// ── 2. Regenerate build.rs's .commands(&[...]) list ─────────────────────
const buildRs = readFileSync(buildRsPath, 'utf8');
const cmdListStr = uniqueCommands.map((c) => `        "${c}",`).join('\n');
const newBuildRs = buildRs.replace(
  /(\.commands\(&\[\n)([\s\S]*?)(\n\s*\]\))/,
  (_match, open, _old, close) => `${open}${cmdListStr}${close}`
);
if (newBuildRs === buildRs && !buildRs.includes(cmdListStr)) {
  console.error('build.rs: could not find `.commands(&[ ... ])` block to replace, or content unchanged unexpectedly.');
}

// ── 3. Regenerate capabilities/default.json's allow-<command> entries ──
const caps = JSON.parse(readFileSync(capsPath, 'utf8'));
const toKebab = (name) => name.replace(/_/g, '-');
const allowCommandPermissions = uniqueCommands.map((c) => `allow-${toKebab(c)}`);

// Keep every existing permission that ISN'T one of our generated
// `allow-<command>` entries — i.e. core:*, fs:*, and everything else —
// and then append a freshly regenerated, de-duplicated, sorted block of
// `allow-<command>` entries for exactly the commands generate_handler!
// currently registers. This is what makes the script idempotent and
// safe to run after adding/removing/renaming a command: stale entries
// for removed commands are dropped, new ones are added, nothing else
// in the file is touched.
// Drop any plain "allow-xxx" string permission (our generated commands
// are always plain kebab-case strings with no ":" in them — real Tauri
// plugin permissions are always namespaced, e.g.
// "fs:allow-read-text-file"), keep everything else (namespaced plugin
// permissions, fs:scope objects, etc.) untouched.
const nonCommandPermissions = caps.permissions.filter((p) => {
  if (typeof p !== 'string') return true;
  if (p.startsWith('allow-') && !p.includes(':')) return false;
  return true;
});

caps.permissions = [...nonCommandPermissions, ...allowCommandPermissions];

const newCapsJson = JSON.stringify(caps, null, 2) + '\n';

// ── 4. Write or check ────────────────────────────────────────────────
if (checkOnly) {
  const currentCaps = readFileSync(capsPath, 'utf8');
  const drifted = newBuildRs !== buildRs || newCapsJson.trim() !== currentCaps.trim();
  if (drifted) {
    console.error(
      'DRIFT DETECTED: build.rs and/or capabilities/default.json are out of sync with ' +
      'the generate_handler![...] list in main.rs. Run `node scripts/gen-tauri-allowlist.mjs` ' +
      'and commit the result.'
    );
    process.exit(1);
  }
  console.log('OK: build.rs and capabilities/default.json match main.rs.');
  process.exit(0);
}

writeFileSync(buildRsPath, newBuildRs);
writeFileSync(capsPath, newCapsJson);
console.log('Wrote src-tauri/build.rs and src-tauri/capabilities/default.json');
