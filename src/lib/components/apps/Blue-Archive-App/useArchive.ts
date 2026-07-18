import { writable } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { ArchiveState } from './types';

export function createArchive() {
  const archive = writable<ArchiveState | null>(null);
  const loading = writable(false);
  const status = writable('');
  const error = writable('');

  async function openFile() {
    const path = await SystemBridge.pickFile(
      [{ name: 'Archives', extensions: ['zip', 'tar', 'gz', 'bz2', 'xz', '7z', 'rar', 'zst', 'lz4', 'tgz'] }],
      'Open Archive'
    );
    if (!path) return;

    loading.set(true);
    error.set('');
    status.set('');
    try {
      const info = await SystemBridge.invokeCommand<any>('archive_list', { path });
      archive.set({ path, name: path.split('/').pop() || path, info });
      if (info.error) error.set(info.error);
    } catch (e: any) {
      error.set(e?.message ?? String(e));
    } finally {
      loading.set(false);
    }
  }

  async function extract(current: ArchiveState | null) {
    if (!current) return;
    const dir = await SystemBridge.pickDirectory();
    if (!dir) return;
    loading.set(true);
    status.set('Extracting...');
    try {
      const msg = await SystemBridge.invokeCommand<string>('archive_extract', { path: current.path, destDir: dir });
      status.set(msg || 'Done');
    } catch (e: any) {
      error.set(e?.message ?? String(e));
    } finally {
      loading.set(false);
      setTimeout(() => status.set(''), 5000);
    }
  }

  return { archive, loading, status, error, openFile, extract };
}
