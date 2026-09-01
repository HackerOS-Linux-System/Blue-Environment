import { openApp } from '../stores/windowManager';
import { AppId } from '../types';

/**
 * Opens `url` in a fresh Blue Web window, via the same `launchArgs`
 * mechanism every app receives (`launchUrl`, read by
 * `BlueWebApp.svelte`'s `onMount`). Use this instead of shelling out to
 * `xdg-open`/the OS default browser for any in-shell "open this link"
 * action — keeps the person inside Blue Environment's own browser
 * (history, bookmarks, content blocking, all of it) rather than
 * bouncing out to whatever browser happens to be the OS default, which
 * may not even be installed in a minimal/live environment.
 *
 * Not used for genuinely external-to-the-shell needs (e.g. opening a
 * `mailto:` link, or a file:// path meant for a real file manager) —
 * those still have their own appropriate handlers elsewhere.
 */
export function openInBlueWeb(url: string): void {
  openApp(AppId.BLUE_WEB, false, undefined, { launchUrl: url });
}
