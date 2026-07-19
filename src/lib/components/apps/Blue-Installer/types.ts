export type InstallStep = 'welcome' | 'language' | 'keyboard' | 'disk' | 'account' | 'summary' | 'installing' | 'done' | 'error';

export interface DiskInfo { path: string; model: string; sizeBytes: number; sizeLabel: string; removable: boolean; }
export type DiskMode = 'erase' | 'manual';

/** One row of a manually-defined partition table. `sizeMiB: null` means "use remaining space". */
export interface PartitionPlanEntry {
  id: string;
  role: 'esp' | 'root' | 'home' | 'swap' | 'other';
  filesystem: 'fat32' | 'ext4' | 'btrfs' | 'xfs' | 'swap';
  mountpoint: string;
  sizeMiB: number | null;
}

export interface InstallConfig {
  locale: string;
  keyboardLayout: string;
  disk: DiskInfo | null;
  diskMode: DiskMode;
  /** Only used when diskMode === 'manual'; null/empty falls back to the default erase layout. */
  partitions: PartitionPlanEntry[];
  hostname: string;
  username: string;
  fullName: string;
  password: string;
  autoLogin: boolean;
}

export const LOCALES = [
  { code: 'en_US.UTF-8', label: 'English (United States)' },
  { code: 'pl_PL.UTF-8', label: 'Polski (Polska)' },
  { code: 'de_DE.UTF-8', label: 'Deutsch (Deutschland)' },
  { code: 'fr_FR.UTF-8', label: 'Français (France)' },
  { code: 'es_ES.UTF-8', label: 'Español (España)' },
  { code: 'ru_RU.UTF-8', label: 'Русский (Россия)' },
  { code: 'uk_UA.UTF-8', label: 'Українська (Україна)' },
  { code: 'cs_CZ.UTF-8', label: 'Čeština (Česko)' },
];

export const KEYBOARD_LAYOUTS = [
  { id: 'us', label: 'US English' }, { id: 'pl', label: 'Polish' }, { id: 'de', label: 'German' },
  { id: 'fr', label: 'French' }, { id: 'es', label: 'Spanish' }, { id: 'ru', label: 'Russian' },
  { id: 'ua', label: 'Ukrainian' }, { id: 'cz', label: 'Czech' }, { id: 'gb', label: 'UK English' },
];
