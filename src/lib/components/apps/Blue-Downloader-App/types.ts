export type DownloadStatus =
  | { state: 'queued' }
  | { state: 'downloading' }
  | { state: 'paused'; resumable: boolean }
  | { state: 'completed' }
  | { state: 'failed'; error: string }
  | { state: 'cancelled' };

export interface DownloadItem {
  id: string;
  url: string;
  filename: string;
  destinationPath: string;
  totalBytes: number | null;
  downloadedBytes: number;
  status: DownloadStatus;
  createdAt: string;
  speedBytesPerSec?: number;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}

export function formatSpeed(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`;
}

export function progressFraction(item: DownloadItem): number | null {
  if (item.totalBytes === null || item.totalBytes === 0) return null;
  return Math.min(1, item.downloadedBytes / item.totalBytes);
}
