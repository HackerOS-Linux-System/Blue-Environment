import { writable } from 'svelte/store';
import { configStore } from '../utils/configStore';

/**
 * `config.theme` (see ControlCenter.svelte's dark/light toggle and
 * app.css's big generated override block) drives light/dark for
 * everything styled with CSS classes — but a handful of games under
 * Blue-Play draw straight to a `<canvas>` with hardcoded hex colors in
 * `fillStyle` calls, which CSS can never reach no matter how the
 * utility classes around the canvas element are overridden. Those
 * games import this store instead and pick their fill colors in code.
 *
 * Kept deliberately tiny (`'dark' | 'light'`, not a copy of every shell
 * theme) — canvas games only need to know which of two palettes to
 * draw with, not the full theming system.
 */
export type ColorScheme = 'dark' | 'light';

const LIGHT_THEME_ID = 'light-glass';

function toScheme(themeId: string | undefined): ColorScheme {
  return themeId === LIGHT_THEME_ID ? 'light' : 'dark';
}

export const colorScheme = writable<ColorScheme>(toScheme(configStore.get().theme));

configStore.subscribe((cfg) => colorScheme.set(toScheme(cfg.theme)));
