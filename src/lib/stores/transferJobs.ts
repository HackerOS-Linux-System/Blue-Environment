import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

/** Zdarzenie `fm:progress` z backendu (src-tauri/src/ExplolerApp/transfer.rs). */
export interface ProgressEvent {
  jobId: string; mode: 'copy' | 'move';
  doneBytes: number; totalBytes: number; doneFiles: number; totalFiles: number;
  current: string; bytesPerSec: number; etaSecs: number | null; paused: boolean;
}
export interface TransferJob extends ProgressEvent {
  startedAt: number;
  /** Okno pokazujemy dopiero po chwili — krótkie operacje nie mrugają dialogiem. */
  visible: boolean;
  cancelling: boolean;
}

export const SHOW_AFTER_MS = 500;
export const transferJobs = writable<TransferJob[]>([]);

export function newJobId(): string {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : `job-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function startJob(id: string, mode: 'copy' | 'move') {
  const job: TransferJob = {
    jobId: id, mode, doneBytes: 0, totalBytes: 0, doneFiles: 0, totalFiles: 0, current: '',
    bytesPerSec: 0, etaSecs: null, paused: false, startedAt: Date.now(), visible: false, cancelling: false,
  };
  transferJobs.update((l) => [...l, job]);
  setTimeout(() => transferJobs.update((l) => l.map((j) => (j.jobId === id ? { ...j, visible: true } : j))), SHOW_AFTER_MS);
}

export function updateJob(ev: ProgressEvent) {
  transferJobs.update((l) => l.map((j) => (j.jobId === ev.jobId ? { ...j, ...ev } : j)));
}

export function finishJob(id: string) {
  transferJobs.update((l) => l.filter((j) => j.jobId !== id));
}

export async function controlJob(id: string, action: 'pause' | 'resume' | 'cancel') {
  if (action === 'cancel') transferJobs.update((l) => l.map((j) => (j.jobId === id ? { ...j, cancelling: true } : j)));
  try { await invoke('fm_job_control', { jobId: id, action }); } catch { /* zadanie już się zakończyło */ }
}

export const hasJobs = () => get(transferJobs).length > 0;

// ── formatowanie (czyste, testowane) ──
export function fmtBytes(b: number): string {
  if (b >= 1e9) return `${(b / 1e9).toFixed(1)} GB`;
  if (b >= 1e6) return `${(b / 1e6).toFixed(1)} MB`;
  if (b >= 1e3) return `${Math.round(b / 1e3)} kB`;
  return `${Math.round(b)} B`;
}
export function fmtEta(secs: number | null): string {
  if (secs == null || !Number.isFinite(secs)) return '—';
  const h = Math.floor(secs / 3600), m = Math.floor((secs % 3600) / 60), s = Math.floor(secs % 60);
  return h ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}` : `${m}:${String(s).padStart(2, '0')}`;
}
/** Postęp 0–100: po bajtach, a gdy rozmiar nieznany (same puste pliki) — po plikach. */
export function percent(j: Pick<ProgressEvent, 'doneBytes' | 'totalBytes' | 'doneFiles' | 'totalFiles'>): number {
  if (j.totalBytes > 0) return Math.min(100, (j.doneBytes / j.totalBytes) * 100);
  if (j.totalFiles > 0) return Math.min(100, (j.doneFiles / j.totalFiles) * 100);
  return 0;
}
