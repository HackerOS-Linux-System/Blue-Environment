import { writable, get } from 'svelte/store';
import { SystemBridge } from './systemBridge';

/**
 * Blue Installer "live mode" contract:
 *   - If `~/.config/Blue-Environment/.live` exists at boot, BEDM auto-logs
 *     the owning user in (see BEDM/bedm-daemon/src/users.rs::find_live_user)
 *     and Blue Environment itself (this check) shows Blue Installer
 *     full-screen instead of the normal desktop.
 *   - Blue Installer deletes that file once installation completes, so the
 *     *next* boot is a normal desktop session with a real login prompt.
 *   - Blue Installer is intentionally NOT registered in constants.ts/APPS —
 *     it's not meant to be launchable from the Start Menu or Settings, only
 *     ever entered via this marker file, matching how a real live-ISO
 *     installer behaves.
 *   - While in live mode the person can flip between two views
 *     (`liveView`): the installer, and the ordinary ("classic") Blue
 *     desktop to look around / use a browser / partition manually. The
 *     installer stays mounted (just hidden) while the desktop is shown, so
 *     nothing they entered is lost, and the desktop shows a floating
 *     "Return to Blue Installer" button. None of this exists outside live
 *     mode: without the `.live` marker there is no installer view at all.
 */
export const isLiveMode = writable(false);
export const liveModeChecked = writable(false);

/** Which view a live session currently shows. Meaningless outside live mode. */
export type LiveView = 'installer' | 'desktop';
export const liveView = writable<LiveView>('installer');

/** Live mode only: leave the installer and show the classic desktop. */
export function switchToClassicDesktop(): void {
  if (get(isLiveMode)) liveView.set('desktop');
}

/** Live mode only: go back from the classic desktop to the installer. */
export function returnToInstaller(): void {
  if (get(isLiveMode)) liveView.set('installer');
}

export async function checkLiveMode(): Promise<boolean> {
  try {
    const result = await SystemBridge.executeCommand('test -f "$HOME/.config/Blue-Environment/.live" && echo yes || echo no');
    const out = typeof result === 'string' ? result : (result as any)?.stdout ?? '';
    const live = out.trim() === 'yes';
    isLiveMode.set(live);
    return live;
  } catch {
    isLiveMode.set(false);
    return false;
  } finally {
    liveModeChecked.set(true);
  }
}

/** Called by Blue Installer once installation finishes successfully. */
export async function clearLiveMode(): Promise<void> {
  await SystemBridge.executeCommand('rm -f "$HOME/.config/Blue-Environment/.live"').catch(() => {});
  isLiveMode.set(false);
  liveView.set('installer');
}
