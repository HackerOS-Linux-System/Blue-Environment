export interface DeviceNotification {
  id: string;
  appName: string;
  title: string;
  text: string;
  time: number;
}

/** "73 %" or "73 % ⚡ (charging)". */
export function batteryLabel(b: { percent: number; charging: boolean }, chargingWord: string): string {
  const pct = Math.max(0, Math.min(100, Math.round(b.percent)));
  return b.charging ? `${pct} % · ${chargingWord}` : `${pct} %`;
}
