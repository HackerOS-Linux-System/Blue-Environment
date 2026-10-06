export interface Track {
  path: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  /** 0 = unknown */
  trackNo: number;
  discNo: number;
  year: number;
  genre: string;
  durationSecs: number;
}

export interface AlbumGroup { key: string; album: string; artist: string; year: number; tracks: Track[]; duration: number }
export interface ArtistGroup { key: string; name: string; albums: AlbumGroup[]; trackCount: number }

export type RepeatMode = 'off' | 'all' | 'one';

/** Lower-case, accent-insensitive form used for searching, sorting and grouping keys. */
export function norm(s: string): string {
  return s
    .normalize('NFD').replace(/[\u0300-\u036f]/g, '')   // ą → a, é → e …
    .replace(/ł/g, 'l').replace(/Ł/g, 'L').replace(/ø/g, 'o').replace(/Ø/g, 'O').replace(/đ/g, 'd').replace(/Đ/g, 'D')
    .toLowerCase().trim();
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
const cmp = (a: string, b: string) => collator.compare(a, b);

/** Artist used to file a track under (album artist wins, so compilations stay together). */
export const filingArtist = (t: Track): string => t.albumArtist || t.artist;

/** Library order: artist → album → disc → track number → title. Unknown artist/album last. */
export function compareTracks(a: Track, b: Track): number {
  const aa = filingArtist(a), ba = filingArtist(b);
  if (!!aa !== !!ba) return aa ? -1 : 1;
  return cmp(aa, ba) || (!!a.album !== !!b.album ? (a.album ? -1 : 1) : 0) || cmp(a.album, b.album)
    || (a.discNo || 1) - (b.discNo || 1)
    || (a.trackNo || 9999) - (b.trackNo || 9999)
    || cmp(a.title, b.title) || cmp(a.path, b.path);
}
export const sortTracks = (ts: Track[]): Track[] => [...ts].sort(compareTracks);

const albumKey = (t: Track) => `${norm(filingArtist(t))}|${norm(t.album)}`;

export function groupAlbums(tracks: Track[]): AlbumGroup[] {
  const map = new Map<string, AlbumGroup>();
  for (const t of sortTracks(tracks)) {
    const key = albumKey(t);
    let g = map.get(key);
    if (!g) { g = { key, album: t.album, artist: filingArtist(t), year: t.year, tracks: [], duration: 0 }; map.set(key, g); }
    g.tracks.push(t); g.duration += t.durationSecs;
    if (!g.year && t.year) g.year = t.year;
  }
  return [...map.values()].sort((a, b) => (!!a.artist !== !!b.artist ? (a.artist ? -1 : 1) : 0) || cmp(a.artist, b.artist) || (a.year || 9999) - (b.year || 9999) || cmp(a.album, b.album));
}

export function groupArtists(tracks: Track[]): ArtistGroup[] {
  const map = new Map<string, ArtistGroup>();
  for (const album of groupAlbums(tracks)) {
    const key = norm(album.artist);
    let g = map.get(key);
    if (!g) { g = { key, name: album.artist, albums: [], trackCount: 0 }; map.set(key, g); }
    g.albums.push(album); g.trackCount += album.tracks.length;
  }
  return [...map.values()].sort((a, b) => (!!a.name !== !!b.name ? (a.name ? -1 : 1) : 0) || cmp(a.name, b.name));
}

/** Every whitespace-separated word of `query` must occur in title / artist / album / genre. */
export function searchTracks(tracks: Track[], query: string): Track[] {
  const words = norm(query).split(/\s+/).filter(Boolean);
  if (!words.length) return tracks;
  return tracks.filter((t) => {
    const hay = norm(`${t.title} ${t.artist} ${t.albumArtist} ${t.album} ${t.genre}`);
    return words.every((w) => hay.includes(w));
  });
}

export function fmtTime(s: number): string {
  if (!isFinite(s) || s < 0) return '0:00';
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = Math.floor(s % 60);
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}` : `${m}:${String(sec).padStart(2, '0')}`;
}

export const totalDuration = (ts: Track[]): number => ts.reduce((n, t) => n + (t.durationSecs || 0), 0);

// ── Queue ────────────────────────────────────────────────────────────────

export interface QueueState {
  /** Play order (already shuffled when `shuffle` is on). */
  items: string[];
  /** The order before shuffling — restored when shuffle is switched off. */
  original: string[];
  pos: number;
  shuffle: boolean;
  repeat: RepeatMode;
}

export type Rng = () => number;

/** Fisher–Yates; `rng` is injectable so tests are deterministic. */
export function shuffled<T>(arr: T[], rng: Rng = Math.random): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(rng() * (i + 1)); [a[i], a[j]] = [a[j], a[i]]; }
  return a;
}

export const emptyQueue = (shuffle = false, repeat: RepeatMode = 'off'): QueueState => ({ items: [], original: [], pos: -1, shuffle, repeat });

/** New queue from `paths`, starting at `startPath` (or the first). With shuffle the start track stays first. */
export function makeQueue(paths: string[], startPath: string | null, shuffle: boolean, repeat: RepeatMode, rng: Rng = Math.random): QueueState {
  const original = [...paths];
  if (!original.length) return emptyQueue(shuffle, repeat);
  const start = startPath && original.includes(startPath) ? startPath : original[0];
  if (!shuffle) return { items: original, original, pos: original.indexOf(start), shuffle, repeat };
  const rest = shuffled(original.filter((p) => p !== start), rng);
  return { items: [start, ...rest], original, pos: 0, shuffle, repeat };
}

export const currentPath = (q: QueueState): string | null => (q.pos >= 0 && q.pos < q.items.length ? q.items[q.pos] : null);

/**
 * Position to play after the current one, or `null` when playback should stop.
 * `auto` = the track ended on its own ("repeat one" replays it); a manual Next always advances.
 */
export function nextPos(q: QueueState, auto: boolean): number | null {
  if (!q.items.length) return null;
  if (auto && q.repeat === 'one') return q.pos;
  if (q.pos + 1 < q.items.length) return q.pos + 1;
  return q.repeat === 'off' && auto ? null : q.repeat === 'off' ? q.pos : 0; // manual Next at the end of a non-repeating queue stays put
}

export function prevPos(q: QueueState): number {
  if (!q.items.length) return -1;
  if (q.pos > 0) return q.pos - 1;
  return q.repeat === 'all' ? q.items.length - 1 : 0;
}

/** Turns shuffle on/off without interrupting the current track. */
export function setShuffle(q: QueueState, on: boolean, rng: Rng = Math.random): QueueState {
  if (on === q.shuffle) return q;
  const cur = currentPath(q);
  if (!on) {
    const original = q.original.length === q.items.length ? q.original : q.items;
    return { ...q, shuffle: false, items: [...original], pos: cur ? Math.max(0, original.indexOf(cur)) : -1 };
  }
  const rest = shuffled(q.items.filter((_, i) => i !== q.pos), rng);
  return { ...q, shuffle: true, original: [...q.items], items: cur ? [cur, ...rest] : rest, pos: cur ? 0 : -1 };
}

export const cycleRepeat = (r: RepeatMode): RepeatMode => (r === 'off' ? 'all' : r === 'all' ? 'one' : 'off');

/** Inserts right after the current track ("Play next"). */
export function enqueueNext(q: QueueState, path: string): QueueState {
  const at = q.pos + 1;
  const items = [...q.items]; items.splice(at, 0, path);
  const original = [...q.original]; original.splice(Math.min(original.length, Math.max(0, original.indexOf(currentPath(q) ?? '') + 1)), 0, path);
  return { ...q, items, original: q.shuffle ? original : items };
}

export function enqueueEnd(q: QueueState, paths: string[]): QueueState {
  const items = [...q.items, ...paths];
  return { ...q, items, original: q.shuffle ? [...q.original, ...paths] : items, pos: q.pos < 0 && items.length ? 0 : q.pos };
}

/** Removes the item at `index`; the current position follows the same track (or the next one if it was removed). */
export function removeAt(q: QueueState, index: number): QueueState {
  if (index < 0 || index >= q.items.length) return q;
  const removed = q.items[index];
  const items = q.items.filter((_, i) => i !== index);
  const oi = q.original.indexOf(removed);
  const original = q.shuffle && oi >= 0 ? q.original.filter((_, i) => i !== oi) : items;
  const pos = index < q.pos ? q.pos - 1 : index === q.pos ? Math.min(q.pos, items.length - 1) : q.pos;
  return { ...q, items, original, pos: items.length ? pos : -1 };
}

/** Drops every path that is no longer in the library (files deleted / folder removed). */
export function pruneQueue(q: QueueState, exists: (path: string) => boolean): QueueState {
  const cur = currentPath(q);
  const items = q.items.filter(exists);
  const original = q.original.filter(exists);
  const pos = cur && items.includes(cur) ? items.indexOf(cur) : items.length ? Math.min(Math.max(q.pos, 0), items.length - 1) : -1;
  return { ...q, items, original: q.shuffle ? original : items, pos };
}

// ── Persistence (parsed defensively — the file may be hand-edited or from an older version) ──

export interface PersistedState {
  folders: string[];
  volume: number;
  muted: boolean;
  shuffle: boolean;
  repeat: RepeatMode;
  queue: string[];
  pos: number;
  positionSecs: number;
}
export const DEFAULT_FOLDERS = ['HOME/Music'];
export const defaultState = (): PersistedState => ({ folders: [...DEFAULT_FOLDERS], volume: 0.8, muted: false, shuffle: false, repeat: 'off', queue: [], pos: -1, positionSecs: 0 });

const isStr = (v: unknown): v is string => typeof v === 'string' && v.length > 0 && v.length < 4096;
const clamp = (n: unknown, lo: number, hi: number, d: number) => (typeof n === 'number' && isFinite(n) ? Math.min(hi, Math.max(lo, n)) : d);

export function parseState(raw: string): PersistedState {
  const d = defaultState();
  try {
    const v = JSON.parse(raw);
    if (!v || typeof v !== 'object') return d;
    const folders = Array.isArray(v.folders) ? [...new Set((v.folders as unknown[]).filter(isStr))] : d.folders;
    const queue = Array.isArray(v.queue) ? (v.queue as unknown[]).filter(isStr).slice(0, 50_000) : [];
    return {
      folders: folders.length ? folders : d.folders,
      volume: clamp(v.volume, 0, 1, d.volume),
      muted: v.muted === true,
      shuffle: v.shuffle === true,
      repeat: v.repeat === 'all' || v.repeat === 'one' ? v.repeat : 'off',
      queue,
      pos: Math.floor(clamp(v.pos, -1, queue.length - 1, -1)),
      positionSecs: clamp(v.positionSecs, 0, 1e7, 0),
    };
  } catch { return d; }
}

export interface Playlist { id: string; name: string; tracks: string[] }

export function parsePlaylists(raw: string): Playlist[] {
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v)) return [];
    const seen = new Set<string>();
    const out: Playlist[] = [];
    for (const p of v) {
      if (!p || !isStr(p.id) || !isStr(p.name) || seen.has(p.id)) continue;
      seen.add(p.id);
      out.push({ id: p.id, name: p.name.slice(0, 120), tracks: Array.isArray(p.tracks) ? p.tracks.filter(isStr) : [] });
    }
    return out;
  } catch { return []; }
}

export const newPlaylistId = (): string => `pl-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;

export function addToPlaylist(pl: Playlist, paths: string[], allowDuplicates = false): Playlist {
  const have = new Set(pl.tracks);
  const add = paths.filter((p) => allowDuplicates || !have.has(p));
  return add.length ? { ...pl, tracks: [...pl.tracks, ...add] } : pl;
}
export const removeFromPlaylist = (pl: Playlist, index: number): Playlist => ({ ...pl, tracks: pl.tracks.filter((_, i) => i !== index) });
/** Playlist entries whose file is gone are skipped when shown/played (but kept, in case the drive is just unmounted). */
export const resolvePlaylist = (pl: Playlist, byPath: Map<string, Track>): Track[] => pl.tracks.map((p) => byPath.get(p)).filter((t): t is Track => !!t);

// ── MPRIS mapping ────────────────────────────────────────────────────────

export const repeatToMpris = (r: RepeatMode): 'None' | 'Playlist' | 'Track' => (r === 'one' ? 'Track' : r === 'all' ? 'Playlist' : 'None');
export const repeatFromMpris = (s: string): RepeatMode => (s === 'Track' ? 'one' : s === 'Playlist' ? 'all' : 'off');
