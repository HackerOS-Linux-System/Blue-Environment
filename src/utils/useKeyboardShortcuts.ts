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

    const heldKeys = useRef<Set<string>>(new Set());
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
        const safeIdx = idx % wins.length;
        const selected = wins[safeIdx];
        if (selected) {
            focusWindow(selected.id);
        }
        setSwitcherVisible(false);
    }, [focusWindow, setSwitcherVisible]);

    const handleKeyDown = useCallback((e: KeyboardEvent) => {
        const key = e.key;
        heldKeys.current.add(key);

        const alt = e.altKey;
        const meta = e.metaKey || key === 'Meta';
        const ctrl = e.ctrlKey;
        const shift = e.shiftKey;

        const wins = windowsRef.current;
        const svRef = switcherVisibleRef.current;

        // Alt+Tab
        if (alt && key === 'Tab') {
            e.preventDefault();
            e.stopPropagation();

            if (wins.length === 0) return;

            if (!svRef) {
                const activeId = activeRef.current;
                const currentIdx = wins.findIndex(w => w.id === activeId);
                const startIdx = currentIdx !== -1 ? currentIdx : 0;
                setSwitcherVisible(true);
                setSwitcherIndex(startIdx);
            } else {
                setSwitcherIndex(prev => (prev + 1) % wins.length);
            }
            return;
        }

        // Alt+Shift+Tab
        if (alt && shift && key === 'Tab') {
            e.preventDefault();
            e.stopPropagation();

            if (wins.length === 0) return;

            if (svRef) {
                setSwitcherIndex(prev => (prev - 1 + wins.length) % wins.length);
            }
            return;
        }

        // Meta key itself – handled on keyup
        if (key === 'Meta') {
            return;
        }

        // Win+Tab -> full-screen menu
        if (meta && key === 'Tab') {
            e.preventDefault();
            e.stopPropagation();
            onOpenFullScreenMenu();
            return;
        }

        // Win + number -> switch workspace
        if (meta && /^[1-9]$/.test(key)) {
            e.preventDefault();
            const num = parseInt(key, 10);
            if (num <= workspaceCount) {
                switchWorkspace(num - 1);
            }
            return;
        }

        // Win+Left/Right
        if (meta && key === 'ArrowRight') {
            e.preventDefault();
            switchWorkspace(currentWorkspaceRef.current + 1);
            return;
        }
        if (meta && key === 'ArrowLeft') {
            e.preventDefault();
            switchWorkspace(currentWorkspaceRef.current - 1);
            return;
        }

        // Win+Up -> maximize
        if (meta && key === 'ArrowUp' && activeRef.current) {
            e.preventDefault();
            maximizeWindow(activeRef.current);
            return;
        }

        // Win+Down -> minimize
        if (meta && key === 'ArrowDown' && activeRef.current) {
            e.preventDefault();
            minimizeWindow(activeRef.current);
            return;
        }

        // Alt+F4 -> close
        if (alt && key === 'F4' && activeRef.current) {
            e.preventDefault();
            closeWindow(activeRef.current);
            return;
        }

        // Ctrl+Alt+T -> terminal
        if (ctrl && alt && key === 't') {
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('blue:open-terminal'));
            return;
        }

        // Ctrl+Shift+V -> clipboard
        if (ctrl && shift && key === 'V') {
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('blue:toggle-clipboard'));
            return;
        }

        // Escape
        if (key === 'Escape') {
            if (svRef) {
                setSwitcherVisible(false);
            }
            window.dispatchEvent(new CustomEvent('blue:close-panels'));
            return;
        }

        // Alt+F1 -> start menu
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
        heldKeys.current.delete(key);

        // Alt release -> commit switcher
        if (key === 'Alt' && switcherVisibleRef.current) {
            commitSwitcher();
            return;
        }

        // Meta release alone -> toggle start menu
        if (key === 'Meta') {
            const nonModifiers = [...heldKeys.current].filter(k => !['Shift', 'Control', 'Alt', 'Meta'].includes(k));
            if (nonModifiers.length === 0) {
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
