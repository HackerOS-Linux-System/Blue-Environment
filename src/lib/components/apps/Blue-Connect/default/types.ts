export type DeviceType = 'phone' | 'tablet' | 'desktop' | 'laptop' | 'tv' | 'unknown';

export interface DiscoveredDevice {
  id: string;
  name: string;
  deviceType: DeviceType;
  address: string;
  tcpPort: number;
  paired: boolean;
  /** SHA-256 fingerprint of the certificate pinned when this device was
   * paired (see src-tauri/src/BlueConnect/tls.rs) — `null`/undefined for
   * devices that were never paired, or paired before certificate
   * pinning existed. Shown in the UI so a person can actually see what
   * they're trusting, not stored purely for internal bookkeeping. */
  pinnedCertSha256?: string | null;
}

export const DEVICE_TYPE_LABELS: Record<DeviceType, string> = {
  phone: 'Phone',
  tablet: 'Tablet',
  desktop: 'Desktop',
  laptop: 'Laptop',
  tv: 'TV',
  unknown: 'Unknown device',
};
