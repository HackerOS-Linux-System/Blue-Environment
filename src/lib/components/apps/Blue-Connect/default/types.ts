export type DeviceType = 'phone' | 'tablet' | 'desktop' | 'laptop' | 'tv' | 'unknown';

export interface DiscoveredDevice {
  id: string;
  name: string;
  deviceType: DeviceType;
  address: string;
  tcpPort: number;
  paired: boolean;
}

export const DEVICE_TYPE_LABELS: Record<DeviceType, string> = {
  phone: 'Phone',
  tablet: 'Tablet',
  desktop: 'Desktop',
  laptop: 'Laptop',
  tv: 'TV',
  unknown: 'Unknown device',
};
