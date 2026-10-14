import type { ShellThemeExtras } from '../data/builtinThemes';

/**
 * Size-dependent parts of the full-screen app menu that a theme can change
 * (`extras.menuTileSize`). Colours, blur and shapes are plain CSS variables
 * (see shellThemeCss.ts); the tile size also changes the icon glyph and how
 * many columns fit, which CSS alone can't express, so it lives here.
 */
export interface MenuTileMetrics {
  /** Edge of the icon box in rem. */
  boxRem: number;
  /** Glyph size in px (passed to the icon component). */
  glyph: number;
  /** Tailwind grid-column classes. */
  gridClass: string;
}

const METRICS: Record<NonNullable<ShellThemeExtras['menuTileSize']>, MenuTileMetrics> = {
  compact: { boxRem: 2.75, glyph: 26, gridClass: 'grid-cols-5 lg:grid-cols-7 xl:grid-cols-8' },
  normal: { boxRem: 3.5, glyph: 36, gridClass: 'grid-cols-4 lg:grid-cols-5 xl:grid-cols-6' },
  large: { boxRem: 4.5, glyph: 48, gridClass: 'grid-cols-3 lg:grid-cols-4 xl:grid-cols-5' },
};

/** Metrics for a theme's tile size; unknown/unset → the standard (`normal`) look. */
export function menuTileMetrics(size: ShellThemeExtras['menuTileSize'] | string | undefined): MenuTileMetrics {
  return (size && (METRICS as Record<string, MenuTileMetrics>)[size]) || METRICS.normal;
}
