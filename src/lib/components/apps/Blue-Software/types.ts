export type SoftwareTab = 'available' | 'installed' | 'updates';
export type ViewMode = 'grid' | 'list';

export type { PackageInfo } from '../../../utils/systemBridge';

export interface InstallLog {
  pkgId: string;
  lines: string[];
  done: boolean;
  success: boolean;
}
