import { DEFAULT_TOP_BAR, normalizeTopBar, type TopBarConfig } from './topBarConfig';

/**
 * One-click bar looks (Settings → Bar → Presets).
 *
 * A preset is a patch on top of the DEFAULT bar — never on top of whatever the
 * bar looks like now — so applying "Minimal" always gives the same result. It
 * keeps the person's *personal* choices that have nothing to do with the look:
 * the Start label and the clock format. Some presets also want the bar on a
 * particular screen edge; that setting lives outside the bar config
 * (`panelPosition`), hence the separate field.
 */
export interface BarPreset {
  id: 'default' | 'minimal' | 'dock' | 'compact' | 'classic';
  /** i18n key of the name. */
  labelKey: string;
  /** i18n key of the one-line description. */
  descKey: string;
  bar: Partial<TopBarConfig>;
  panelPosition?: 'top' | 'bottom';
}

export const BAR_PRESETS: readonly BarPreset[] = [
  { id: 'default', labelKey: 'sbar.preset_default', descKey: 'sbar.preset_default_desc', bar: {} },
  {
    id: 'minimal', labelKey: 'sbar.preset_minimal', descKey: 'sbar.preset_minimal_desc',
    bar: {
      showWeather: false, showClipboard: false, showWorkspaces: false, showBatteryPercent: false,
      runningAppsLabels: false, pinnedStyle: 'flat', indicatorStyle: 'dot', fontSize: 11, itemSpacing: 4,
      showBorder: false, blur: 'none',
    },
  },
  {
    // A macOS-style dock: a floating, rounded bar at the bottom with big flat
    // icons and dot indicators; the app strip is redundant next to it.
    id: 'dock', labelKey: 'sbar.preset_dock', descKey: 'sbar.preset_dock_desc',
    bar: {
      floating: true, floatingMargin: 10, cornerRadius: 24, blur: 'xl', shadow: true, showBorder: true,
      pinnedStyle: 'flat', pinnedIconSize: 30, indicatorStyle: 'dot', startButtonStyle: 'pill',
      showRunningApps: false, showWorkspaces: false, showWeather: false, showClipboard: false, showBatteryPercent: false,
    },
    panelPosition: 'bottom',
  },
  {
    id: 'compact', labelKey: 'sbar.preset_compact', descKey: 'sbar.preset_compact_desc',
    bar: {
      pinnedIconSize: 16, fontSize: 11, itemSpacing: 4, runningAppsLabels: false, startButtonStyle: 'square',
      showWeather: false, showBatteryPercent: false,
    },
  },
  {
    // Taskbar like on Windows: full-width, at the bottom, running apps with labels.
    id: 'classic', labelKey: 'sbar.preset_classic', descKey: 'sbar.preset_classic_desc',
    bar: {
      floating: false, runningAppsLabels: true, runningAppsMax: 8, pinnedStyle: 'flat', indicatorStyle: 'line',
      startButtonStyle: 'square', showBorder: true, blur: 'md',
    },
    panelPosition: 'bottom',
  },
];

/** Personal preferences a preset must not overwrite. */
const KEEP: (keyof TopBarConfig)[] = ['startLabel', 'clockStyle', 'clock24h', 'showSeconds'];

export function getBarPreset(id: string): BarPreset | undefined {
  return BAR_PRESETS.find((p) => p.id === id);
}

/** The bar config a preset produces for `current`. Unknown ids return `current` unchanged. */
export function applyBarPreset(current: TopBarConfig, id: string): TopBarConfig {
  const preset = getBarPreset(id);
  if (!preset) return current;
  const kept: Partial<TopBarConfig> = {};
  for (const k of KEEP) (kept as Record<string, unknown>)[k] = current[k];
  return normalizeTopBar({ ...DEFAULT_TOP_BAR, ...preset.bar, ...kept });
}
