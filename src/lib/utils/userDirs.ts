import { derived, get, writable } from 'svelte/store';
import { SystemBridge } from './systemBridge';
import userDirNamesData from '../data/userDirNames.json';

/**
 * Localized XDG user folders (Pulpit, Dokumenty, Pobrane, …).
 *
 * The file manager, the file picker and the "Save as" dialogs used to
 * hard-code `HOME/Downloads`, `HOME/Documents`… — on a system installed in
 * Polish (where the installer creates `~/Pobrane`) none of them matched. The
 * real answer comes from the backend (`get_user_dirs` in `src-tauri/src/
 * user_dirs.rs`): it reads `~/.config/user-dirs.dirs`, falls back to
 * recognising the localized folder names of every installer language, and
 * finally to the English defaults.
 *
 * Until the first answer arrives (and always outside Tauri — plain browser
 * dev) the store holds the English defaults, so nothing ever renders empty.
 */

export type UserDirKey =
  | 'desktop' | 'documents' | 'downloads' | 'music' | 'pictures' | 'videos' | 'templates' | 'public';

export interface UserDir {
  key: UserDirKey;
  /** Absolute path. */
  path: string;
  /** `HOME/<relative>` when inside the home folder, otherwise `null`. */
  homePath: string | null;
  /** Folder name to show (`Pobrane`). */
  name: string;
  exists: boolean;
}

/** Shape sent by `user_dirs.rs` (serde, snake_case). */
interface RawUserDir { key: string; path: string; home_path: string | null; name: string; exists: boolean }

// English names come from the same JSON the installer and the backend use.
const ENGLISH = (userDirNamesData as { lang: string; names: Record<UserDirKey, string> }[])
  .find((e) => e.lang === 'en')!.names;

const KEYS = Object.keys(ENGLISH) as UserDirKey[];

export function defaultUserDirs(): UserDir[] {
  return KEYS.map((key) => ({
    key, path: `HOME/${ENGLISH[key]}`, homePath: `HOME/${ENGLISH[key]}`, name: ENGLISH[key], exists: true,
  }));
}

/** Normalizes the backend answer; unknown keys are dropped, missing ones filled with English defaults. */
export function normalizeUserDirs(raw: unknown): UserDir[] {
  const list = Array.isArray(raw) ? (raw as RawUserDir[]) : [];
  const byKey = new Map<string, RawUserDir>();
  for (const r of list) {
    if (r && typeof r.key === 'string' && typeof r.path === 'string') byKey.set(r.key, r);
  }
  return defaultUserDirs().map((fallback) => {
    const r = byKey.get(fallback.key);
    if (!r) return fallback;
    return {
      key: fallback.key,
      path: r.path,
      homePath: typeof r.home_path === 'string' ? r.home_path : null,
      name: r.name || fallback.name,
      exists: r.exists !== false,
    };
  });
}

export const userDirs = writable<UserDir[]>(defaultUserDirs());

let loading: Promise<void> | null = null;
let liveListener: Promise<void> | null = null;

/**
 * Keeps the store in sync with the disk: the backend (`user_dirs::start_watcher`)
 * emits `user-dirs:changed` whenever user-dirs.dirs is rewritten (language
 * switched, `xdg-user-dirs-update`), or a folder is renamed / created / removed
 * — e.g. by the folder migration in Settings → Language. Outside Tauri this is
 * a no-op.
 */
function listenForChanges(): Promise<void> {
  if (!liveListener) {
    liveListener = import('@tauri-apps/api/event')
      .then((mod) => mod.listen('user-dirs:changed', (e: { payload: unknown }) => { userDirs.set(normalizeUserDirs(e.payload)); }))
      .then(() => undefined)
      .catch(() => undefined);
  }
  return liveListener;
}

/** Loads (once) the real folders from the backend and starts following changes. Safe to call from many components. */
export function ensureUserDirsLoaded(force = false): Promise<void> {
  void listenForChanges();
  if (loading && !force) return loading;
  loading = SystemBridge.invokeCommand<RawUserDir[]>('get_user_dirs')
    .then((raw) => { userDirs.set(normalizeUserDirs(raw)); })
    .catch(() => { /* not in Tauri / command unavailable → keep the English defaults */ });
  return loading;
}

/** Path usable with every file command (`HOME/Pobrane` or an absolute path). */
export function userDirPath(key: UserDirKey, dirs: UserDir[] = get(userDirs)): string {
  const d = dirs.find((x) => x.key === key);
  return d ? (d.homePath ?? d.path) : `HOME/${ENGLISH[key]}`;
}

/** `~/Pobrane` form used by the "Save as" shortcut chips. */
export function userDirTilde(key: UserDirKey, dirs: UserDir[] = get(userDirs)): string {
  return userDirPath(key, dirs).replace(/^HOME(?=\/|$)/, '~');
}

export interface Place { name: string; path: string; iconName: string }

const PLACE_ICONS: Partial<Record<UserDirKey, string>> = {
  desktop: 'Folder', documents: 'FileText', downloads: 'Download',
  pictures: 'Image', music: 'Music', videos: 'Video',
};

/**
 * Sidebar "Places" for the file manager / file picker: Home first, then the
 * user's folders that actually exist, named exactly as they are on disk.
 * `homeLabel` is the (translated) label for the home entry.
 */
export function buildPlaces(dirs: UserDir[], homeLabel = 'Home'): Place[] {
  const places: Place[] = [{ name: homeLabel, path: 'HOME', iconName: 'Home' }];
  for (const key of ['desktop', 'documents', 'downloads', 'pictures', 'music', 'videos'] as UserDirKey[]) {
    const d = dirs.find((x) => x.key === key);
    if (!d || !d.exists) continue;
    places.push({ name: d.name, path: d.homePath ?? d.path, iconName: PLACE_ICONS[key] ?? 'Folder' });
  }
  return places;
}

/** Chips for the Save-as / Create-archive dialogs: Documents, Desktop, Downloads. */
export const savePlaces = derived(userDirs, ($d) =>
  (['documents', 'desktop', 'downloads'] as UserDirKey[]).map((k) => userDirTilde(k, $d)),
);
