import { writable, get } from 'svelte/store';

/**
 * Shell-wide context menu. Any component can open one with
 * `showContextMenu(event, items)`; the single <ContextMenu/> mounted in
 * App.svelte renders it above everything (windows can't clip it).
 */
export interface MenuItem {
  label?: string;
  icon?: any;                 // lucide component
  shortcut?: string;
  action?: () => unknown;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
  checked?: boolean;
  children?: MenuItem[];      // submenu
  /** Shown instead of a submenu while `children` is empty (e.g. "Loading…"). */
  emptyLabel?: string;
}

export interface MenuState { x: number; y: number; items: MenuItem[]; }

export const contextMenu = writable<MenuState | null>(null);

/** Incremented on every open — lets the global fallback know a component already handled the event. */
let serial = 0;
export function contextMenuSerial() { return serial; }

export function showContextMenu(e: MouseEvent | { clientX: number; clientY: number; preventDefault?: () => void }, items: MenuItem[]) {
  e.preventDefault?.();
  const clean = items.filter((it, i, arr) => !(it.separator && (i === 0 || i === arr.length - 1 || arr[i - 1]?.separator)));
  if (!clean.length) return;
  serial++;
  contextMenu.set({ x: e.clientX, y: e.clientY, items: clean });
}

export function closeContextMenu() { if (get(contextMenu)) contextMenu.set(null); }
