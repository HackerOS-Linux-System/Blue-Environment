import type { SettingsTab } from '../components/apps/Settings-App/types';

/** Settings pages offered by Spotlight; `labelKey` is an existing/new i18n key, `en` the search fallback. */
export const SETTINGS_PAGES: { tab: SettingsTab; labelKey: string; en: string; keywords: string }[] = [
  { tab: 'accessibility', labelKey: 'settings.tab.accessibility', en: 'Accessibility', keywords: 'contrast text size magnifier screen reader motion zoom' },
  { tab: 'dnd', labelKey: 'settings.tab.dnd', en: 'Do Not Disturb', keywords: 'notifications silence quiet focus' },
  { tab: 'printers' as SettingsTab, labelKey: 'settings.tab.printers', en: 'Printers', keywords: 'print cups scanner queue' },
  { tab: 'monitors' as SettingsTab, labelKey: 'settings.tab.monitors', en: 'Monitors', keywords: 'display screen resolution scale arrangement multi monitor' },
  { tab: 'power' as SettingsTab, labelKey: 'settings.tab.power', en: 'Power', keywords: 'battery profile performance sleep' },
  { tab: 'parental_controls', labelKey: 'settings.tab.parental_controls', en: 'Parental Controls', keywords: 'children limits screen time pin' },
];
