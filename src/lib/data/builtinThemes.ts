export type WindowControlsStyle = 'macos' | 'windows' | 'gnome' | 'minimal';
export type WindowControlsPosition = 'left' | 'right';
export type PanelPosition = 'top' | 'bottom' | 'left' | 'right';
export type CornerStyle = 'rounded' | 'sharp';
export type IconStyle = 'outline' | 'filled';

export interface ShellThemeColors {
  accent: string;
  background: string;
  surface: string;
  surfaceElevated: string;
  text: string;
  textMuted: string;
  border: string;
}

export interface ShellThemeLayout {
  panelPosition: PanelPosition;
  windowControlsPosition: WindowControlsPosition;
  windowControlsStyle: WindowControlsStyle;
  cornerStyle: CornerStyle;
  iconStyle: IconStyle;
}

export interface ShellTheme {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  /** Lucide icon name (see lucide-svelte) used as a lightweight stand-in
   * for a real preview screenshot — genuine preview images are a real
   * follow-up (see ThemesSection.svelte's doc), not attempted here. */
  previewIcon: string;
  colors: ShellThemeColors;
  layout: ShellThemeLayout;
  requiresRestart: true;
  builtin: true;
  placeholder?: boolean;
  comingSoon?: boolean;
  /** Public-root-relative path (served straight from `public/`, see
   * Hydra's entry below — the only theme that sets this so far) to a
   * wallpaper this theme forces while active, overriding whatever
   * wallpaper the person has otherwise chosen. Only really makes sense
   * for a theme whose *identity* is tied to a specific piece of art the
   * way Hydra's is — most themes should leave this unset and just let
   * the person's own wallpaper choice show through their color palette.
   * Deliberately a *system* path (see below), not a bundled asset — the
   * app doesn't ship this file itself. */
  wallpaper?: string;
  /** Public-root-relative path to a small preview image for
   * ThemesSection.svelte's grid card, used instead of `previewIcon`
   * when set. Unlike `wallpaper` above, this one *is* bundled with the
   * app (`public/previews/...`) — a settings-UI thumbnail needs to
   * render reliably regardless of what's actually installed on the
   * system, unlike the real wallpaper it's a preview *of*. */
  previewImage?: string;
}

export const BUILTIN_THEMES: ShellTheme[] = [
  {
    id: 'azure',
    name: 'Azure',
    description: 'The default Blue Environment look — cool blues, a top panel, right-aligned window controls.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Droplet',
    colors: {
      accent: '#3b82f6', background: '#0f172a', surface: '#1e293b', surfaceElevated: '#293548',
      text: '#f8fafc', textMuted: '#94a3b8', border: 'rgba(255,255,255,0.08)',
    },
    layout: { panelPosition: 'top', windowControlsPosition: 'right', windowControlsStyle: 'windows', cornerStyle: 'rounded', iconStyle: 'outline' },
    requiresRestart: true, builtin: true,
  },
  {
    id: 'obsidian',
    name: 'Obsidian',
    description: 'Near-black, high-contrast, minimal chrome. Sharp corners, a bottom panel.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Gem',
    colors: {
      accent: '#a855f7', background: '#09090b', surface: '#18181b', surfaceElevated: '#27272a',
      text: '#fafafa', textMuted: '#a1a1aa', border: 'rgba(255,255,255,0.06)',
    },
    layout: { panelPosition: 'bottom', windowControlsPosition: 'right', windowControlsStyle: 'minimal', cornerStyle: 'sharp', iconStyle: 'outline' },
    requiresRestart: true, builtin: true,
  },
  {
    id: 'aurora',
    name: 'Aurora',
    description: 'Teal-and-green gradient palette, macOS-style left-aligned traffic-light window controls.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Sparkles',
    colors: {
      accent: '#14b8a6', background: '#0c1a17', surface: '#132420', surfaceElevated: '#1c332d',
      text: '#f0fdfa', textMuted: '#5eead4', border: 'rgba(20,184,166,0.15)',
    },
    layout: { panelPosition: 'top', windowControlsPosition: 'left', windowControlsStyle: 'macos', cornerStyle: 'rounded', iconStyle: 'filled' },
    requiresRestart: true, builtin: true,
  },
  {
    id: 'ember',
    name: 'Ember',
    description: 'Warm oranges and deep browns. Windows-style controls, a left side panel.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Flame',
    colors: {
      accent: '#f97316', background: '#1c1210', surface: '#2a1b17', surfaceElevated: '#3a251f',
      text: '#fff7ed', textMuted: '#fdba74', border: 'rgba(249,115,22,0.15)',
    },
    layout: { panelPosition: 'left', windowControlsPosition: 'right', windowControlsStyle: 'windows', cornerStyle: 'rounded', iconStyle: 'filled' },
    requiresRestart: true, builtin: true,
  },
  {
    id: 'glacier',
    name: 'Glacier',
    description: 'Pale, icy, high-key light theme — a rare light-mode built-in among mostly dark ones.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Snowflake',
    colors: {
      accent: '#0ea5e9', background: '#f0f9ff', surface: '#e0f2fe', surfaceElevated: '#ffffff',
      text: '#0c4a6e', textMuted: '#0369a1', border: 'rgba(14,165,233,0.15)',
    },
    layout: { panelPosition: 'top', windowControlsPosition: 'right', windowControlsStyle: 'gnome', cornerStyle: 'rounded', iconStyle: 'outline' },
    requiresRestart: true, builtin: true,
  },
  {
    id: 'crimson',
    name: 'Crimson',
    description: 'Deep reds and near-black. Sharp corners, minimal window controls, bottom panel.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Zap',
    colors: {
      accent: '#dc2626', background: '#180a0a', surface: '#241010', surfaceElevated: '#331616',
      text: '#fef2f2', textMuted: '#fca5a5', border: 'rgba(220,38,38,0.15)',
    },
    layout: { panelPosition: 'bottom', windowControlsPosition: 'left', windowControlsStyle: 'minimal', cornerStyle: 'sharp', iconStyle: 'filled' },
    requiresRestart: true, builtin: true,
  },
  {
    id: 'mono',
    name: 'Mono',
    description: 'Grayscale, no accent color to speak of. Sharp corners, GNOME-style controls.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Circle',
    colors: {
      accent: '#71717a', background: '#0a0a0a', surface: '#171717', surfaceElevated: '#262626',
      text: '#fafafa', textMuted: '#a3a3a3', border: 'rgba(255,255,255,0.08)',
    },
    layout: { panelPosition: 'top', windowControlsPosition: 'right', windowControlsStyle: 'gnome', cornerStyle: 'sharp', iconStyle: 'outline' },
    requiresRestart: true, builtin: true,
  },
  {
    id: 'orchid',
    name: 'Orchid',
    description: 'Pink-and-purple, rounded, playful. macOS-style controls, right side panel.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Flower2',
    colors: {
      accent: '#ec4899', background: '#1a0f18', surface: '#271524', surfaceElevated: '#372034',
      text: '#fdf4ff', textMuted: '#f0abfc', border: 'rgba(236,72,153,0.15)',
    },
    layout: { panelPosition: 'right', windowControlsPosition: 'left', windowControlsStyle: 'macos', cornerStyle: 'rounded', iconStyle: 'filled' },
    requiresRestart: true, builtin: true,
  },

  // ── Formerly placeholders, now fully applied ─────────────────────────
  // Hydra and HDE below used to ship with `placeholder: true` (visible
  // in the grid, not selectable) while their palettes/layouts were
  // being finished. Neither sets that flag anymore — both are real,
  // fully-appliable themes now; see `ShellThemeStyle.svelte` for what
  // "applied" actually changes. Kept as their own labeled group here
  // only because they're this file's two most recent additions, not
  // because anything about them is still incomplete.
  {
    id: 'hydra',
    name: 'Hydra',
    description: 'Neon cyberpunk hacker aesthetic — pink/purple/blue gradient panel moved to the bottom of the screen, glowing accents, bundled matching wallpaper. The first shell theme with a real, fully-applied look (not a placeholder anymore) — see ShellThemeStyle.svelte for exactly what it changes.',
    author: 'Blue Environment',
    version: '1.0.0',
    previewIcon: 'Waves',
    colors: {
      accent: '#ec4899', background: '#12071f', surface: '#1d0f2e', surfaceElevated: '#2a1642',
      text: '#fdf2ff', textMuted: '#d8b4fe', border: 'rgba(236,72,153,0.25)',
    },
    layout: { panelPosition: 'bottom', windowControlsPosition: 'right', windowControlsStyle: 'windows', cornerStyle: 'rounded', iconStyle: 'filled' },
    requiresRestart: true, builtin: true,
    // Assumes the system already has this file — same convention every
    // other wallpaper path in this app already uses (see
    // `configStore.ts`'s own doc on `/usr/share/wallpapers/...`
    // resolution, and `toAssetUrl()` in systemBridge.ts, which is what
    // actually turns this bare path into a loadable `asset://` URL — no
    // `file://` prefix needed, `toAssetUrl` strips that if present
    // anyway). Not bundled with the app itself (no `public/wallpapers/`
    // copy shipped) — if this exact file isn't present on a given
    // install, the wallpaper silently falls through to the gradient
    // fallback `App.svelte`'s background already has for a missing/
    // unresolved wallpaper (see that file's own fix for the equivalent
    // default-wallpaper case), not a broken-image icon.
    wallpaper: '/usr/share/wallpapers/HackerOS-Wallpapers/Wallpaper22.png',
    previewImage: '/previews/hydra.png',
  },
  {
    id: 'hde',
    name: 'HDE',
    description: 'HackerOS Desktop Environment\'s signature look, brought to Blue Environment — phosphor-green on near-black, sharp corners, a minimal top panel with left-aligned window controls.',
    author: 'HackerOS',
    version: '1.0.0',
    previewIcon: 'Terminal',
    colors: {
      accent: '#22c55e', background: '#050805', surface: '#0a120a', surfaceElevated: '#0f1c0f',
      text: '#bbf7d0', textMuted: '#4ade80', border: 'rgba(34,197,94,0.25)',
    },
    layout: { panelPosition: 'top', windowControlsPosition: 'left', windowControlsStyle: 'minimal', cornerStyle: 'sharp', iconStyle: 'outline' },
    requiresRestart: true, builtin: true,
    // Same convention as Hydra's `wallpaper` above: a system path this
    // theme assumes already exists (not bundled with the app) — falls
    // through to the normal missing-wallpaper gradient if it doesn't.
    wallpaper: '/usr/share/wallpapers/HackerOS-Wallpapers/Wallpaper5.png',
    previewImage: '/previews/hde.png',
  },
];

export function getBuiltinTheme(id: string): ShellTheme | undefined {
  return BUILTIN_THEMES.find((t) => t.id === id);
}

export const DEFAULT_SHELL_THEME_ID = 'azure';

/** Resolves a stored `shellThemeId` to the theme that should actually
 * override anything (colors/wallpaper/panel position) — `null` means
 * "no override, use normal look", which is the case both for the
 * default theme itself (Azure needs no CSS overrides, it's meant to
 * just be this app's already-existing normal appearance) and for a
 * placeholder (not selectable/appliable — see ThemesSection.svelte).
 * Shared by `App.svelte` (wallpaper/panel-position overrides) and
 * `ShellThemeStyle.svelte` (color overrides) so both agree on exactly
 * the same "is there a real active theme" condition rather than two
 * independently-maintained copies of it drifting apart. */
export function resolveActiveShellTheme(id: string | undefined): ShellTheme | null {
  const resolved = getBuiltinTheme(id ?? DEFAULT_SHELL_THEME_ID);
  if (!resolved || resolved.id === DEFAULT_SHELL_THEME_ID || resolved.placeholder) return null;
  return resolved;
}
