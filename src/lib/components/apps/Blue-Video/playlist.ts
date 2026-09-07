import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';

export interface VideoItem { name: string; url: string; }

export function createPlaylist() {
  const playlist = writable<VideoItem[]>([]);
  const currentIdx = writable(0);

  async function openFiles() {
    const paths = await SystemBridge.pickFiles(
      [{ name: 'Videos', extensions: ['mp4', 'webm', 'mkv', 'avi', 'mov', 'flv', 'wmv', 'ogg', 'ts', 'm4v'] }],
      'Open Videos',
      'blue-video.open-files'
    );
    if (!paths.length) return;
    const items: VideoItem[] = paths.map((p) => ({ name: p.split('/').pop() || p, url: `file://${p}` }));
    const wasEmpty = get(playlist).length === 0;
    playlist.update((prev) => [...prev, ...items]);
    if (wasEmpty) currentIdx.set(0);
  }

  function remove(i: number) {
    const len = get(playlist).length;
    playlist.update((prev) => prev.filter((_, j) => j !== i));
    currentIdx.update((prev) => (i < prev ? prev - 1 : Math.min(prev, len - 2)));
  }

  return { playlist, currentIdx, openFiles, remove };
}

export async function openInMPV(url: string): Promise<void> {
  await SystemBridge.executeCommand(`mpv ${JSON.stringify(url)} 2>/dev/null || vlc ${JSON.stringify(url)} 2>/dev/null &`).catch(() => {});
}
