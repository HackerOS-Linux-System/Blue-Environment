<script lang="ts">
  /**
   * Applies the active shell theme (see builtinThemes.ts) to the running
   * UI. This is the piece that was explicitly missing before — earlier
   * revisions of the theme system had selection, persistence, and the
   * restart-prompt flow (ThemesSection.svelte) but nothing that actually
   * changed how anything *looked*. Mount once, near the root (App.svelte
   * does this, right after the wallpaper/panel-position setup those also
   * now defer to the active theme for).
   *
   * ── What this actually does ──────────────────────────────────────────
   * Sets `--bg-primary`/`--bg-secondary`/`--text-primary`/
   * `--text-secondary`/`--accent`/`--accent-hover` as inline custom
   * properties on `documentElement` whenever the active theme is a real
   * (non-placeholder) one with `colors` — these are the *same* variable
   * names `src/app.css`'s existing `.theme-bg-primary` /
   * `.theme-text-primary` / etc. utility classes already read from (see
   * that file — `Window.svelte` and others already use those classes
   * throughout), so applying a shell theme's colors automatically
   * reaches every component already using that established convention,
   * with no per-component changes needed.
   *
   * Deliberately *inline* `style.setProperty` on `documentElement`
   * rather than a new `[data-shell-theme='hydra']` block in app.css the
   * way `[data-theme='cyberpunk']` etc. already work there — those
   * existing blocks are keyed off `data-theme`, which is bound to
   * `UserConfig.theme` (dark/light mode), a *different* setting from
   * `shellThemeId` (checked directly — `App.svelte`'s `data-theme={theme}`
   * reads `cfg.theme`, never `cfg.themeName` or a shell theme id, despite
   * `app.css` having unused-looking `[data-theme='cyberpunk']` blocks
   * that suggest otherwise — a separate, pre-existing gap, not something
   * this component's job to fix). Reusing `data-theme` for shell themes
   * too would conflate two independent settings; inline properties on
   * `documentElement` avoid that entirely without touching app.css or
   * the dark/light mode wiring.
   *
   * ── What this does NOT do yet (honest scope) ─────────────────────────
   * - Window-control button restyling (`layout.windowControlsStyle`/
   *   `windowControlsPosition`) — `Window.svelte`'s button markup is
   *   still hardcoded (Minimize/PiP/Maximize/Close, fixed right-aligned
   *   order). A real follow-up, not attempted here.
   * - `layout.cornerStyle`/`iconStyle` — read from the active theme
   *   (exported below via `activeShellTheme` for anything that wants
   *   them) but nothing currently consumes them.
   * - Per-component palette beyond the 6 CSS variables above (surface/
   *   surfaceElevated/border from `ShellThemeColors` aren't mapped to
   *   anything — `app.css`'s existing variable set doesn't have slots
   *   for them). Real follow-up if a theme ever needs finer-grained
   *   control than "one background, one secondary background, one
   *   accent" — Hydra's current look doesn't need it.
   */
  import { onDestroy } from 'svelte';
  import { resolveActiveShellTheme, type ShellTheme } from '../data/builtinThemes';
  import { activeShellThemeId } from '../stores/shellTheme';

  // Only `shellThemeId` is ever read — a bare string prop rather than
  // the full `UserConfig` type, so callers (App.svelte specifically)
  // don't need to thread an entire config object through just for this.
  export let shellThemeId: string | undefined = undefined;

  $: activeShellTheme = resolveActiveShellTheme(shellThemeId);

  $: applyTheme(activeShellTheme);

  function applyTheme(theme: ShellTheme | null) {
    activeShellThemeId.set(theme?.id);
    const root = document.documentElement;
    if (!theme) {
      // Reverting to default — remove our overrides so app.css's own
      // `:root` defaults (or whatever `data-theme` block is active for
      // dark/light mode) take back over, rather than leaving a stale
      // inline override behind that a CSS custom-property removal is
      // the only way to undo (setting them back to app.css's own
      // literal default values would silently go stale the next time
      // someone changes app.css's `:root` block).
      root.style.removeProperty('--bg-primary');
      root.style.removeProperty('--bg-secondary');
      root.style.removeProperty('--text-primary');
      root.style.removeProperty('--text-secondary');
      root.style.removeProperty('--accent');
      root.style.removeProperty('--accent-hover');
      root.style.removeProperty('--shell-radius');
      root.removeAttribute('data-shell-theme');
      root.removeAttribute('data-icon-style');
      return;
    }
    root.style.setProperty('--bg-primary', theme.colors.background);
    root.style.setProperty('--bg-secondary', theme.colors.surface);
    root.style.setProperty('--text-primary', theme.colors.text);
    root.style.setProperty('--text-secondary', theme.colors.textMuted);
    root.style.setProperty('--accent', theme.colors.accent);
    root.style.setProperty('--accent-hover', theme.colors.accent);
    // `cornerStyle`/`iconStyle` — previously defined on every theme's
    // `layout` and read by nothing at all. `--shell-radius` is the
    // concrete hook: `sharp` themes set it to `0px`, `rounded` themes
    // don't set it (falls through to whatever each component's own
    // Tailwind class already specifies, e.g. `rounded-xl` — see
    // Window.svelte's own use of `var(--shell-radius, ...)` for exactly
    // this fallback pattern). `iconStyle` doesn't have an equivalent
    // single CSS hook (Lucide icons are outline-only in this app's
    // icon set — see this component's own module doc for why `filled`
    // isn't wired to anything yet) but the attribute is still set below
    // so a future per-icon check (`data-shell-theme` + `iconStyle`
    // read from `resolveActiveShellTheme` directly, same as this
    // component already does) has something to key off without this
    // file changing again first.
    if (theme.layout.cornerStyle === 'sharp') {
      root.style.setProperty('--shell-radius', '0px');
    } else {
      root.style.removeProperty('--shell-radius');
    }
    // `iconStyle` — same "was defined, nothing read it" gap as
    // `cornerStyle` had. This app's whole icon set (lucide-svelte) is
    // outline-only by construction — there's no separate "filled"
    // icon asset to swap to the way a real icon font/pack with both
    // variants would have. The real, honest hook this can offer
    // without a second icon set: a CSS-only "soft fill" — a subtle
    // tinted background fill behind the outline glyph — via the
    // `data-icon-style` attribute set below, consumed by
    // `.panel-icon-btn`'s own rule in app.css. Not equivalent to true
    // filled icon glyphs; flagged as the honest ceiling of what's
    // achievable without adding a second icon asset set.
    root.setAttribute('data-icon-style', theme.layout.iconStyle);
    // Not read by app.css today (see this component's module doc) —
    // set anyway so any component that wants to opt into
    // theme-specific styling beyond the 6 mapped variables can do so
    // via `[data-shell-theme='hydra']` selectors without needing this
    // component changed again first. TopBar.svelte's gradient panel
    // background does exactly that (a `shellTheme` prop, not a CSS
    // attribute selector, but same underlying id).
    root.setAttribute('data-shell-theme', theme.id);
  }

  onDestroy(() => {
    // Belt-and-suspenders — if this component is ever conditionally
    // unmounted (it isn't today, App.svelte mounts it unconditionally
    // for the app's whole lifetime), don't leave inline style overrides
    // stuck on documentElement forever.
    applyTheme(null);
  });
</script>
