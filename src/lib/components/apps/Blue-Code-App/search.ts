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

  return { searchTerm, searchResults, searchFiles };
}
