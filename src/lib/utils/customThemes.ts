import { writable, get } from 'svelte/store';
import { SystemBridge } from './systemBridge';
import type { ShellTheme, ShellThemeExtras } from '../data/builtinThemes';

/** Mirrors `custom_shell_themes.rs`'s `CustomShellTheme` — snake_case
 * field names because that's what `serde` sends over IPC for a Rust
 * struct with no `#[serde(rename_all = "camelCase")]`. Converted to/from
 * the frontend's own camelCase `ShellTheme` shape by `toShellTheme` /
 * `fromShellTheme` below, so the rest of the app (ThemesSection,
 * ShellThemeStyle) only ever deals with the one shape it already knows. */
interface RawCustomTheme {
  id: string;
  name: string;
  description: string;
  colors: {
    accent: string; background: string; surface: string; surface_elevated: string;
    text: string; text_muted: string; border: string;
  };
  layout: {
    panel_position: string; window_controls_position: string; window_controls_style: string;
    corner_style: string; icon_style: string;
  };
  extras: {
    corner_radius_px?: number; wallpaper_blur?: number; panel_opacity?: number;
    accent_secondary?: string; font_family?: string; animation_speed?: string;
  };
}

export function toShellTheme(raw: RawCustomTheme): ShellTheme {
  return {
    id: raw.id, name: raw.name, description: raw.description,
    author: 'You', version: '1.0.0', previewIcon: 'Palette',
    colors: {
      accent: raw.colors.accent, background: raw.colors.background, surface: raw.colors.surface,
      surfaceElevated: raw.colors.surface_elevated, text: raw.colors.text, textMuted: raw.colors.text_muted,
      border: raw.colors.border,
    },
    layout: {
      panelPosition: raw.layout.panel_position as ShellTheme['layout']['panelPosition'],
      windowControlsPosition: raw.layout.window_controls_position as ShellTheme['layout']['windowControlsPosition'],
      windowControlsStyle: raw.layout.window_controls_style as ShellTheme['layout']['windowControlsStyle'],
      cornerStyle: raw.layout.corner_style as ShellTheme['layout']['cornerStyle'],
      iconStyle: raw.layout.icon_style as ShellTheme['layout']['iconStyle'],
    },
    requiresRestart: false,
    builtin: false,
    custom: true,
    extras: {
      cornerRadiusPx: raw.extras.corner_radius_px,
      wallpaperBlur: raw.extras.wallpaper_blur,
      panelOpacity: raw.extras.panel_opacity,
      accentSecondary: raw.extras.accent_secondary,
      fontFamily: raw.extras.font_family,
      animationSpeed: raw.extras.animation_speed as ShellThemeExtras['animationSpeed'],
    },
  };
}

export function fromShellTheme(theme: ShellTheme): RawCustomTheme {
  return {
    id: theme.id, name: theme.name, description: theme.description,
    colors: {
      accent: theme.colors.accent, background: theme.colors.background, surface: theme.colors.surface,
      surface_elevated: theme.colors.surfaceElevated, text: theme.colors.text, text_muted: theme.colors.textMuted,
      border: theme.colors.border,
    },
    layout: {
      panel_position: theme.layout.panelPosition, window_controls_position: theme.layout.windowControlsPosition,
      window_controls_style: theme.layout.windowControlsStyle, corner_style: theme.layout.cornerStyle,
      icon_style: theme.layout.iconStyle,
    },
    extras: {
      corner_radius_px: theme.extras?.cornerRadiusPx,
      wallpaper_blur: theme.extras?.wallpaperBlur,
      panel_opacity: theme.extras?.panelOpacity,
      accent_secondary: theme.extras?.accentSecondary,
      font_family: theme.extras?.fontFamily,
      animation_speed: theme.extras?.animationSpeed,
    },
  };
}

export const customShellThemes = writable<ShellTheme[]>([]);
let loaded = false;

export async function ensureCustomThemesLoaded(): Promise<ShellTheme[]> {
  if (loaded) return get(customShellThemes);
  return refreshCustomThemes();
}

export async function refreshCustomThemes(): Promise<ShellTheme[]> {
  const raw = await SystemBridge.invokeCommand<RawCustomTheme[]>('custom_theme_list').catch(() => []);
  loaded = true;
  const themes = (raw ?? []).map(toShellTheme);
  customShellThemes.set(themes);
  return themes;
}

export function newCustomThemeId(): string {
  return `custom-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function blankCustomTheme(): ShellTheme {
  return {
    id: newCustomThemeId(), name: 'My Theme', description: '', author: 'You', version: '1.0.0',
    previewIcon: 'Palette',
    colors: {
      accent: '#38bdf8', background: '#0f172a', surface: '#1e293b', surfaceElevated: '#293548',
      text: '#f8fafc', textMuted: '#94a3b8', border: 'rgba(255,255,255,0.08)',
    },
    layout: { panelPosition: 'top', windowControlsPosition: 'right', windowControlsStyle: 'windows', cornerStyle: 'rounded', iconStyle: 'outline' },
    requiresRestart: false, builtin: false, custom: true,
    extras: { cornerRadiusPx: 12, wallpaperBlur: 0, panelOpacity: 100, animationSpeed: 'normal' },
  };
}

export async function saveCustomTheme(theme: ShellTheme): Promise<{ ok: true } | { ok: false; error: string }> {
  try {
    const raw = await SystemBridge.invokeCommand<RawCustomTheme[]>('custom_theme_upsert', { theme: fromShellTheme(theme) });
    customShellThemes.set((raw ?? []).map(toShellTheme));
    return { ok: true };
  } catch (e) {
    return { ok: false, error: typeof e === 'string' ? e : 'Failed to save theme.' };
  }
}

export async function removeCustomTheme(id: string): Promise<void> {
  const raw = await SystemBridge.invokeCommand<RawCustomTheme[]>('custom_theme_remove', { id }).catch(() => null);
  if (raw) customShellThemes.set(raw.map(toShellTheme));
}

export function exportCustomThemeJson(theme: ShellTheme): string {
  return JSON.stringify(fromShellTheme(theme), null, 2);
}

export async function importCustomThemeJson(jsonText: string): Promise<{ ok: true; theme: ShellTheme } | { ok: false; error: string }> {
  try {
    const raw = await SystemBridge.invokeCommand<RawCustomTheme>('custom_theme_parse_import', { jsonText: jsonText });
    return { ok: true, theme: toShellTheme(raw) };
  } catch (e) {
    return { ok: false, error: typeof e === 'string' ? e : 'Not a valid theme file.' };
  }
}
