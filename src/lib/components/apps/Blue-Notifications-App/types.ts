export interface NotificationRule {
  id: string;
  name: string;
  kind: string; // only "rss" has a real checker today — see mod.rs's module doc
  url: string;
  intervalMinutes: number;
  enabled: boolean;
  lastSeenGuids: string[];
}

export const INTERVAL_OPTIONS = [5, 15, 30, 60, 180, 360, 1440] as const;
