<script context="module" lang="ts">
  // Plain store module — same ".svelte-file-as-module" convention this
  // codebase already uses for Blue Messages' messagesStore.svelte
  // (wrapped in `context="module"` so it behaves as an ordinary
  // importable TS module rather than a component).
  import { writable, get } from 'svelte/store';
  import { SystemBridge } from '../../../utils/systemBridge';
  import type { DownloadItem } from './types';

  export function createDownloaderStore() {
    const downloads = writable<DownloadItem[]>([]);
    const error = writable<string | null>(null);
    let unlisten: (() => void) | null = null;

    async function load() {
      downloads.set(await SystemBridge.downloaderList());
    }

    /** Subscribes to `blue-downloader://progress` so every download's
     * card updates live (bytes/speed/status) without polling — see
     * BlueDownloaderApp/mod.rs's `run_download` for what emits this and
     * how often. */
    async function subscribe() {
      if (!SystemBridge.isTauri() || unlisten) return;
      try {
        const mod = await import('@tauri-apps/api/event');
        unlisten = await mod.listen('blue-downloader://progress', (e: any) => {
          const updated = e.payload as DownloadItem;
          if (!updated?.id) return;
          downloads.update((list) => {
            const idx = list.findIndex((d) => d.id === updated.id);
            if (idx === -1) return [updated, ...list];
            const next = [...list];
            next[idx] = updated;
            return next;
          });
        });
      } catch {
        /* not running under Tauri — the list just won't update live */
      }
    }

    function unsubscribe() {
      unlisten?.();
      unlisten = null;
    }

    async function add(url: string, destinationDir?: string) {
      error.set(null);
      const trimmed = url.trim();
      if (!trimmed) return;
      const res = await SystemBridge.downloaderAdd(trimmed, destinationDir);
      if (!res.ok) {
        error.set(res.error ?? 'Failed to start download');
        return;
      }
      if (res.item) downloads.update((list) => [res.item!, ...list]);
    }

    async function pause(id: string) {
      const res = await SystemBridge.downloaderPause(id);
      if (!res.ok) error.set(res.error ?? 'Failed to pause');
    }
    async function resume(id: string) {
      const res = await SystemBridge.downloaderResume(id);
      if (!res.ok) error.set(res.error ?? 'Failed to resume');
    }
    async function cancel(id: string) {
      const res = await SystemBridge.downloaderCancel(id);
      if (!res.ok) { error.set(res.error ?? 'Failed to cancel'); return; }
      downloads.update((list) => list.map((d) => (d.id === id ? { ...d, status: { state: 'cancelled' } } : d)));
    }
    async function remove(id: string) {
      const res = await SystemBridge.downloaderRemove(id);
      if (!res.ok) { error.set(res.error ?? 'Failed to remove'); return; }
      downloads.update((list) => list.filter((d) => d.id !== id));
    }

    return { downloads, error, load, subscribe, unsubscribe, add, pause, resume, cancel, remove };
  }

  export type DownloaderStore = ReturnType<typeof createDownloaderStore>;
</script>
