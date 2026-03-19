import { useEffect, useCallback, useRef } from 'react';
import { WindowState } from '../types';

interface KeyboardShortcutsConfig {
    windows: WindowState[];
    activeWindowId: string | null;
    currentWorkspace: number;
    workspaceCount: number;

    // WM actions
    focusWindow: (id: string) => void;
    minimizeWindow: (id: string) => void;
    closeWindow: (id: string) => void;
    maximizeWindow: (id: string) => void;
    switchWorkspace: (index: number) => void;

    // UI panels
    onToggleStartMenu: () => void;
    onOpenFullScreenMenu: () => void;
    onToggleControlCenter: () => void;

    // Switcher state
    isSwitcherVisible: boolean;
    switcherSelectedIndex: number;
    setSwitcherVisible: (v: boolean) => void;
    setSwitcherIndex: (idx: number | ((prev: number) => number)) => void;
}

export function useKeyboardShortcuts({
    windows,
    activeWindowId,
    currentWorkspace,
    workspaceCount,
    focusWindow,
    minimizeWindow,
    closeWindow,
    maximizeWindow,
    switchWorkspace,
    onToggleStartMenu,
    onOpenFullScreenMenu,
    onToggleControlCenter,
    isSwitcherVisible,
    switcherSelectedIndex,
    setSwitcherVisible,
    setSwitcherIndex,
}: KeyboardShortcutsConfig) {

    // Track held keys to handle combos properly
    const heldKeys = useRef<Set<string>>(new Set());
    // Store latest windows in ref to avoid stale closures in event listeners
    const windowsRef = useRef(windows);
    windowsRef.current = windows;
    const activeRef = useRef(activeWindowId);
    activeRef.current = activeWindowId;
    const switcherVisibleRef = useRef(isSwitcherVisible);
    switcherVisibleRef.current = isSwitcherVisible;
    const switcherIndexRef = useRef(switcherSelectedIndex);
    switcherIndexRef.current = switcherSelectedIndex;
    const currentWorkspaceRef = useRef(currentWorkspace);
    currentWorkspaceRef.current = currentWorkspace;

    const commitSwitcher = useCallback(() => {
        const wins = windowsRef.current;
        const idx = switcherIndexRef.current;
        if (wins.length === 0) return;
        const selected = wins[idx % wins.length];
        if (selected) {
            focusWindow(selected.id);
            if (selected.isMinimized) {
                // un-minimize via focusWindow side-effect handled in WM
            }
        }
        setSwitcherVisible(false);
    }, [focusWindow, setSwitcherVisible]);

    const handleKeyDown = useCallback((e: KeyboardEvent) => {
        const key = e.key;
        heldKeys.current.add(key);

        const alt = e.altKey;
        const meta = e.metaKey || e.key === 'Meta';
        const ctrl = e.ctrlKey;
        const shift = e.shiftKey;
        const wins = windowsRef.current;
        const activeId = activeRef.current;
        const svRef = switcherVisibleRef.current;
        const siRef = switcherIndexRef.current;
        const wsCount = workspaceCount;
        const cws = currentWorkspaceRef.current;

        // ── Alt+Tab ─────────────────────────────────────────
        if (alt && key === 'Tab') {
            e.preventDefault();
            if (wins.length === 0) return;

            if (!svRef) {
                const currentIdx = wins.findIndex(w => w.id === activeId);
                const nextIdx = (currentIdx + 1) % wins.length;
                setSwitcherVisible(true);
                setSwitcherIndex(nextIdx);
            } else {
                setSwitcherIndex(prev => (prev + 1) % wins.length);
            }
            return;
        }

        // ── Alt+Shift+Tab (backwards) ────────────────────────
        if (alt && shift && key === 'Tab') {
            e.preventDefault();
            if (wins.length === 0) return;
            setSwitcherIndex(prev => (prev - 1 + wins.length) % wins.length);
            return;
        }

        // ── Super/Meta key (Win key) ─────────────────────────
        if (key === 'Meta') {
            // Handled on keyup to distinguish press vs combo
            return;
        }

        // ── Super + number → switch workspace ───────────────
        if (alt && e.metaKey && /^[1-4]$/.test(key)) {
            e.preventDefault();
            switchWorkspace(parseInt(key) - 1);
            return;
        }

        // ── Super + Tab → full-screen app switcher ──────────
        if (e.metaKey && key === 'Tab') {
            e.preventDefault();
            onOpenFullScreenMenu();
            return;
        }

        // ── Super + Left/Right → switch workspace ───────────
        if (e.metaKey && key === 'ArrowRight') {
            e.preventDefault();
            switchWorkspace(cws + 1);
            return;
        }
        if (e.metaKey && key === 'ArrowLeft') {
            e.preventDefault();
            switchWorkspace(cws - 1);
            return;
        }

        // ── Super + Up → maximize active window ─────────────
        if (e.metaKey && key === 'ArrowUp' && activeId) {
            e.preventDefault();
            maximizeWindow(activeId);
            return;
        }

        // ── Super + Down → minimize active window ────────────
        if (e.metaKey && key === 'ArrowDown' && activeId) {
            e.preventDefault();
            minimizeWindow(activeId);
            return;
        }

        // ── Alt+F4 → close active window ─────────────────────
        if (alt && key === 'F4' && activeId) {
            e.preventDefault();
            closeWindow(activeId);
            return;
        }

        // ── Ctrl+Alt+T → open terminal ──────────────────────
        // (handled in App.tsx via openApp but we can emit event)
        if (ctrl && alt && key === 't') {
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('blue:open-terminal'));
            return;
        }

        // ── Escape → close switcher / close menus ───────────
        if (key === 'Escape') {
            if (svRef) {
                setSwitcherVisible(false);
            }
            window.dispatchEvent(new CustomEvent('blue:close-panels'));
            return;
        }

        // ── Alt+F1 → toggle start menu ──────────────────────
        if (alt && key === 'F1') {
            e.preventDefault();
            onToggleStartMenu();
            return;
        }

    }, [
        workspaceCount,
        setSwitcherVisible,
        setSwitcherIndex,
        switchWorkspace,
        maximizeWindow,
        minimizeWindow,
        closeWindow,
        onToggleStartMenu,
        onOpenFullScreenMenu,
    ]);

    const handleKeyUp = useCallback((e: KeyboardEvent) => {
        const key = e.key;
        const wasHeld = heldKeys.current.has('Meta') || key === 'Meta';
        heldKeys.current.delete(key);

        // ── Alt release → commit Alt+Tab selection ───────────
        if (key === 'Alt' && switcherVisibleRef.current) {
            commitSwitcher();
            return;
        }

        // ── Meta release alone (no combo) → start menu ───────
        if (key === 'Meta' && wasHeld) {
            // Only toggle if no other key was pressed with Meta
            const hadCombo = [...heldKeys.current].some(k => k !== 'Meta');
            if (!hadCombo) {
                onToggleStartMenu();
            }
        }
    }, [commitSwitcher, onToggleStartMenu]);

    useEffect(() => {
        window.addEventListener('keydown', handleKeyDown, { capture: true });
        window.addEventListener('keyup', handleKeyUp, { capture: true });
        return () => {
            window.removeEventListener('keydown', handleKeyDown, { capture: true });
            window.removeEventListener('keyup', handleKeyUp, { capture: true });
        };
    }, [handleKeyDown, handleKeyUp]);
}
