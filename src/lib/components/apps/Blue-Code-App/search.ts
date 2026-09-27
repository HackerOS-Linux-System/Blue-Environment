import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { SearchResult } from './types';
import { SEARCHABLE_EXT_RE } from './languageMap';

const SKIP_DIRS = new Set(['node_modules', '.git', 'target', 'dist', 'build', '__pycache__', '.next', 'vendor']);

export function createSearch(rootPathStore: { subscribe: (fn: (v: string) => void) => () => void }) {
  const searchTerm = writable('');
  const searchResults = writable<SearchResult[]>([]);

  let rootPath = '';
  rootPathStore.subscribe((v) => (rootPath = v));

  async function searchFiles() {
    const term = get(searchTerm).trim().toLowerCase();
    if (!term || !rootPath) return;
    const results: SearchResult[] = [];

  async function searchDir(dir: string) {
    if (results.length >= 50) return;
    const files = await SystemBridge.getFiles(dir);
    for (const file of files) {
      if (results.length >= 50) return;
      if (file.is_dir) {
        if (SKIP_DIRS.has(file.name)) continue;
        await searchDir(file.path);
        continue;
      }
      if (!file.mime_type?.startsWith('text/') && !SEARCHABLE_EXT_RE.test(file.name)) continue;
      const content = await SystemBridge.readFile(file.path).catch(() => '');
      const lines = content.split('\n');
      for (let i = 0; i < lines.length; i++) {
        if (lines[i].toLowerCase().includes(term)) {
          results.push({ file: file.path, line: i + 1, content: lines[i].trim() });
          if (results.length >= 50) break;
        }
      }
    }
  }

    await searchDir(rootPath);
    searchResults.set(results);
  }

  /** Replace-across-files, the missing half of "search in files" — a
   * plain, case-insensitive, non-regex substring replace (matching
   * `searchFiles`'s own matching rules exactly, so "what you searched is
   * what gets replaced" with no surprises from an accidental regex
   * metacharacter in someone's search term). Runs over the *current*
   * `searchResults` file set (so the person reviews matches before
   * committing to a replace, rather than this doing a fresh blind
   * filesystem walk of its own), applying the given case-preserving
   * substring replace to every matching line in each file, then writing
   * the file back via the same `SystemBridge.writeFile` path a normal
   * editor save uses. Returns how many files/lines actually changed, so
   * the Sidebar can show "Replaced 12 occurrences in 4 files." rather
   * than a bare "Done".
   */
  async function replaceAll(replacement: string): Promise<{ files: number; occurrences: number }> {
    const term = get(searchTerm);
    const results = get(searchResults);
    if (!term) return { files: 0, occurrences: 0 };

    const filesToTouch = [...new Set(results.map((r) => r.file))];
    let filesChanged = 0;
    let occurrences = 0;

    for (const file of filesToTouch) {
      const content = await SystemBridge.readFile(file).catch(() => null);
      if (content === null) continue;
      // Case-insensitive global replace — a hand-rolled split/join
      // rather than `new RegExp(term, 'gi')`, so a search term
      // containing regex metacharacters (very common in code: `.`,
      // `(`, `[`, `$`) is treated as the literal substring it looked
      // like in the search results, not reinterpreted as a pattern.
      const lowerContent = content.toLowerCase();
      const lowerTerm = term.toLowerCase();
      if (!lowerContent.includes(lowerTerm)) continue;

      let result = '';
      let cursor = 0;
      let fileOccurrences = 0;
      let idx = lowerContent.indexOf(lowerTerm, cursor);
      while (idx !== -1) {
        result += content.slice(cursor, idx) + replacement;
        cursor = idx + term.length;
        fileOccurrences++;
        idx = lowerContent.indexOf(lowerTerm, cursor);
      }
      result += content.slice(cursor);

      if (fileOccurrences > 0) {
        await SystemBridge.writeFile(file, result);
        filesChanged++;
        occurrences += fileOccurrences;
      }
    }

    if (occurrences > 0) await searchFiles(); // refresh results to reflect the now-applied replacement
    return { files: filesChanged, occurrences };
  }

  return { searchTerm, searchResults, searchFiles, replaceAll };
}
