import { configStore } from '../utils/configStore';
import { initLanguage } from '../stores/language';
import {
  startExternalWindowPolling, stopExternalWindowPolling,
  startParentalControlsUsageTracking, stopParentalControlsUsageTracking,
} from '../stores/windowManager';
import { startCalendarReminders } from '../components/apps/Blue-Calendar-App/calendarReminders';
import { createNotificationsStore } from '../components/apps/Blue-Notifications-App/notificationsStore';
import { hideSplash, hideSplashAfter } from '../utils/splash';

/**
 * Services every alternative shell needs, the same ones `App.svelte` starts
 * for the desktop shell: config, language, external-window polling, parental
 * controls, calendar reminders and the notification feed watcher. Returns a
 * cleanup function. Idempotent enough to call once per shell root.
 *
 * Deliberately NOT started here (desktop-only): the hot corners, the window
 * switcher, global context menu and the first-run welcome window.
 */
export function startAltShell(onConfig?: (cfg: ReturnType<typeof configStore.get>) => void): () => void {
  hideSplashAfter(10000);
  initLanguage();
  startExternalWindowPolling();
  startParentalControlsUsageTracking();
  createNotificationsStore().startPolling();
  const stopReminders = startCalendarReminders();

  const ready = configStore.init();
  ready.then((cfg) => { onConfig?.(cfg); }).finally(() => hideSplash());
  const unsub = configStore.subscribe((cfg) => onConfig?.(cfg));

  return () => {
    unsub();
    stopReminders?.();
    stopExternalWindowPolling();
    stopParentalControlsUsageTracking();
  };
}
