import { writable, derived, get } from 'svelte/store';
import type { WindowState, ExternalWindow } from '../types';
import { AppId } from '../types';
import { APPS } from '../constants';
import { SystemBridge } from '../utils/systemBridge';
import { notificationManager } from '../utils/notificationManager';

export const windows = writable<WindowState[]>([]);
export const activeWindowId = writable<string | null>(null);
export const currentWorkspace = writable(0);
export const workspaceCount = writable(4);
export const externalWindows = writable<ExternalWindow[]>([]);

let nextZIndex = 10;
let pollTimer: ReturnType<typeof setInterval> | undefined;

export const visibleWindows = derived(
  [windows, currentWorkspace],
  ([$windows, $currentWorkspace]) => $windows.filter((w) => w.workspace === $currentWorkspace || w.workspace === undefined)
);

export function startExternalWindowPolling() {
  if (pollTimer) return;
  const poll = async () => {
    try {
      externalWindows.set(await SystemBridge.getExternalWindows());
    } catch {
      /* no external window tracking in this session */
    }
  };
  poll();
  pollTimer = setInterval(poll, 2000);
}

export function stopExternalWindowPolling() {
  clearInterval(pollTimer);
  pollTimer = undefined;
}

export async function openApp(appId: string, isExternal = false, exec?: string, launchArgs?: Record<string, unknown>) {
  const blockReason = await checkParentalControls(appId);
  if (blockReason) {
    notificationManager.add({
      title: 'Blocked by Parental Controls',
      message: blockReason,
      appId: 'settings',
      icon: '',
    });
    return;
  }

  if (isExternal && exec) {
    SystemBridge.launchApp(exec);
    return;
  }

  const appDef = APPS[appId as AppId];
  if (!appDef) return;

  if (appDef.isExternal) {
    const execPath = appDef.externalPath ? appDef.externalPath : appId;
    SystemBridge.launchApp(execPath, appId);
    SystemBridge.recordAppLaunch(appId);
    return;
  }

  const wins = get(windows);
  const ws = get(currentWorkspace);
  const zIndex = nextZIndex++;
  const newWindow: WindowState = {
    id: `${appId}-${Date.now()}`,
    appId,
    title: appDef.title,
    x: 150 + (wins.length % 8) * 30,
    y: 100 + (wins.length % 8) * 30,
    width: appDef.defaultWidth ?? 800,
    height: appDef.defaultHeight ?? 600,
    isMinimized: false,
    isMaximized: false,
    zIndex,
    isExternal: false,
    workspace: ws,
    ...(launchArgs ? { launchArgs } : {}),
  };

  windows.update((w) => [...w, newWindow]);
  activeWindowId.set(newWindow.id);
}

/**
 * Checks Parental Controls before letting an app open. Previously
 * `parental_controls_check_launch` existed in the Rust backend and did
 * nothing wrong logically, but nothing anywhere ever called it — a
 * blocked app would open exactly as normal. This is the single call site
 * every app-open path in the shell funnels through (`openApp` is what the
 * dock, launcher, taskbar, and `Ctrl+Space`-style search all ultimately
 * call), so checking here covers both internal Svelte-component "apps"
 * and real external OS processes in one place.
 *
 * Returns a human-readable block reason, or `null` if the app may open.
 * Fails open (returns `null`) if the backend call itself fails, e.g.
 * because Parental Controls isn't configured yet — that's the existing
 * behavior of `parental_controls_check_launch` itself (returns `None`
 * whenever `cfg.enabled` is false), duplicated here only so a network/IPC
 * hiccup doesn't accidentally lock a child (or anyone) out of every app.
 */
async function checkParentalControls(appId: string): Promise<string | null> {
  if (!SystemBridge.isTauri()) return null;
  try {
    return (await SystemBridge.invoke<string | null>('parental_controls_check_launch', { appId })) ?? null;
  } catch {
    return null;
  }
}

// ── Parental Controls usage tracking ────────────────────────────────────
//
// `parental_controls_record_usage` existed in the backend but nothing
// ever called it, so daily time limits couldn't actually accumulate used
// time even once launch-blocking (above) was wired up — a limited app
// would be blockable at launch but never *reach* its limit from an
// already-running session. This polls the currently active window's
// `appId` every 60s and reports one more minute of usage for it.
let usageTrackingTimer: ReturnType<typeof setInterval> | undefined;

export function startParentalControlsUsageTracking() {
  if (usageTrackingTimer || !SystemBridge.isTauri()) return;
  usageTrackingTimer = setInterval(() => {
    const activeId = get(activeWindowId);
    if (!activeId) return;
    const win = get(windows).find((w) => w.id === activeId);
    if (!win || win.isMinimized) return; // don't bill time for a minimized/backgrounded app
    SystemBridge.invoke('parental_controls_record_usage', { appId: win.appId, minutes: 1 }).catch(() => {});
  }, 60_000);
}

export function stopParentalControlsUsageTracking() {
  clearInterval(usageTrackingTimer);
  usageTrackingTimer = undefined;
}

export function embedExternalWindow(extWin: ExternalWindow) {
  const wins = get(windows);
  const existing = wins.find((w) => w.externalWindowId === extWin.id);
  if (existing) {
    focusWindow(existing.id);
    return;
  }

  const ws = get(currentWorkspace);
  const zIndex = nextZIndex++;
  const newWindow: WindowState = {
    id: `external-${extWin.id}-${Date.now()}`,
    appId: AppId.EXTERNAL,
    title: extWin.title || extWin.class || 'External App',
    x: 200 + (wins.length % 5) * 30,
    y: 150 + (wins.length % 5) * 30,
    width: 900,
    height: 650,
    isMinimized: extWin.isMinimized,
    isMaximized: false,
    zIndex,
    isExternal: true,
    workspace: ws,
    externalWindowId: extWin.id,
    pid: extWin.pid,
  };

  windows.update((w) => [...w, newWindow]);
  activeWindowId.set(newWindow.id);
  SystemBridge.embedExternalWindow(extWin.id, newWindow.id);
}

export function closeWindow(id: string) {
  const win = get(windows).find((w) => w.id === id);
  if (win?.isExternal && win.externalWindowId) {
    SystemBridge.closeExternalWindow(win.externalWindowId);
  }
  windows.update((w) => w.filter((x) => x.id !== id));
  activeWindowId.update((cur) => (cur === id ? null : cur));
}

export function focusWindow(id: string) {
  const zIndex = nextZIndex++;
  activeWindowId.set(id);
  windows.update((w) => w.map((x) => (x.id === id ? { ...x, zIndex, isMinimized: false } : x)));

  const win = get(windows).find((w) => w.id === id);
  if (win?.isExternal && win.externalWindowId) {
    SystemBridge.focusExternalWindow(win.externalWindowId);
  }
}

export function minimizeWindow(id: string) {
  const win = get(windows).find((w) => w.id === id);
  if (win?.isExternal && win.externalWindowId) {
    SystemBridge.minimizeExternalWindow(win.externalWindowId);
  }
  windows.update((w) => w.map((x) => (x.id === id ? { ...x, isMinimized: true } : x)));
  activeWindowId.update((cur) => (cur === id ? null : cur));
}

export function maximizeWindow(id: string) {
  windows.update((w) => w.map((x) => (x.id === id ? { ...x, isMaximized: !x.isMaximized, isPiP: false } : x)));
  focusWindow(id);
}

/**
 * Picture-in-Picture — Android-style "float this window on top of
 * everything, small, in a corner, always visible" mode. Distinct from
 * maximize/minimize: a PiP window stays visible and interactive while every
 * other window behaves normally underneath it. Toggling it back off
 * restores the window's previous geometry.
 */
export function togglePiP(id: string) {
  windows.update((w) =>
    w.map((x) => {
      if (x.id !== id) return x;
      if (x.isPiP) {
        // Restore
        return { ...x, isPiP: false, ...(x.prePiPGeometry ?? {}), prePiPGeometry: undefined };
      }
      const corner = { x: 24, y: 24, width: 360, height: 220 };
      return {
        ...x,
        isPiP: true,
        isMaximized: false,
        prePiPGeometry: { x: x.x, y: x.y, width: x.width, height: x.height },
        ...corner,
      };
    })
  );
  focusWindow(id);
}

export function moveWindow(id: string, x: number, y: number) {
  windows.update((w) => w.map((win) => (win.id === id ? { ...win, x, y } : win)));
}

export function resizeWindow(id: string, width: number, height: number) {
  windows.update((w) => w.map((win) => (win.id === id ? { ...win, width, height } : win)));
}

export function toggleWindowFromTaskbar(id: string) {
  const win = get(windows).find((w) => w.id === id);
  if (!win) return;

  if (win.isMinimized) {
    const zIndex = nextZIndex++;
    windows.update((w) => w.map((x) => (x.id === id ? { ...x, isMinimized: false, zIndex } : x)));
    activeWindowId.set(id);
    if (win.isExternal && win.externalWindowId) {
      SystemBridge.focusExternalWindow(win.externalWindowId);
    }
  } else if (get(activeWindowId) === id) {
    minimizeWindow(id);
  } else {
    focusWindow(id);
  }
}

export function switchWorkspace(index: number) {
  const total = get(workspaceCount);
  const next = ((index % total) + total) % total;
  currentWorkspace.set(next);
  windows.update((w) => w.map((win) => ({ ...win, isMinimized: win.workspace !== next ? true : win.isMinimized })));
  activeWindowId.set(null);
}

export function moveWindowToWorkspace(windowId: string, workspace: number) {
  windows.update((w) => w.map((win) => (win.id === windowId ? { ...win, workspace } : win)));
}
