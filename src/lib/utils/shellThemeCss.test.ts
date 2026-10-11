import { describe, it, expect } from 'vitest';
import { BUILTIN_THEMES, type ShellTheme } from '../data/builtinThemes';
import { THEME_ATTRS, THEME_CSS_VARS, themeToCss } from './shellThemeCss';

const base = (): ShellTheme => JSON.parse(JSON.stringify(BUILTIN_THEMES[0]));

describe('themeToCss', () => {
  it('maps the palette, including the previously unused elevated/border colours', () => {
    const t = base();
    const { vars, attrs, rootFontSize } = themeToCss(t);
    expect(vars['--bg-primary']).toBe(t.colors.background);
    expect(vars['--surface-elevated']).toBe(t.colors.surfaceElevated);
    expect(vars['--border-color']).toBe(t.colors.border);
    expect(attrs['data-shell-theme']).toBe(t.id);
    expect(rootFontSize).toBeNull();
  });

  it('emits no window/scale overrides for a theme without extras', () => {
    const { vars, attrs } = themeToCss(base());
    expect(vars['--window-shadow']).toBeUndefined();
    expect(vars['--titlebar-height']).toBeUndefined();
    expect(attrs['data-window-shadow']).toBeUndefined();
    expect(attrs['data-scrollbar']).toBeUndefined();
  });

  it('applies the new window, wallpaper and panel extras', () => {
    const t = base();
    t.extras = {
      windowShadow: 'glow', windowOpacity: 85, windowBorderWidth: 2, windowBorderColor: '#ff00ff',
      titlebarHeight: 44, uiScalePercent: 110, scrollbarStyle: 'thin',
      wallpaperBrightness: 60, wallpaperSaturation: 0, panelColor: '#102030', panelOpacity: 70,
    };
    const { vars, attrs, rootFontSize } = themeToCss(t);
    expect(vars['--window-shadow']).toContain('rgba(59, 130, 246, 0.45)'); // accent #3b82f6 glow
    expect(vars['--window-opacity']).toBe('0.85');
    expect(vars['--window-border-width']).toBe('2px');
    expect(vars['--window-border-color']).toBe('#ff00ff');
    expect(vars['--titlebar-height']).toBe('44px');
    expect(vars['--wallpaper-brightness']).toBe('0.6');
    expect(vars['--wallpaper-saturate']).toBe('0');
    expect(vars['--panel-rgb']).toBe('16, 32, 48');
    expect(vars['--panel-opacity']).toBe('0.7');
    expect(rootFontSize).toBe('110%');
    expect(attrs['data-scrollbar']).toBe('thin');
    expect(attrs['data-window-shadow']).toBe('');
  });

  it('clamps out-of-range values instead of emitting them', () => {
    const t = base();
    t.extras = { titlebarHeight: 500, uiScalePercent: 10, windowOpacity: 5, windowBorderWidth: 99, cornerRadiusPx: 999 };
    const { vars, rootFontSize } = themeToCss(t);
    expect(vars['--titlebar-height']).toBe('56px');
    expect(rootFontSize).toBe('80%');
    expect(vars['--window-opacity']).toBe('0.6');
    expect(vars['--window-border-width']).toBe('4px');
    expect(vars['--shell-radius']).toBe('32px');
  });

  it('only ever emits variables and attributes it declares (so reset can undo them)', () => {
    const t = base();
    t.extras = {
      windowShadow: 'strong', windowOpacity: 90, windowBorderWidth: 1, windowBorderColor: '#fff', titlebarHeight: 40,
      scrollbarStyle: 'accent', wallpaperBlur: 5, wallpaperBrightness: 80, wallpaperSaturation: 120,
      panelColor: '#000', panelOpacity: 50, accentSecondary: '#f0f', fontFamily: 'serif', animationSpeed: 'fast', cornerRadiusPx: 4,
    };
    t.layout.cornerStyle = 'sharp';
    const { vars, attrs } = themeToCss(t);
    for (const k of Object.keys(vars)) expect(THEME_CSS_VARS as readonly string[]).toContain(k);
    for (const k of Object.keys(attrs)) expect(THEME_ATTRS as readonly string[]).toContain(k);
  });

  it('every builtin theme produces valid output', () => {
    for (const t of BUILTIN_THEMES) {
      const { vars, attrs } = themeToCss(t);
      expect(vars['--accent']).toBe(t.colors.accent);
      expect(attrs['data-shell-theme']).toBe(t.id);
    }
  });
});
