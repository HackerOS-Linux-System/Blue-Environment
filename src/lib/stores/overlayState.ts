import { writable, derived } from 'svelte/store';
import { activeDialog } from './dialog';

/**
 * App.svelte's own start menu / clipboard / control center /
 * notifications / power menu / alt-tab switcher open-state booleans are
 * plain component-local `let`s — fine for that component's own
 * template, but invisible to anything else. App.svelte mirrors them
 * into this store with a single reactive statement
 * (`$: shellOverlayOpen.set(...)`) so other components can observe
 * "is any of that currently open" without each of those booleans needing
 * to become its own exported store.
 */
export const shellOverlayOpen = writable(false);

/**
 * True while *any* UI surface that's supposed to render above every app
 * window is open — the shell overlays tracked in `shellOverlayOpen`
 * above, plus an in-shell prompt/confirm/alert dialog (`activeDialog`,
 * from `./dialog` — combined in here via `derived` specifically so
 * callers only need to import this one store, not both).
 *
 * Exists for `BlueWebApp.svelte`'s embedded-webview visibility gating —
 * see that file's module doc for why a native child webview needs this
 * at all: it's a separate OS-level surface that always paints on top of
 * this window's own DOM content, so none of these overlays' normal CSS
 * z-index would keep it underneath one otherwise. Hiding the webview
 * outright while any of this is open is the only correct fix given that
 * constraint.
 */
export const blockingOverlayOpen = derived(
  [shellOverlayOpen, activeDialog],
  ([$shell, $dialog]) => $shell || $dialog !== null
);

/**
 * True for the duration of a window drag / resize (set by Window.svelte).
 *
 * Why this exists: Blue Web's pages live in native child webviews that sit
 * ABOVE this shell's DOM. While the person drags or resizes an app window
 * the pointer can run over such a native surface (the window lags a frame
 * behind the cursor) — the shell's DOM then stops receiving `mousemove` /
 * `mouseup`, and the window gets "stuck" to the cursor. BlueWebApp.svelte
 * watches this flag and makes every embedded webview click-through
 * (`web_view_set_interactive`) until the drag/resize ends.
 */
export const windowInteracting = writable(false);
