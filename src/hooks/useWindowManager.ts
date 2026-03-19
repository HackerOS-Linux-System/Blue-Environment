import { useState, useCallback, useRef } from 'react';
import { WindowState, AppId } from '../types';
import { APPS } from '../constants';
import { SystemBridge } from '../utils/systemBridge';

export interface WindowManagerState {
    windows: WindowState[];
    activeWindowId: string | null;
    workspaces: number;
    currentWorkspace: number;
}

export function useWindowManager() {
    const [windows, setWindows] = useState<WindowState[]>([]);
    const [activeWindowId, setActiveWindowId] = useState<string | null>(null);
    const [currentWorkspace, setCurrentWorkspace] = useState(0);
    const [workspaceCount] = useState(4);
    const nextZIndex = useRef(10);

    // ------- Core WM operations -------

    const openApp = useCallback((appId: string, isExternal = false, exec?: string) => {
        if (isExternal && exec) {
            SystemBridge.launchApp(exec);
            return;
        }

        const appDef = APPS[appId as AppId];
        if (!appDef) return;

        // Jeśli aplikacja jest zewnętrzna (np. Blue Edit), uruchamiamy przez SystemBridge
        if (appDef.isExternal) {
            const execPath = appDef.externalPath ? `${appDef.externalPath}` : appId;
            SystemBridge.launchApp(execPath, appId);
            SystemBridge.recordAppLaunch(appId);
            return;
        }

        // W przeciwnym razie to wewnętrzna aplikacja React
        const zIndex = nextZIndex.current++;
        const newWindow: WindowState = {
            id: `${appId}-${Date.now()}`,
                                appId,
                                title: appDef.title,
                                x: 150 + ((windows.length % 8) * 30),
                                y: 100 + ((windows.length % 8) * 30),
                                width: appDef.defaultWidth ?? 800,
                                height: appDef.defaultHeight ?? 600,
                                isMinimized: false,
                                isMaximized: false,
                                zIndex,
                                isExternal: false,
                                workspace: currentWorkspace,
        };

        setWindows(prev => [...prev, newWindow]);
        setActiveWindowId(newWindow.id);
    }, [windows.length, currentWorkspace]);

    const closeWindow = useCallback((id: string) => {
        setWindows(prev => prev.filter(w => w.id !== id));
        setActiveWindowId(prev => prev === id ? null : prev);
    }, []);

    const focusWindow = useCallback((id: string) => {
        const zIndex = nextZIndex.current++;
        setActiveWindowId(id);
        setWindows(prev => prev.map(w => w.id === id ? { ...w, zIndex } : w));
    }, []);

    const minimizeWindow = useCallback((id: string) => {
        setWindows(prev => prev.map(w => w.id === id ? { ...w, isMinimized: true } : w));
        setActiveWindowId(prev => prev === id ? null : prev);
    }, []);

    const maximizeWindow = useCallback((id: string) => {
        setWindows(prev => prev.map(w => w.id === id ? { ...w, isMaximized: !w.isMaximized } : w));
        focusWindow(id);
    }, [focusWindow]);

    const moveWindow = useCallback((id: string, x: number, y: number) => {
        setWindows(prev => prev.map(w => w.id === id ? { ...w, x, y } : w));
    }, []);

    const resizeWindow = useCallback((id: string, width: number, height: number) => {
        setWindows(prev => prev.map(w => w.id === id ? { ...w, width, height } : w));
    }, []);

    const toggleWindowFromTaskbar = useCallback((id: string) => {
        setWindows(prev => {
            const win = prev.find(w => w.id === id);
            if (!win) return prev;

            if (win.isMinimized) {
                const zIndex = nextZIndex.current++;
                setActiveWindowId(id);
                return prev.map(w => w.id === id ? { ...w, isMinimized: false, zIndex } : w);
            }
            return prev;
        });

        setActiveWindowId(prevActive => {
            if (prevActive === id) {
                minimizeWindow(id);
                return null;
            }
            focusWindow(id);
            return id;
        });
    }, [focusWindow, minimizeWindow]);

    // ------- Workspace management -------

    const switchWorkspace = useCallback((index: number, total = workspaceCount) => {
        const next = ((index % total) + total) % total;
        setCurrentWorkspace(next);
        // Minimize all windows not on the new workspace
        setWindows(prev => prev.map(w => ({
            ...w,
            isMinimized: w.workspace !== next ? true : w.isMinimized,
        })));
        setActiveWindowId(null);
    }, [workspaceCount]);

    const moveWindowToWorkspace = useCallback((windowId: string, workspace: number) => {
        setWindows(prev => prev.map(w =>
        w.id === windowId ? { ...w, workspace } : w
        ));
    }, []);

    // ------- Visible windows (current workspace) -------

    const visibleWindows = windows.filter(
        w => w.workspace === currentWorkspace || w.workspace === undefined
    );

    return {
        windows,
        visibleWindows,
        activeWindowId,
        currentWorkspace,
        workspaceCount,
        openApp,
        closeWindow,
        focusWindow,
        minimizeWindow,
        maximizeWindow,
        moveWindow,
        resizeWindow,
        toggleWindowFromTaskbar,
        switchWorkspace,
            moveWindowToWorkspace,
    };
}
