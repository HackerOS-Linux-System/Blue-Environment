import { describe, it, expect } from 'vitest';
import { menuTileMetrics } from './startMenuTheme';
import { BUILTIN_THEMES, type ShellTheme } from '../data/builtinThemes';
import { themeToCss } from './shellThemeCss';

const base = (): ShellTheme => JSON.parse(JSON.stringify(BUILTIN_THEMES[0]));

describe('menuTileMetrics', () => {
  it('keeps the standard look by default', () => {
    expect(menuTileMetrics(undefined).glyph).toBe(36);
    expect(menuTileMetrics('normal').boxRem).toBe(3.5);
    expect(menuTileMetrics('bogus').glyph).toBe(36);
  });
  it('scales glyph and density with the size', () => {
    expect(menuTileMetrics('compact').glyph).toBeLessThan(menuTileMetrics('normal').glyph);
    expect(menuTileMetrics('large').glyph).toBeGreaterThan(menuTileMetrics('normal').glyph);
    expect(menuTileMetrics('large').gridClass).toContain('grid-cols-3');
  });
});

describe('app menu theme variables', () => {
  it('emits nothing for a theme that does not customise the menu', () => {
    const { vars } = themeToCss(base());
    expect(Object.keys(vars).filter((k) => k.startsWith('--menu-'))).toEqual([]);
  });

  it('maps colour, opacity, blur, sidebar and shapes', () => {
    const t = base();
    t.extras = { menuBackgroundColor: '#102030', menuOpacity: 55, menuBlur: 12, menuSidebar: 'solid', menuTileShape: 'circle', menuSearchShape: 'pill' };
    const { vars } = themeToCss(t);
    expect(vars['--menu-bg-rgb']).toBe('16, 32, 48');
    expect(vars['--menu-opacity']).toBe('0.55');
    expect(vars['--menu-blur']).toBe('12px');
    expect(vars['--menu-sidebar-bg']).toBe('rgba(16, 32, 48, 0.9)');
    expect(vars['--menu-tile-radius']).toBe('9999px');
    expect(vars['--menu-search-radius']).toBe('9999px');
  });

  it('transparent sidebar, square shapes and clamping', () => {
    const t = base();
    t.extras = { menuOpacity: 1, menuBlur: 500, menuSidebar: 'transparent', menuTileShape: 'square', menuSearchShape: 'square' };
    const { vars } = themeToCss(t);
    expect(vars['--menu-opacity']).toBe('0.2');
    expect(vars['--menu-blur']).toBe('60px');
    expect(vars['--menu-sidebar-bg']).toBe('transparent');
    expect(vars['--menu-tile-radius']).toBe('0.25rem');
    expect(vars['--menu-search-radius']).toBe('0.375rem');
  });
});
