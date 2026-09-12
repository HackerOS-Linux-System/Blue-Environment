import { writable, derived, get } from 'svelte/store';
import type { WindowState, ExternalWindow } from '../types';
import { AppId } from '../types';
import { APPS } from '../constants';
import { SystemBridge } from '../utils/systemBridge';
import { configStore } from '../utils/configStore';
import { notificationManager } from '../utils/notificationManager';

export const windows = writable<WindowState[]>([]);
export const activeWindowId = writable<string | null>(null);
export const currentWorkspace = writable(0);
export const workspaceCount = writable(4);
export const externalWindows = writable<ExternalWindow[]>([]);

/**
 * A window the Alt+Tab switcher (and, potentially, any other
 * "list every open window" surface) can show and activate, regardless
 * of whether it's a real Blue Environment app window or an externally-
 * running native process window tracked via `window_tracker.rs`
 * (X11 wmctrl/xdotool, or the compositor's own foreign-toplevel list
 * over IPC for Wayland clients).
 *
 * This didn't exist before — `externalWindows` was polled into this
 * store (see `startExternalWindowPolling` below) but nothing in the
 * UI ever read it: Alt+Tab, the taskbar, and everywhere else only ever
 * showed `windows` (Blue Environment's own app windows). A native app
 * running alongside Blue Environment was completely invisible to
 * window switching, despite the backend already fully resolving a real
 * icon for it via `icon_resolver.rs`/`window_tracker.rs`.
 */
export interface SwitcherItem {
  id: string;
  title: string;
  isMinimized: boolean;
  workspace: number;
  isExternal: boolean;
  /** Set when `isExternal` is false — looked up against `APPS` for a
   * real (lucide-component) icon. */
  appId?: AppId;
  /** Set when `isExternal` is true — a `file://`/`http(s)://` URI
   * already resolved by the backend, rendered via `AppIconGlyph`
   * exactly like any other string-icon case it already handles. */
  iconPath?: string;
}

/** Single source of truth for "every window that can be switched to",
 * shared between keyboardShortcuts.ts's Alt+Tab handling and
 * WindowSwitcher.svelte's rendering, so the two can never drift out of
 * sync about what "window at index N" means. Takes both lists as plain
 * arguments (rather than reading `windows`/`externalWindows` via
 * `get()` internally) so callers stay in control of *when* it's
 * recomputed — in particular, App.svelte's `$: switcherItems =
 * getSwitcherItems($windows, $externalWindows)` needs `$windows`/
 * `$externalWindows` referenced directly for Svelte to know to re-run
 * it; a version that quietly called `get()` internally wouldn't be a
 * reactive dependency Svelte's compiler could see at all. */
export function getSwitcherItems(internalWindows: WindowState[], external: ExternalWindow[]): SwitcherItem[] {
  const internal: SwitcherItem[] = internalWindows.map((w) => ({
    id: w.id,
    title: w.title,
    isMinimized: w.isMinimized,
    workspace: w.workspace,
    isExternal: false,
    appId: w.appId as AppId,
  }));
  const externalItems: SwitcherItem[] = external.map((w) => ({
    id: w.id,
    title: w.title || w.class || 'External window',
    isMinimized: w.isMinimized,
    // External windows carry their own desktop/workspace tag from the
    // window manager/compositor rather than this shell's own
    // `currentWorkspace` store — 0 as a fallback keeps them visible on
    // the default workspace instead of vanishing if a tracker backend
    // ever fails to report one.
    workspace: w.desktop ?? 0,
    isExternal: true,
    iconPath: w.iconPath,
  }));
  return [...internal, ...externalItems];
}

/** Activates whatever `getSwitcherItems()` returned at `item.id` —
 * routes to the right backend depending on `isExternal` so callers
 * (keyboardShortcuts.ts, and potentially a future taskbar) don't need
 * to duplicate this branch themselves. */
export function activateSwitcherItem(item: SwitcherItem) {
  if (item.isExternal) {
    SystemBridge.focusExternalWindow(item.id);
  } else {
    focusWindow(item.id);
  }
}

let nextZIndex = 10;
let pollTimer: ReturnType<typeof setInterval> | undefined;

export const visibleWindows = derived(
  [windows, currentWorkspace],
  ([$windows, $currentWorkspace]) => $windows.filter((w) => w.workspace === $currentWorkspace || w.workspace === undefined)
);

/**
 * Which window is visually topmost right now, among windows actually
 * showing on the current workspace — accounting for `isPiP`, which
 * (per `Window.svelte`) always renders at a fixed CSS `z-index: 9998`
 * regardless of the window's own numeric `zIndex`, overriding the
 * normal stacking order. `null` when nothing is visible at all.
 *
 * Exists for `BlueWebApp.svelte`'s embedded-webview visibility gating:
 * a native child webview always paints on top of this window's own DOM
 * (see that file's module doc), so it needs to know explicitly whether
 * it's actually the topmost thing on screen right now, not just
 * whether it's "focused" in the usual sense — a window can be
 * `activeWindowId` and still be visually covered by a PiP window, for
 * instance.
 */
export const topmostVisibleWindowId = derived(visibleWindows, ($visible) => {
  const showing = $visible.filter((w) => !w.isMinimized);
  if (showing.length === 0) return null;
  const pip = showing.filter((w) => w.isPiP);
  const pool = pip.length > 0 ? pip : showing;
  return pool.reduce((top, w) => (w.zIndex > top.zIndex ? w : top)).id;
});

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

/**
 * Opens an app. `appId` starting with `clone:` is resolved against
 * `configStore`'s `clonedApps` (see ClonedAppEntry's doc for the
 * "Cloned Apps" feature and its current scope) — this is the single
 * place that resolution happens, so every existing launch surface
 * (Start Menu, Desktop icons, taskbar, search) already supports opening
 * a clone with zero changes of their own, as long as whatever calls
 * `openApp` was given `clone:<id>` as the id in the first place.
 */
export async function openApp(appId: string, isExternal = false, exec?: string, launchArgs?: Record<string, unknown>, titleOverride?: string) {
  if (appId.startsWith('clone:')) {
    const cloneId = appId.slice('clone:'.length);
    const entry = configStore.get().clonedApps?.find((c) => c.id === cloneId);
    if (!entry) return; // stale/deleted clone reference — nothing to open
    return openApp(entry.baseAppId, false, undefined, { ...launchArgs, cloneProfileId: entry.id }, entry.label);
  }

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
    title: titleOverride ?? appDef.title,
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

// ── Usage tracking (Parental Controls + Screen Time) ────────────────────
//
// `parental_controls_record_usage` existed in the backend but nothing
// ever called it, so daily time limits couldn't actually accumulate used
// time even once launch-blocking (above) was wired up — a limited app
// would be blockable at launch but never *reach* its limit from an
// already-running session. This polls the currently active window's
// `appId` every 60s and reports one more minute of usage for it.
//
// The same tick also feeds `screen_time_record_usage` (see
// screen_time.rs) — Settings' "Screen Time" section's permanent daily
// history. The two backends intentionally stay separate modules with
// different retention policies (Parental Controls resets at midnight;
// Screen Time never does on its own), but there's no reason to run two
// independent 60-second timers polling the same `activeWindowId`/
// `windows` state to feed them, so one tick reports to both.
let usageTrackingTimer: ReturnType<typeof setInterval> | undefined;

export function startParentalControlsUsageTracking() {
  if (usageTrackingTimer || !SystemBridge.isTauri()) return;
  usageTrackingTimer = setInterval(() => {
    const activeId = get(activeWindowId);
    if (!activeId) return;
    const win = get(windows).find((w) => w.id === activeId);
    if (!win || win.isMinimized) return; // don't bill time for a minimized/backgrounded app
    SystemBridge.invoke('parental_controls_record_usage', { appId: win.appId, minutes: 1 }).catch(() => {});
    SystemBridge.invoke('screen_time_record_usage', { appId: win.appId, minutes: 1 }).catch(() => {});
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
