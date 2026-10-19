export interface Snapshot { name: string; created: string; transferredBytes: number; warnings: number }
export interface BackupConfig {
  destination: string; sources: string[]; excludes: string[];
  keepDaily: number; keepWeekly: number; keepMonthly: number;
  schedule: 'off' | 'hourly' | 'daily' | 'weekly';
}
export interface BackupEntry { name: string; isDir: boolean; size: number; modified: number }

/** `snap-20261016-093000` → Date (local), or null. */
export function parseSnapshotName(name: string): Date | null {
  const m = /^snap-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$/.exec(name);
  if (!m) return null;
  const [, y, mo, d, h, mi, s] = m.map(Number);
  const dt = new Date(y, mo - 1, d, h, mi, s);
  return dt.getFullYear() === y && dt.getMonth() === mo - 1 && dt.getDate() === d ? dt : null;
}

export function joinRel(base: string, name: string): string {
  return [base, name].filter(Boolean).join('/').replace(/\/+/g, '/');
}

export function breadcrumbs(rel: string): { label: string; path: string }[] {
  const parts = rel.split('/').filter(Boolean);
  return parts.map((p, i) => ({ label: p, path: parts.slice(0, i + 1).join('/') }));
}

export function parentRel(rel: string): string {
  const parts = rel.split('/').filter(Boolean);
  parts.pop();
  return parts.join('/');
}

/** Absolute path a snapshot-relative path had on the live system. */
export function originalPath(rel: string): string {
  return '/' + rel.replace(/^\/+/, '');
}

export function isInside(child: string, parent: string): boolean {
  const c = child.replace(/\/+$/, '');
  const p = parent.replace(/\/+$/, '');
  return c === p || c.startsWith(p + '/');
}

/** Mirrors the backend's rule: the destination must not live inside a source. */
export function destinationProblem(cfg: Pick<BackupConfig, 'destination' | 'sources'>): 'no_destination' | 'no_sources' | 'inside_source' | null {
  if (!cfg.destination.trim()) return 'no_destination';
  if (cfg.sources.length === 0) return 'no_sources';
  if (cfg.sources.some((s) => isInside(cfg.destination, s))) return 'inside_source';
  return null;
}

export function toggleSource(sources: string[], path: string): string[] {
  return sources.includes(path) ? sources.filter((s) => s !== path) : [...sources, path];
}
