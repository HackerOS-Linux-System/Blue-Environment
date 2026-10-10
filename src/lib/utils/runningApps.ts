import type { ExternalWindow, WindowState } from '../types';

export interface RunningItem {
  /** Blue window id, or the compositor's id for a native (external) window. */
  id: string;
  title: string;
  /** Blue app id (icon comes from the app registry). */
  appId?: string;
  /** Native window's icon (`file://…`), when the tracker resolved one. */
  iconPath?: string;
  isExternal: boolean;
  isMinimized: boolean;
  isActive: boolean;
  workspace: number;
}

export const RUNNING_APPS_DEFAULT_MAX = 4;
export const RUNNING_APPS_MAX_LIMIT = 12;

/**
 * One entry per open window — Blue windows first (in opening order), then
 * native ones. A native window that Blue embeds in one of its own windows
 * (`externalWindowId`) is listed once, as that Blue window.
 */
export function buildRunningItems(
  blueWindows: WindowState[],
  nativeWindows: ExternalWindow[],
  activeWindowId: string | null,
): RunningItem[] {
  const embedded = new Set(blueWindows.map((w) => w.externalWindowId).filter((id): id is string => !!id));
  const blue: RunningItem[] = blueWindows.map((w) => ({
    id: w.id,
    title: w.title,
    appId: w.appId,
    isExternal: false,
    isMinimized: w.isMinimized,
    isActive: w.id === activeWindowId,
    workspace: w.workspace,
  }));
  const native: RunningItem[] = nativeWindows
    .filter((w) => !embedded.has(w.id))
    .map((w) => ({
      id: w.id,
      title: w.title || w.class || '',
      iconPath: w.iconPath || undefined,
      isExternal: true,
      isMinimized: w.isMinimized,
      isActive: false,
      workspace: w.desktop ?? 0,
    }));
  return [...blue, ...native];
}

/** Whole numbers inside `1..RUNNING_APPS_MAX_LIMIT`; anything else → the default. */
export function clampRunningMax(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return RUNNING_APPS_DEFAULT_MAX;
  return Math.min(RUNNING_APPS_MAX_LIMIT, Math.max(1, Math.round(value)));
}

/**
 * The first `max` items stay where they are (so icons don't jump around while
 * the person works), except that the focused window is never hidden behind the
 * "+N" chip: it takes the last visible slot.
 */
export function pickVisible(items: RunningItem[], max: number): { visible: RunningItem[]; hidden: number } {
  const limit = clampRunningMax(max);
  if (items.length <= limit) return { visible: items, hidden: 0 };
  const visible = items.slice(0, limit);
  const activeIdx = items.findIndex((i) => i.isActive);
  if (activeIdx >= limit) visible[limit - 1] = items[activeIdx];
  return { visible, hidden: items.length - limit };
}

export interface WorkspaceGroup {
  workspace: number;
  items: RunningItem[];
}

/** For the "all open apps" list: current workspace first, then the others in order. */
export function groupByWorkspace(items: RunningItem[], currentWorkspace: number): WorkspaceGroup[] {
  const map = new Map<number, RunningItem[]>();
  for (const it of items) {
    const list = map.get(it.workspace);
    if (list) list.push(it);
    else map.set(it.workspace, [it]);
  }
  return [...map.entries()]
    .sort(([a], [b]) => (a === currentWorkspace ? -1 : b === currentWorkspace ? 1 : a - b))
    .map(([workspace, groupItems]) => ({ workspace, items: groupItems }));
}
