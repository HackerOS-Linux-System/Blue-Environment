export interface LauncherApp {
  id: string;
  title: string;
  /** A Svelte (lucide) component constructor or an image URL — see `AppDefinition.icon`. */
  icon: any;
}

/** A running app as shown on the phone's recent-apps screen. */
export interface RecentItem { id: string; title: string; icon: any; active: boolean }

interface AppLike { id: string; title: string; icon: any }

/**
 * Launchable apps, alphabetical. An app is hidden when the person switched it
 * off (`appsEnabled[id] === false`, Settings → Apps) or when it is listed in
 * `exclude` (e.g. the shell's own settings entry on a kiosk-like TV).
 */
export function listLauncherApps(
  registry: Record<string, AppLike>,
  appsEnabled: Record<string, boolean> = {},
  exclude: readonly string[] = [],
): LauncherApp[] {
  return Object.values(registry)
    .filter((a) => a && a.id && a.title && appsEnabled[a.id] !== false && !exclude.includes(a.id))
    .map((a) => ({ id: String(a.id), title: a.title, icon: a.icon }))
    .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }));
}

/** Case/diacritics-insensitive "contains" filter (`zolw` finds `Żółw`). */
export function filterApps<T extends { title: string }>(apps: T[], query: string): T[] {
  const q = fold(query.trim());
  if (!q) return apps;
  return apps.filter((a) => fold(a.title).includes(q));
}

// Letters that Unicode does NOT decompose into base + accent (so NFD alone
// leaves them untouched): Polish ł, Croatian đ, Nordic ø/æ, German ß, œ,
// Turkish dotless ı. Without this, typing "zolw" would not find "Żółw".
const NON_DECOMPOSING: Record<string, string> = { ł: 'l', đ: 'd', ø: 'o', ß: 'ss', æ: 'ae', œ: 'oe', ı: 'i' };

function fold(s: string): string {
  return s
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[łđøßæœı]/g, (c) => NON_DECOMPOSING[c] ?? c);
}

/** Splits a list into pages of `size` (the last page may be shorter). `size < 1` → one page. */
export function paginate<T>(items: T[], size: number): T[][] {
  if (!Number.isFinite(size) || size < 1) return items.length ? [items] : [];
  const pages: T[][] = [];
  for (let i = 0; i < items.length; i += size) pages.push(items.slice(i, i + size));
  return pages;
}

/** Pinned apps that still exist in the registry (and are enabled), in the pinned order. */
export function resolvePinned(pinned: readonly string[] | undefined, apps: LauncherApp[], max: number): LauncherApp[] {
  if (!pinned?.length) return apps.slice(0, max);
  const byId = new Map(apps.map((a) => [a.id, a] as const));
  const out: LauncherApp[] = [];
  for (const id of pinned) {
    const a = byId.get(id);
    if (a && !out.includes(a)) out.push(a);
    if (out.length >= max) break;
  }
  return out.length ? out : apps.slice(0, max);
}
