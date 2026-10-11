<script lang="ts">
  /**
   * Applies the active shell theme (see builtinThemes.ts) to the running UI.
   * Mount once, near the root (App.svelte does this).
   *
   * ── How it works ─────────────────────────────────────────────────────
   * The theme → CSS translation lives in `utils/shellThemeCss.ts` (pure,
   * unit-tested): it returns custom properties, `data-*` attributes and an
   * optional root font size. This component only writes that result to
   * `document.documentElement` and removes whatever a previous theme set.
   *
   * The six base variables (`--bg-primary`, `--bg-secondary`, `--text-*`,
   * `--accent*`) are the ones `app.css`'s `.theme-*` utility classes already
   * read, so a theme's palette reaches every component that follows that
   * convention without per-component changes. Everything newer — window
   * shadow/opacity/border, title-bar height, UI scale, scrollbar style,
   * wallpaper brightness/saturation, panel colour — is exposed through
   * variables + `data-*` attributes that `app.css` and the shell components
   * (`Window.svelte`, `TopBar.svelte`, `App.svelte`'s wallpaper layer) read.
   *
   * Deliberately *inline* properties on `documentElement` rather than a
   * `[data-shell-theme='…']` block per theme: `data-theme` is bound to the
   * dark/light mode setting (a different thing), and custom themes are data,
   * not CSS files.
   *
   * Still not handled here: icon glyph swapping for `iconStyle: 'filled'`
   * (the icon set is outline-only — the attribute is set so CSS can add a
   * soft fill, see `.panel-icon-btn` in app.css).
   */
  import { onDestroy, onMount } from 'svelte';
  import { resolveActiveShellThemeWithCustom, type ShellTheme } from '../data/builtinThemes';
  import { activeShellThemeId } from '../stores/shellTheme';
  import { customShellThemes, ensureCustomThemesLoaded } from '../utils/customThemes';
  import { THEME_ATTRS, THEME_CSS_VARS, themeToCss } from '../utils/shellThemeCss';

  // Only `shellThemeId` is ever read — a bare string prop rather than the
  // full `UserConfig`, so callers don't need to thread a whole config through.
  export let shellThemeId: string | undefined = undefined;

  onMount(() => { ensureCustomThemesLoaded(); });

  $: activeShellTheme = resolveActiveShellThemeWithCustom(shellThemeId, $customShellThemes);

  $: applyTheme(activeShellTheme);

  function clearTheme(root: HTMLElement) {
    for (const v of THEME_CSS_VARS) root.style.removeProperty(v);
    for (const a of THEME_ATTRS) root.removeAttribute(a);
    root.style.removeProperty('font-size');
  }

  function applyTheme(theme: ShellTheme | null) {
    activeShellThemeId.set(theme?.id);
    const root = document.documentElement;
    // Always start from a clean slate so a property the previous theme set
    // (and this one doesn't) never lingers.
    clearTheme(root);
    if (!theme) return; // default look: app.css's own `:root` values take over
    const css = themeToCss(theme);
    for (const [k, v] of Object.entries(css.vars)) root.style.setProperty(k, v);
    for (const [k, v] of Object.entries(css.attrs)) root.setAttribute(k, v);
    if (css.rootFontSize) root.style.fontSize = css.rootFontSize;
  }

  onDestroy(() => {
    // Belt-and-suspenders — App.svelte mounts this for the app's whole
    // lifetime, but never leave inline overrides stuck on <html>.
    clearTheme(document.documentElement);
  });
</script>
