export type OsType = 'linux' | 'windows' | 'bsd' | 'other';
export type VmStatus = 'stopped' | 'running';

export interface VmConfig {
  id: string;
  name: string;
  osType: OsType;
  cpuCores: number;
  memoryMb: number;
  diskPath: string;
  diskSizeGb: number;
  isoPath: string | null;
  useKvm: boolean;
  createdAt: string;
}

export interface VmSummary extends VmConfig {
  status: VmStatus;
}

export const OS_TYPE_LABELS: Record<OsType, string> = {
  linux: 'Linux',
  windows: 'Windows',
  bsd: 'BSD',
  other: 'Other',
};
