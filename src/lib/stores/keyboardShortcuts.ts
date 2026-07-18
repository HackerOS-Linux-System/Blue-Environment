import { get } from 'svelte/store';
import {
  windows, activeWindowId, currentWorkspace, workspaceCount,
  focusWindow, minimizeWindow, closeWindow, maximizeWindow, switchWorkspace,
} from './windowManager';

export interface ShortcutCallbacks {
  onToggleStartMenu: () => void;
  onOpenFullScreenMenu: () => void;
  onToggleControlCenter: () => void;
  isSwitcherVisible: () => boolean;
  switcherIndex: () => number;
  setSwitcherVisible: (v: boolean) => void;
  setSwitcherIndex: (updater: (prev: number) => number) => void;
}

/** Call once from App.svelte's onMount; returns a cleanup function for onDestroy. */
export function initKeyboardShortcuts(cb: ShortcutCallbacks): () => void {
  const heldKeys = new Set<string>();

  function commitSwitcher() {
    const wins = get(windows);
    const idx = cb.switcherIndex();
    if (wins.length === 0) return;
    const selected = wins[idx % wins.length];
    if (selected) focusWindow(selected.id);
    cb.setSwitcherVisible(false);
  }

  function handleKeyDown(e: KeyboardEvent) {
    const key = e.key;
    heldKeys.add(key);

    const alt = e.altKey;
    const meta = e.metaKey || key === 'Meta';
    const ctrl = e.ctrlKey;
    const shift = e.shiftKey;
    const wins = get(windows);
    const svVisible = cb.isSwitcherVisible();

    if (alt && key === 'Tab') {
      e.preventDefault(); e.stopPropagation();
      if (wins.length === 0) return;
      if (!svVisible) {
        const activeId = get(activeWindowId);
        const currentIdx = wins.findIndex((w) => w.id === activeId);
        const startIdx = currentIdx !== -1 ? (currentIdx + 1) % wins.length : 0;
        cb.setSwitcherVisible(true);
        cb.setSwitcherIndex(() => startIdx);
      } else {
        cb.setSwitcherIndex((prev) => (prev + 1) % wins.length);
      }
      return;
    }

    if (alt && shift && key === 'Tab') {
      e.preventDefault(); e.stopPropagation();
      if (wins.length === 0) return;
      if (svVisible) cb.setSwitcherIndex((prev) => (prev - 1 + wins.length) % wins.length);
      return;
    }

    if (key === 'Meta') return;

    if (meta && key === 'Tab') { e.preventDefault(); e.stopPropagation(); cb.onOpenFullScreenMenu(); return; }

    if (meta && /^[1-9]$/.test(key)) {
      e.preventDefault();
      const num = parseInt(key, 10);
      if (num <= get(workspaceCount)) switchWorkspace(num - 1);
      return;
    }

    if (meta && key === 'ArrowRight') { e.preventDefault(); switchWorkspace(get(currentWorkspace) + 1); return; }
    if (meta && key === 'ArrowLeft') { e.preventDefault(); switchWorkspace(get(currentWorkspace) - 1); return; }
    if (meta && key === 'ArrowUp' && get(activeWindowId)) { e.preventDefault(); maximizeWindow(get(activeWindowId)!); return; }
    if (meta && key === 'ArrowDown' && get(activeWindowId)) { e.preventDefault(); minimizeWindow(get(activeWindowId)!); return; }

    if (alt && key === 'F4' && get(activeWindowId)) { e.preventDefault(); closeWindow(get(activeWindowId)!); return; }

    if (ctrl && alt && key === 't') { e.preventDefault(); window.dispatchEvent(new CustomEvent('blue:open-terminal')); return; }
    if (ctrl && shift && key === 'V') { e.preventDefault(); window.dispatchEvent(new CustomEvent('blue:toggle-clipboard')); return; }
    if (ctrl && alt && key === 'c') { e.preventDefault(); cb.onToggleControlCenter(); return; }
    if (key === 'PrintScreen') { e.preventDefault(); window.dispatchEvent(new CustomEvent('blue:screenshot')); return; }

    if (key === 'Escape') {
      if (svVisible) cb.setSwitcherVisible(false);
      window.dispatchEvent(new CustomEvent('blue:close-panels'));
      return;
    }

    if (alt && key === 'F1') { e.preventDefault(); cb.onToggleStartMenu(); return; }
    if (meta && key === 'l') { e.preventDefault(); window.dispatchEvent(new CustomEvent('blue:lock-screen')); return; }
    if (meta && key === 'd') { e.preventDefault(); window.dispatchEvent(new CustomEvent('blue:show-desktop')); return; }
  }

  function handleKeyUp(e: KeyboardEvent) {
    const key = e.key;
    heldKeys.delete(key);

    if (key === 'Alt' && cb.isSwitcherVisible()) { commitSwitcher(); return; }

    if (key === 'Meta') {
      const nonModifiers = [...heldKeys].filter((k) => !['Shift', 'Control', 'Alt', 'Meta'].includes(k));
      if (nonModifiers.length === 0) cb.onToggleStartMenu();
    }
  }

  window.addEventListener('keydown', handleKeyDown, { capture: true });
  window.addEventListener('keyup', handleKeyUp, { capture: true });

  return () => {
    window.removeEventListener('keydown', handleKeyDown, { capture: true });
    window.removeEventListener('keyup', handleKeyUp, { capture: true });
  };
}
