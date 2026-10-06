import { SystemBridge } from '../../../utils/systemBridge';
import { parsePlaylists, parseState, type PersistedState, type Playlist, type Track } from './musicLogic';

const DIR = 'HOME/.config/Blue-Environment/blue-music';
const STATE_FILE = `${DIR}/state.json`;
const PLAYLISTS_FILE = `${DIR}/playlists.json`;

export interface ScanResult { tracks: Track[]; missingFolders: string[]; reused: number }

export const loadState = async (): Promise<PersistedState> => parseState(await SystemBridge.readFile(STATE_FILE).catch(() => ''));
export const loadPlaylists = async (): Promise<Playlist[]> => parsePlaylists(await SystemBridge.readFile(PLAYLISTS_FILE).catch(() => ''));

/** Debounced writer: rapid changes (dragging the volume slider…) become one write. */
function debouncedWriter<T>(file: string, ms: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending: T | undefined;
  const flush = () => { if (timer) { clearTimeout(timer); timer = undefined; } if (pending !== undefined) { const v = pending; pending = undefined; SystemBridge.writeFile(file, JSON.stringify(v)).catch(() => {}); } };
  return { write(v: T) { pending = v; if (timer) clearTimeout(timer); timer = setTimeout(flush, ms); }, flush };
}
export const stateWriter = debouncedWriter<PersistedState>(STATE_FILE, 700);
export const playlistWriter = debouncedWriter<Playlist[]>(PLAYLISTS_FILE, 300);

export const scanLibrary = (folders: string[]): Promise<ScanResult> => SystemBridge.invokeCommand<ScanResult>('music_scan', { folders });

// ── Cover art (cached thumbnail files; at most a few lookups in flight) ──
const coverCache = new Map<string, Promise<string | null>>();
let active = 0;
const waiting: (() => void)[] = [];
const MAX_PARALLEL = 3;

async function slot<T>(fn: () => Promise<T>): Promise<T> {
  if (active >= MAX_PARALLEL) await new Promise<void>((r) => waiting.push(r));
  active++;
  try { return await fn(); } finally { active--; waiting.shift()?.(); }
}

/** Absolute path of the cover thumbnail for `trackPath`, or null. `key` shares one lookup per album. */
export function coverFile(trackPath: string, key: string = trackPath): Promise<string | null> {
  let p = coverCache.get(key);
  if (!p) {
    p = slot(() => SystemBridge.invokeCommand<string | null>('music_cover', { path: trackPath })).catch(() => null);
    coverCache.set(key, p);
  }
  return p;
}
export const clearCoverCache = () => coverCache.clear();
