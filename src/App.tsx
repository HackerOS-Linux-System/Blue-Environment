import React, { useState, useEffect, useCallback, useRef } from 'react';
import { AppId, UserConfig } from './types';
import { APPS } from './constants';
import { configStore } from './utils/configStore';
import { useWindowManager } from './hooks/useWindowManager';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';
import { SystemBridge } from './utils/systemBridge';
import Window from './components/Window';
import TopBar from './components/TopBar';
import StartMenu from './components/StartMenu';
import ControlCenter from './components/ControlCenter';
import NotificationPanel from './components/NotificationPanel';
import WindowSwitcher from './components/WindowSwitcher';
import WorkspaceSwitcher from './components/WorkspaceSwitcher';
import { FileText, Folder, Trash2, Monitor, Smartphone, Globe } from 'lucide-react';

interface DesktopItem {
    id: string;
    name: string;
    type: 'file' | 'folder' | 'app';
    x: number;
    y: number;
    icon?: React.ComponentType<any>;
    appId?: string;
    path?: string;
}

interface ContextMenuState {
    visible: boolean;
    x: number;
    y: number;
    targetId?: string;
}

// Resolve the exec path for external Blue apps
function getExternalAppExec(appId: string): string {
    const app = APPS[appId as AppId];
    if (!app?.externalPath) return appId;
    // Binary lives at ~/.hackeros/Blue-Environment/apps/<name>/<name>
    const home = (window as any).__TAURI_HOME__ || '~';
    return `${home}/.hackeros/Blue-Environment/apps/${app.externalPath}/${app.externalPath}`;
}

export default function App() {
    const [userConfig, setUserConfig] = useState<UserConfig | null>(null);
    const [sessionType, setSessionType] = useState<string>('unknown');

    useEffect(() => {
        configStore.init().then(setUserConfig);
        SystemBridge.getSessionType().then(setSessionType);
        return configStore.subscribe(setUserConfig);
    }, []);

    const wm = useWindowManager();

    // Handle PrintScreen → screenshot (not intercepted by useKeyboardShortcuts)
    useEffect(() => {
        const handlePrintScreen = (e: KeyboardEvent) => {
            if (e.key === 'PrintScreen') {
                e.preventDefault();
                SystemBridge.takeScreenshot();
            }
        };
        window.addEventListener('keydown', handlePrintScreen, { capture: true });
        return () => window.removeEventListener('keydown', handlePrintScreen, { capture: true });
    }, []);

    // Custom events from keyboard shortcuts
    useEffect(() => {
        const openTerminal = () => wm.openApp(AppId.TERMINAL, true, '/usr/share/HackerOS/Scripts/HackerOS-Apps/Hacker-Term.AppImage');
        const closePanels = () => closeAllPanels();
        window.addEventListener('blue:open-terminal', openTerminal);
        window.addEventListener('blue:close-panels', closePanels);
        return () => {
            window.removeEventListener('blue:open-terminal', openTerminal);
            window.removeEventListener('blue:close-panels', closePanels);
        };
    }, []);

    // ── Panels ──────────────────────────────────────────────────────────
    const [isStartMenuOpen, setIsStartMenuOpen] = useState(false);
    const [isFullScreenStartOpen, setIsFullScreenStartOpen] = useState(false);
    const [isControlCenterOpen, setIsControlCenterOpen] = useState(false);
    const [isNotificationCenterOpen, setIsNotificationCenterOpen] = useState(false);

    const closeAllPanels = useCallback(() => {
        setIsStartMenuOpen(false);
        setIsFullScreenStartOpen(false);
        setIsControlCenterOpen(false);
        setIsNotificationCenterOpen(false);
    }, []);

    // ── Alt+Tab switcher ─────────────────────────────────────────────────
    const [isSwitcherVisible, setIsSwitcherVisible] = useState(false);
    const [switcherSelectedIndex, setSwitcherSelectedIndex] = useState(0);

    useKeyboardShortcuts({
        windows: wm.windows,
        activeWindowId: wm.activeWindowId,
        currentWorkspace: wm.currentWorkspace,
        workspaceCount: wm.workspaceCount,
        focusWindow: wm.focusWindow,
        minimizeWindow: wm.minimizeWindow,
        closeWindow: wm.closeWindow,
        maximizeWindow: wm.maximizeWindow,
        switchWorkspace: wm.switchWorkspace,
        onToggleStartMenu: () => { closeAllPanels(); setIsStartMenuOpen(prev => !prev); },
        onOpenFullScreenMenu: () => { closeAllPanels(); setIsFullScreenStartOpen(true); },
        onToggleControlCenter: () => { const was = isControlCenterOpen; closeAllPanels(); setIsControlCenterOpen(!was); },
        isSwitcherVisible,
        switcherSelectedIndex,
        setSwitcherVisible: setIsSwitcherVisible,
        setSwitcherIndex: setSwitcherSelectedIndex,
    });

    // ── Open app ─────────────────────────────────────────────────────────
    const handleOpenApp = useCallback((appId: string, isExternal = false, exec?: string) => {
        closeAllPanels();

        const appDef = APPS[appId as AppId];

        // External app defined in constants (Blue Edit, Blue Screen, etc.)
        if (appDef?.isExternal || isExternal) {
            const resolvedExec = exec || (appDef?.externalPath ? getExternalAppExec(appId) : appId);
            SystemBridge.launchApp(resolvedExec, appId);
            SystemBridge.recordAppLaunch(appId);
            return;
        }

        // Internal React app
        wm.openApp(appId, false);
        SystemBridge.recordAppLaunch(appId);
    }, [wm.openApp, closeAllPanels]);

    // ── Settings with tab ────────────────────────────────────────────────
    const handleOpenSettings = useCallback((tab?: string) => {
        closeAllPanels();
        wm.openApp(AppId.SETTINGS, false);
        if (tab) {
            // Emit event so SettingsApp can navigate to tab
            setTimeout(() => window.dispatchEvent(new CustomEvent('blue:settings-tab', { detail: tab })), 100);
        }
    }, [wm.openApp, closeAllPanels]);

    // ── Desktop items ─────────────────────────────────────────────────────
    const [desktopItems, setDesktopItems] = useState<DesktopItem[]>([
        { id: 'trash', name: 'Trash', type: 'folder', x: 20, y: 60, icon: Trash2 },
        { id: 'home', name: 'Home', type: 'folder', x: 20, y: 140, icon: Folder },
        { id: 'app-web', name: 'Blue Web', type: 'app', x: 20, y: 220, appId: AppId.BLUE_WEB, icon: Globe },
        { id: 'app-connect', name: 'Blue Connect', type: 'app', x: 20, y: 300, appId: AppId.BLUE_CONNECT, icon: Smartphone },
    ]);
    const [selectedItems, setSelectedItems] = useState<string[]>([]);
    const [selectionBox, setSelectionBox] = useState<{ start: { x: number; y: number }; end: { x: number; y: number } } | null>(null);
    const [contextMenu, setContextMenu] = useState<ContextMenuState>({ visible: false, x: 0, y: 0 });
    const desktopRef = useRef<HTMLDivElement>(null);

    const handleDesktopMouseDown = useCallback((e: React.MouseEvent) => {
        if (e.target === desktopRef.current) {
            setSelectionBox({ start: { x: e.clientX, y: e.clientY }, end: { x: e.clientX, y: e.clientY } });
            setSelectedItems([]);
            setContextMenu(c => ({ ...c, visible: false }));
            closeAllPanels();
        }
    }, [closeAllPanels]);

    const handleDesktopMouseMove = useCallback((e: React.MouseEvent) => {
        if (selectionBox) setSelectionBox(prev => prev ? { ...prev, end: { x: e.clientX, y: e.clientY } } : null);
    }, [selectionBox]);

    const handleDesktopMouseUp = useCallback(() => {
        if (!selectionBox) return;
        const l = Math.min(selectionBox.start.x, selectionBox.end.x);
        const r = Math.max(selectionBox.start.x, selectionBox.end.x);
        const t = Math.min(selectionBox.start.y, selectionBox.end.y);
        const b = Math.max(selectionBox.start.y, selectionBox.end.y);
        if (r - l > 4 || b - t > 4) {
            setSelectedItems(desktopItems.filter(i => {
                const cx = i.x + 32, cy = i.y + 32;
                return cx >= l && cx <= r && cy >= t && cy <= b;
            }).map(i => i.id));
        }
        setSelectionBox(null);
    }, [selectionBox, desktopItems]);

    const workspaceWindowCounts = Array.from({ length: wm.workspaceCount }, (_, i) =>
        wm.windows.filter(w => w.workspace === i && !w.isMinimized).length
    );

    if (!userConfig) return null;

    return (
        <div
            ref={desktopRef}
            className="relative w-screen h-screen overflow-hidden bg-cover bg-center select-none theme-text-primary font-sans"
            style={{ backgroundImage: `url(${userConfig.wallpaper})` }}
            onMouseDown={handleDesktopMouseDown}
            onMouseMove={handleDesktopMouseMove}
            onMouseUp={handleDesktopMouseUp}
            onContextMenu={e => { e.preventDefault(); setContextMenu({ visible: true, x: e.clientX, y: e.clientY }); }}
        >
            <div className="absolute inset-0 pointer-events-none bg-black/10" />

            <WindowSwitcher windows={wm.windows} selectedIndex={switcherSelectedIndex} isVisible={isSwitcherVisible} />
            <WorkspaceSwitcher currentWorkspace={wm.currentWorkspace} workspaceCount={wm.workspaceCount} windowCounts={workspaceWindowCounts} />

            {/* Desktop icons */}
            {desktopItems.map(item => (
                <div
                    key={item.id}
                    className={`absolute flex flex-col items-center gap-1 p-2 w-24 rounded-lg border border-transparent transition-all hover:bg-white/10 ${selectedItems.includes(item.id) ? 'bg-blue-600/30 border-blue-500/50' : ''}`}
                    style={{ left: item.x, top: item.y }}
                    onMouseDown={e => { e.stopPropagation(); if (e.ctrlKey) setSelectedItems(prev => prev.includes(item.id) ? prev.filter(i => i !== item.id) : [...prev, item.id]); else setSelectedItems([item.id]); }}
                    onDoubleClick={() => { if (item.type === 'app' && item.appId) handleOpenApp(item.appId); else if (item.type === 'folder') handleOpenApp(AppId.EXPLORER); }}
                    onContextMenu={e => { e.stopPropagation(); setContextMenu({ visible: true, x: e.clientX, y: e.clientY, targetId: item.id }); }}
                >
                    <div className="w-12 h-12 flex items-center justify-center text-white drop-shadow-md">
                        {item.icon ? <item.icon size={40} /> : <FileText size={40} />}
                    </div>
                    <span className="text-xs text-center text-white font-medium drop-shadow-md line-clamp-2 leading-tight px-1 bg-black/20 rounded">
                        {item.name}
                    </span>
                </div>
            ))}

            {/* Selection box */}
            {selectionBox && (
                <div className="absolute bg-blue-500/20 border border-blue-400/60 z-10 pointer-events-none"
                    style={{
                        left: Math.min(selectionBox.start.x, selectionBox.end.x),
                        top: Math.min(selectionBox.start.y, selectionBox.end.y),
                        width: Math.abs(selectionBox.end.x - selectionBox.start.x),
                        height: Math.abs(selectionBox.end.y - selectionBox.start.y),
                    }}
                />
            )}

            {/* Context menu */}
            {contextMenu.visible && (
                <div
                    className="absolute w-48 theme-bg-secondary/95 backdrop-blur-xl theme-border border rounded-xl shadow-2xl z-50 flex flex-col py-1"
                    style={{ left: contextMenu.x, top: contextMenu.y }}
                    onMouseDown={e => e.stopPropagation()}
                >
                    <button onClick={() => { handleOpenSettings('personalization'); setContextMenu(c => ({ ...c, visible: false })); }}
                        className="px-3 py-2 hover:theme-accent hover:text-white text-sm text-left">Personalization</button>
                    <button onClick={() => { handleOpenApp(AppId.TERMINAL, true); setContextMenu(c => ({ ...c, visible: false })); }}
                        className="px-3 py-2 hover:theme-accent hover:text-white text-sm text-left">Open Terminal</button>
                    <div className="h-px bg-white/10 my-1" />
                    <button onClick={() => { setDesktopItems(prev => [...prev, { id: Date.now().toString(), name: 'New Folder', type: 'folder', x: contextMenu.x - 20, y: contextMenu.y - 20, icon: Folder }]); setContextMenu(c => ({ ...c, visible: false })); }}
                        className="px-3 py-2 hover:theme-accent hover:text-white text-sm text-left">New Folder</button>
                    <div className="h-px bg-white/10 my-1" />
                    <div className="px-3 py-1 text-[10px] text-slate-500">Session: {sessionType}</div>
                </div>
            )}

            {/* Windows */}
            {wm.visibleWindows.map(win => {
                const AppComp = APPS[win.appId as AppId]?.component;
                if (!AppComp) return null;
                return (
                    <Window
                        key={win.id}
                        window={win}
                        isActive={wm.activeWindowId === win.id}
                        onClose={wm.closeWindow}
                        onMinimize={wm.minimizeWindow}
                        onMaximize={wm.maximizeWindow}
                        onFocus={wm.focusWindow}
                        onMove={wm.moveWindow}
                        onResize={wm.resizeWindow}
                    >
                        <AppComp windowId={win.id} />
                    </Window>
                );
            })}

            {/* Overlays */}
            <div onClick={e => e.stopPropagation()}>
                <StartMenu
                    isOpen={isStartMenuOpen || isFullScreenStartOpen}
                    isFullScreen={isFullScreenStartOpen}
                    onOpenApp={handleOpenApp}
                    onClose={closeAllPanels}
                    onToggleFullScreen={() => { setIsStartMenuOpen(false); setIsFullScreenStartOpen(true); }}
                />
                <ControlCenter
                    isOpen={isControlCenterOpen}
                    onOpenSettings={handleOpenSettings}
                />
                <NotificationPanel isOpen={isNotificationCenterOpen} onClose={() => setIsNotificationCenterOpen(false)} />
            </div>

            {/* TopBar */}
            <div onClick={e => e.stopPropagation()}>
                <TopBar
                    openWindows={wm.windows.map(w => ({ id: w.id, appId: w.appId as AppId, isMinimized: w.isMinimized, isActive: w.id === wm.activeWindowId, workspace: w.workspace ?? 0 }))}
                    currentWorkspace={wm.currentWorkspace}
                    workspaceCount={wm.workspaceCount}
                    onOpenApp={id => handleOpenApp(id)}
                    onToggleWindow={wm.toggleWindowFromTaskbar}
                    onStartClick={() => { closeAllPanels(); setIsStartMenuOpen(prev => !prev); }}
                    onStartDoubleClick={() => { closeAllPanels(); setIsFullScreenStartOpen(true); }}
                    onToggleControlCenter={() => { const was = isControlCenterOpen; closeAllPanels(); setIsControlCenterOpen(!was); }}
                    onToggleNotifications={() => { const was = isNotificationCenterOpen; closeAllPanels(); setIsNotificationCenterOpen(!was); }}
                    onSwitchWorkspace={wm.switchWorkspace}
                    isStartMenuOpen={isStartMenuOpen || isFullScreenStartOpen}
                />
            </div>
        </div>
    );
}
