import type { ShellTheme, ShellThemeExtras } from '../data/builtinThemes';
import { rgbTriplet, withAlpha } from './colorUtil';

/**
 * Pure translation of a shell theme into CSS: custom properties on `<html>`,
 * `data-*` attributes (which `app.css` keys its rules off) and the root font
 * size. Kept free of DOM access so it can be unit-tested; `ShellThemeStyle.
 * svelte` is the only caller that actually writes it to the document.
 *
 * Every property/attribute this function can ever emit is listed in
 * [`THEME_CSS_VARS`] / [`THEME_ATTRS`], so the "no theme" case can remove
 * exactly what a previous theme set.
 */

export const THEME_CSS_VARS = [
  '--bg-primary', '--bg-secondary', '--text-primary', '--text-secondary', '--accent', '--accent-hover',
  '--surface-elevated', '--border-color',
  '--shell-radius', '--wallpaper-blur', '--wallpaper-brightness', '--wallpaper-saturate',
  '--panel-opacity', '--panel-rgb', '--accent-gradient', '--shell-font', '--shell-anim-duration',
  '--window-shadow', '--window-opacity', '--window-border-width', '--window-border-color', '--titlebar-height',
  '--menu-bg-rgb', '--menu-opacity', '--menu-blur', '--menu-sidebar-bg', '--menu-tile-radius', '--menu-search-radius',
] as const;

export const THEME_ATTRS = [
  'data-shell-theme', 'data-icon-style', 'data-window-shadow', 'data-window-border',
  'data-window-opacity', 'data-titlebar', 'data-scrollbar',
] as const;

export interface ThemeCss {
  vars: Record<string, string>;
  attrs: Record<string, string>;
  /** `html { font-size }` — scales every rem-based size in the shell; `null` = leave alone. */
  rootFontSize: string | null;
}

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

export const WINDOW_SHADOWS = ['none', 'soft', 'medium', 'strong', 'glow'] as const;
export const SCROLLBAR_STYLES = ['default', 'thin', 'hidden', 'accent'] as const;

function windowShadow(kind: ShellThemeExtras['windowShadow'], accent: string): string | null {
  switch (kind) {
    case 'none': return 'none';
    case 'soft': return '0 4px 16px rgba(0, 0, 0, 0.25)';
    case 'medium': return '0 10px 30px rgba(0, 0, 0, 0.4)';
    case 'strong': return '0 25px 60px rgba(0, 0, 0, 0.65)';
    case 'glow': return `0 0 28px ${withAlpha(accent, 0.45) ?? accent}, 0 8px 24px rgba(0, 0, 0, 0.4)`;
    default: return null;
  }
}

export function themeToCss(theme: ShellTheme): ThemeCss {
  const vars: Record<string, string> = {};
  const attrs: Record<string, string> = {};
  const e: ShellThemeExtras = theme.extras ?? {};
  const c = theme.colors;

  vars['--bg-primary'] = c.background;
  vars['--bg-secondary'] = c.surface;
  vars['--text-primary'] = c.text;
  vars['--text-secondary'] = c.textMuted;
  vars['--accent'] = c.accent;
  vars['--accent-hover'] = c.accent;
  // `surfaceElevated` and `border` used to be stored on every theme and read
  // by nothing. They now back `.theme-bg-elevated` / `.theme-border` (app.css).
  vars['--surface-elevated'] = c.surfaceElevated;
  vars['--border-color'] = c.border;

  if (theme.layout.cornerStyle === 'sharp') vars['--shell-radius'] = '0px';
  if (num(e.cornerRadiusPx)) vars['--shell-radius'] = `${clamp(e.cornerRadiusPx, 0, 32)}px`;

  if (num(e.wallpaperBlur) && e.wallpaperBlur > 0) vars['--wallpaper-blur'] = `${clamp(e.wallpaperBlur, 0, 40)}px`;
  if (num(e.wallpaperBrightness) && e.wallpaperBrightness !== 100) vars['--wallpaper-brightness'] = String(clamp(e.wallpaperBrightness, 30, 130) / 100);
  if (num(e.wallpaperSaturation) && e.wallpaperSaturation !== 100) vars['--wallpaper-saturate'] = String(clamp(e.wallpaperSaturation, 0, 200) / 100);

  if (num(e.panelOpacity)) vars['--panel-opacity'] = String(clamp(e.panelOpacity, 0, 100) / 100);
  const panelRgb = rgbTriplet(e.panelColor);
  if (panelRgb) vars['--panel-rgb'] = panelRgb;

  vars['--accent-gradient'] = e.accentSecondary
    ? `linear-gradient(135deg, ${c.accent}, ${e.accentSecondary})`
    : c.accent;
  if (e.fontFamily) vars['--shell-font'] = e.fontFamily;
  vars['--shell-anim-duration'] = e.animationSpeed === 'fast' ? '80ms' : e.animationSpeed === 'none' ? '0ms' : '200ms';

  // ── Windows ──────────────────────────────────────────────────────────
  const shadow = windowShadow(e.windowShadow, c.accent);
  if (shadow) { vars['--window-shadow'] = shadow; attrs['data-window-shadow'] = ''; }
  if (num(e.windowOpacity) && e.windowOpacity < 100) {
    vars['--window-opacity'] = String(clamp(e.windowOpacity, 60, 100) / 100);
    attrs['data-window-opacity'] = '';
  }
  if (num(e.windowBorderWidth) || e.windowBorderColor) {
    if (num(e.windowBorderWidth)) vars['--window-border-width'] = `${clamp(e.windowBorderWidth, 0, 4)}px`;
    if (e.windowBorderColor) vars['--window-border-color'] = e.windowBorderColor;
    attrs['data-window-border'] = '';
  }
  if (num(e.titlebarHeight) && e.titlebarHeight !== 36) {
    vars['--titlebar-height'] = `${clamp(e.titlebarHeight, 28, 56)}px`;
    attrs['data-titlebar'] = '';
  }

  if (e.scrollbarStyle && e.scrollbarStyle !== 'default') attrs['data-scrollbar'] = e.scrollbarStyle;

  // ── Full-screen application menu (read by StartMenu.svelte with fallbacks) ──
  const menuRgb = rgbTriplet(e.menuBackgroundColor);
  if (menuRgb) vars['--menu-bg-rgb'] = menuRgb;
  if (num(e.menuOpacity)) vars['--menu-opacity'] = String(clamp(e.menuOpacity, 20, 100) / 100);
  if (num(e.menuBlur)) vars['--menu-blur'] = `${clamp(e.menuBlur, 0, 60)}px`;
  if (e.menuSidebar === 'transparent') vars['--menu-sidebar-bg'] = 'transparent';
  else if (e.menuSidebar === 'solid') vars['--menu-sidebar-bg'] = `rgba(${menuRgb ?? '2, 6, 23'}, 0.9)`;
  if (e.menuTileShape === 'circle') vars['--menu-tile-radius'] = '9999px';
  else if (e.menuTileShape === 'square') vars['--menu-tile-radius'] = '0.25rem';
  if (e.menuSearchShape === 'pill') vars['--menu-search-radius'] = '9999px';
  else if (e.menuSearchShape === 'square') vars['--menu-search-radius'] = '0.375rem';

  attrs['data-icon-style'] = theme.layout.iconStyle;
  attrs['data-shell-theme'] = theme.id;

  const rootFontSize = num(e.uiScalePercent) && e.uiScalePercent !== 100
    ? `${clamp(e.uiScalePercent, 80, 130)}%` : null;

  return { vars, attrs, rootFontSize };
}
