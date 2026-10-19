export type MwPhase = 'download' | 'hash' | 'write' | 'flush' | 'verify' | 'restore' | 'done';

export interface MwProgress {
  phase: MwPhase;
  done: number;
  total: number | null;
  speed: number;
}

export interface DriveInfo {
  path: string;
  model: string;
  vendor: string;
  sizeBytes: number;
  transport: string;
  mounted: string[];
  labels: string[];
}

export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  category: string;
  arch: string;
  url?: string | null;
  pageUrl?: string | null;
  checksumUrl?: string | null;
}

export interface ImageInfo {
  path: string;
  name: string;
  sizeBytes: number;
  compression: 'none' | 'xz' | 'gzip' | 'zstd' | 'bzip2';
  uncompressedBytes: number | null;
  supported: boolean;
}

export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return '–';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return i === 0 ? `${Math.round(v)} B` : `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function formatSpeed(bytesPerSec: number): string {
  return bytesPerSec > 0 ? `${formatBytes(bytesPerSec)}/s` : '';
}

/** `m:ss` / `h:mm:ss`, or empty when it can't be estimated. */
export function formatEta(p: MwProgress): string {
  if (!p.total || p.speed <= 0 || p.done >= p.total) return '';
  const s = Math.max(0, Math.round((p.total - p.done) / p.speed));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (x: number) => String(x).padStart(2, '0');
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${m}:${pad(sec)}`;
}

/** 0–100, or null when the total is unknown (indeterminate bar). */
export function percent(p: MwProgress): number | null {
  if (!p.total || p.total <= 0) return null;
  return Math.max(0, Math.min(100, (p.done / p.total) * 100));
}

export function driveTitle(d: DriveInfo): string {
  const name = [d.vendor, d.model].filter(Boolean).join(' ').trim();
  return name || d.path;
}

/** Only https is accepted by the backend; checking early gives a clear message. */
export function validateImageUrl(raw: string): 'ok' | 'empty' | 'not_https' | 'invalid' {
  const v = raw.trim();
  if (!v) return 'empty';
  try {
    const u = new URL(v);
    if (u.protocol !== 'https:') return 'not_https';
    return u.hostname ? 'ok' : 'invalid';
  } catch {
    return 'invalid';
  }
}

export function isSha256(raw: string): boolean {
  const first = raw.trim().split(/\s+/)[0]?.replace(/^sha256:/i, '').replace(/^\*/, '') ?? '';
  return /^[0-9a-fA-F]{64}$/.test(first);
}

/** What the user must tick/confirm before the writer may start. */
export function canStartWrite(opts: { image: string | null; drive: DriveInfo | null; imageBytes: number | null; confirmed: boolean }): { ok: boolean; reason?: 'no_image' | 'no_drive' | 'too_big' | 'unconfirmed' } {
  if (!opts.image) return { ok: false, reason: 'no_image' };
  if (!opts.drive) return { ok: false, reason: 'no_drive' };
  if (opts.imageBytes != null && opts.imageBytes > opts.drive.sizeBytes) return { ok: false, reason: 'too_big' };
  if (!opts.confirmed) return { ok: false, reason: 'unconfirmed' };
  return { ok: true };
}
