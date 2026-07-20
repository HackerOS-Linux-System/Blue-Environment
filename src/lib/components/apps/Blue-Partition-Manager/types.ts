export interface BpmDevice {
  name: string;
  path: string;
  kind: string; // "disk" | "part" | "loop" | "rom" | ...
  size_bytes: number;
  fstype: string | null;
  label: string | null;
  mountpoint: string | null;
  model: string | null;
  uuid: string | null;
  removable: boolean;
  read_only: boolean;
  children: BpmDevice[];
}

export const FS_OPTIONS = ['ext4', 'btrfs', 'xfs', 'fat32', 'ntfs', 'swap'] as const;
export type FsOption = (typeof FS_OPTIONS)[number];

export function formatBytes(bytes: number): string {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}
