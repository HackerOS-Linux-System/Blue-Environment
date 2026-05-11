import { useState, useCallback, useRef, useEffect } from 'react';
import { WindowState, AppId, ExternalWindow } from '../types';
import { APPS } from '../constants';
import { SystemBridge } from '../utils/systemBridge';

export function useWindowManager() {
    const [windows, setWindows] = useState<WindowState[]>([]);
    const [activeWindowId, setActiveWindowId] = useState<string | null>(null);
    const [currentWorkspace, setCurrentWorkspace] = useState(0);
    const [workspaceCount] = useState(4);
    const [externalWindows, setExternalWindows] = useState<ExternalWindow[]>([]);
    const nextZIndex = useRef(10);

    // Poll for external windows every 2 seconds
    useEffect(() => {
        const poll = async () => {
            try {
                const extWins = await SystemBridge.getExternalWindows();
                setExternalWindows(extWins);
            } catch {
                // silently fail — no external window tracking in this session
            }
        };
        poll();
        const interval = setInterval(poll, 2000);
        return () => clearInterval(interval);
    }, []);

    // Core WM operations

    const openApp = useCallback((appId: string, isExternal = false, exec?: string) => {
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

    // Open an external window inside a Blue Environment frame
    const embedExternalWindow = useCallback((extWin: ExternalWindow) => {
        // Check if already embedded
        const existing = windows.find(w => w.externalWindowId === extWin.id);
        if (existing) {
            focusWindow(existing.id);
            return;
        }

        const zIndex = nextZIndex.current++;
        const newWindow: WindowState = {
            id: `external-${extWin.id}-${Date.now()}`,
            appId: AppId.EXTERNAL,
            title: extWin.title || extWin.class || 'External App',
            x: 200 + ((windows.length % 5) * 30),
            y: 150 + ((windows.length % 5) * 30),
            width: 900,
            height: 650,
            isMinimized: extWin.isMinimized,
            isMaximized: false,
            zIndex,
            isExternal: true,
            workspace: currentWorkspace,
            externalWindowId: extWin.id,
            pid: extWin.pid,
        };

        setWindows(prev => [...prev, newWindow]);
        setActiveWindowId(newWindow.id);
        // Attempt to reparent the window into our frame
        SystemBridge.embedExternalWindow(extWin.id, newWindow.id);
    }, [windows, currentWorkspace]);

    const closeWindow = useCallback((id: string) => {
        const win = windows.find(w => w.id === id);
        if (win?.isExternal && win.externalWindowId) {
            SystemBridge.closeExternalWindow(win.externalWindowId);
        }
        setWindows(prev => prev.filter(w => w.id !== id));
        setActiveWindowId(prev => prev === id ? null : prev);
    }, [windows]);

    const focusWindow = useCallback((id: string) => {
        const zIndex = nextZIndex.current++;
        setActiveWindowId(id);
        setWindows(prev => prev.map(w => w.id === id ? { ...w, zIndex, isMinimized: false } : w));

        // Also focus external window if it's embedded
        const win = windows.find(w => w.id === id);
        if (win?.isExternal && win.externalWindowId) {
            SystemBridge.focusExternalWindow(win.externalWindowId);
        }
    }, [windows]);

    const minimizeWindow = useCallback((id: string) => {
        const win = windows.find(w => w.id === id);
        if (win?.isExternal && win.externalWindowId) {
            SystemBridge.minimizeExternalWindow(win.externalWindowId);
        }
        setWindows(prev => prev.map(w => w.id === id ? { ...w, isMinimized: true } : w));
        setActiveWindowId(prev => prev === id ? null : prev);
    }, [windows]);

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
        const win = windows.find(w => w.id === id);
        if (!win) return;

        if (win.isMinimized) {
            const zIndex = nextZIndex.current++;
            setWindows(prev => prev.map(w => w.id === id ? { ...w, isMinimized: false, zIndex } : w));
            setActiveWindowId(id);
            if (win.isExternal && win.externalWindowId) {
                SystemBridge.focusExternalWindow(win.externalWindowId);
            }
        } else if (activeWindowId === id) {
            minimizeWindow(id);
        } else {
            focusWindow(id);
        }
    }, [windows, activeWindowId, focusWindow, minimizeWindow]);

    const switchWorkspace = useCallback((index: number, total = workspaceCount) => {
        const next = ((index % total) + total) % total;
        setCurrentWorkspace(next);
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

    const visibleWindows = windows.filter(
        w => w.workspace === currentWorkspace || w.workspace === undefined
    );

    return {
        windows,
        visibleWindows,
        activeWindowId,
        currentWorkspace,
        workspaceCount,
        externalWindows,
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
        embedExternalWindow,
    };
}
