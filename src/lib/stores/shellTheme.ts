import { writable } from 'svelte/store';

/**
 * The active shell theme's id (e.g. `'hydra'`), or `undefined` when no
 * override is active (default theme / placeholder — see
 * `resolveActiveShellTheme` in `builtinThemes.ts`).
 *
 * Written to by `ShellThemeStyle.svelte` (the single place that already
 * decides what "active" means, shared with `App.svelte`'s own
 * `activeShellTheme` derivation) — nothing else should call `.set()` on
 * this.
 *
 * Exists so individual *app* components (Terminal, Explorer/Files, and
 * any future one) can react to the active shell theme without needing
 * it prop-drilled through `App.svelte` → `Window.svelte` →
 * `ErrorBoundary` → the app component — that chain doesn't currently
 * pass any shell-theme information into apps at all (checked — app
 * components are mounted via `<ErrorBoundary component={appDef.
 * component} props={{ windowId: win.id, ...win.launchArgs }} />`, no
 * theme id in that props object), and adding it there would mean every
 * app component's prop signature changes just to get one optional
 * string through. A subscribable store avoids that entirely: any app
 * that cares can `import { activeShellThemeId } from
 * '$lib/stores/shellTheme'` and use `$activeShellThemeId` directly.
 *
 * `terminalSession.ts` currently reads `document.documentElement
 * .dataset.shellTheme` directly instead of this store (a one-time,
 * non-reactive check at session creation, before this store existed) —
 * both approaches read the same underlying fact
 * (`ShellThemeStyle.svelte`'s `data-shell-theme` attribute and this
 * store are updated together, see that component), so there's no
 * correctness difference, just two different components independently
 * arriving at similar solutions to the same "how do I find out the
 * active theme without prop-drilling" problem. Newer app code should
 * prefer this store — it's reactive (a `$:` block re-runs if the theme
 * changes while the app is already open), which a one-time DOM read
 * isn't.
 */
export const activeShellThemeId = writable<string | undefined>(undefined);
