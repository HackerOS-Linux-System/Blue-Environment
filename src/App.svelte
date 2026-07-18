import React, { useState, useEffect, useCallback, useRef } from 'react';
import { AppId, UserConfig } from './types';
import { APPS } from './constants';
import { configStore } from './utils/configStore';
import { useWindowManager } from './hooks/useWindowManager';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';
import { SystemBridge } from './utils/systemBridge';
import { LanguageProvider, useLanguage } from './contexts/LanguageContext';
import { DialogProvider, useDialog } from './contexts/DialogContext';
import Window from './components/Window';
import TopBar from './components/TopBar';
import StartMenu from './components/StartMenu';
import ControlCenter from './components/ControlCenter';
import NotificationCenter from './components/NotificationCenter';
import WindowSwitcher from './components/WindowSwitcher';
import WorkspaceSwitcher from './components/WorkspaceSwitcher';
import ClipboardPanel from './components/ClipboardPanel';
import ToastContainer from './components/ToastContainer';
import { FileText, Folder, Image, File, Music, Video, Archive, Power, RefreshCcw, Moon, HardDrive, LogOut } from 'lucide-react';

interface DesktopItem {
    id: string;
    name: string;
    type: 'file' | 'folder' | 'app';
    path: string;
    x: number;
    y: number;
    icon?: React.ComponentType<any>;
    appId?: string;
}

interface ContextMenuState {
    visible: boolean;
    x: number;
    y: number;
    targetId?: string;
}

function getExternalAppExec(appId: string): string {
    const app = APPS[appId as AppId];
    if (!app?.externalPath) return appId;
    const home = (window as any).__TAURI_HOME__ || '~';
    return `${home}/.legendaryos/Blue-Environment/apps/${app.externalPath}/${app.externalPath}`;
}

function getIconForFile(name: string, isDir: boolean, mimeType?: string): React.ComponentType<any> {
    if (isDir) return Folder;
    const ext = name.split('.').pop()?.toLowerCase();
    if (mimeType?.startsWith('image/') || ['png', 'jpg', 'jpeg', 'gif', 'svg'].includes(ext || '')) return Image;
    if (mimeType?.startsWith('audio/') || ['mp3', 'wav', 'ogg', 'flac'].includes(ext || '')) return Music;
    if (mimeType?.startsWith('video/') || ['mp4', 'mkv', 'avi', 'mov'].includes(ext || '')) return Video;
    if (['zip', 'tar', 'gz', '7z', 'rar'].includes(ext || '')) return Archive;
    return FileText;
}

// ── Power menu overlay ─────────────────────────────────────────────────────
const PowerOverlay: React.FC<{ onClose: () => void }> = ({ onClose }) => {
    const [countdown, setCountdown] = useState<number | null>(null);
    const [action, setAction] = useState<string | null>(null);

    const triggerAction = (act: string) => {
        setAction(act);
        setCountdown(5);
    };

    useEffect(() => {
        if (countdown === null) return;
        if (countdown === 0) {
            SystemBridge.powerAction(action!);
            onClose();
            return;
        }
        const t = setTimeout(() => setCountdown(c => (c ?? 1) - 1), 1000);
        return () => clearTimeout(t);
    }, [countdown, action, onClose]);

    return (
        <div className="fixed inset-0 z-[500] bg-black/70 backdrop-blur-md flex items-center justify-center" onClick={onClose}>
        <div className="bg-slate-900/95 border border-white/10 rounded-3xl p-8 shadow-2xl w-96 text-center" onClick={e => e.stopPropagation()}>
        {countdown !== null ? (
            <>
            <div className="text-6xl font-bold text-white mb-2">{countdown}</div>
            <p className="text-slate-400 mb-6">
            {action === 'shutdown' && 'Shutting down'}
            {action === 'reboot' && 'Restarting'}
            {action === 'suspend' && 'Suspending'}
            {action === 'hibernate' && 'Hibernating'}
            {action === 'logout' && 'Logging out'}
            ...
            </p>
            <button onClick={onClose} className="px-6 py-2 bg-slate-700 hover:bg-slate-600 rounded-xl text-white transition-colors">
            Cancel
            </button>
            </>
        ) : (
            <>
            <div className="w-16 h-16 bg-red-500/10 rounded-full flex items-center justify-center mx-auto mb-6">
            <Power size={32} className="text-red-400" />
            </div>
            <h2 className="text-xl font-bold text-white mb-2">Power Options</h2>
            <p className="text-slate-400 text-sm mb-6">Choose what you want to do</p>
            <div className="grid grid-cols-3 gap-3 mb-4">
            {[
                { action: 'shutdown', icon: Power, label: 'Shut Down', color: 'hover:bg-red-500/20 hover:text-red-400' },
             { action: 'reboot', icon: RefreshCcw, label: 'Restart', color: 'hover:bg-yellow-500/20 hover:text-yellow-400' },
             { action: 'suspend', icon: Moon, label: 'Suspend', color: 'hover:bg-blue-500/20 hover:text-blue-400' },
             { action: 'hibernate', icon: HardDrive, label: 'Hibernate', color: 'hover:bg-purple-500/20 hover:text-purple-400' },
             { action: 'logout', icon: LogOut, label: 'Log Out', color: 'hover:bg-orange-500/20 hover:text-orange-400' },
            ].map(item => (
                <button
                key={item.action}
                onClick={() => triggerAction(item.action)}
                className={`flex flex-col items-center gap-2 p-4 rounded-2xl bg-slate-800 border border-white/5 transition-all ${item.color} group`}
                >
                <item.icon size={24} className="text-slate-400 group-hover:scale-110 transition-transform" />
                <span className="text-xs text-slate-400 group-hover:text-white">{item.label}</span>
                </button>
            ))}
            </div>
            <button onClick={onClose} className="text-sm text-slate-500 hover:text-white transition-colors">
            Cancel
            </button>
            </>
        )}
        </div>
        </div>
    );
};

// ── Lock screen ────────────────────────────────────────────────────────────
const LockScreen: React.FC<{ onUnlock: () => void }> = ({ onUnlock }) => {
    const [password, setPassword] = useState('');
    const [time, setTime] = useState(new Date());
    const [shake, setShake] = useState(false);

    useEffect(() => {
        const t = setInterval(() => setTime(new Date()), 1000);
        return () => clearInterval(t);
    }, []);

    const handleUnlock = () => {
        // In a real environment, verify against system auth
        // For now, any input or Enter unlocks (demo mode)
        onUnlock();
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') handleUnlock();
    };

        return (
            <div className="fixed inset-0 z-[600] bg-slate-900 flex flex-col items-center justify-center"
            style={{ background: 'linear-gradient(135deg, #0f172a 0%, #1e1b4b 50%, #0f172a 100%)' }}>
            <div className="text-center">
            <div className="text-7xl font-thin text-white mb-2">
            {time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
            </div>
            <div className="text-xl text-slate-400 mb-12">
            {time.toLocaleDateString([], { weekday: 'long', month: 'long', day: 'numeric' })}
            </div>
            <div className="w-20 h-20 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center mx-auto mb-4 shadow-2xl shadow-blue-500/30">
            <span className="text-3xl font-bold text-white">
            {SystemBridge.getUsername().charAt(0).toUpperCase()}
            </span>
            </div>
            <div className="text-white font-medium mb-6">{SystemBridge.getUsername()}</div>
            <div className={`flex gap-3 items-center ${shake ? 'animate-pulse' : ''}`}>
            <input
            type="password"
            value={password}
            onChange={e => setPassword(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Enter password or press Enter"
            autoFocus
            className="bg-white/10 border border-white/20 rounded-full px-5 py-3 text-white placeholder-slate-400 focus:outline-none focus:border-blue-400 text-center w-64 backdrop-blur"
            />
            <button onClick={handleUnlock} className="w-12 h-12 rounded-full bg-blue-600 hover:bg-blue-500 flex items-center justify-center transition-colors shadow-lg">
            →
            </button>
            </div>
            <p className="text-slate-600 text-xs mt-4">Press Enter or click → to unlock</p>
            </div>
            </div>
        );
};

// ── Main app content ───────────────────────────────────────────────────────
const AppContent: React.FC = () => {
    const { t } = useLanguage();
    const dialog = useDialog();
    const [userConfig, setUserConfig] = useState<UserConfig | null>(null);
    const [sessionType, setSessionType] = useState<string>('unknown');
    const [wallpaperData, setWallpaperData] = useState<string>('');
    const [desktopItems, setDesktopItems] = useState<DesktopItem[]>([]);
    const [selectedItems, setSelectedItems] = useState<string[]>([]);
    const [selectionBox, setSelectionBox] = useState<{
        start: { x: number; y: number };
        end: { x: number; y: number };
    } | null>(null);
    const [contextMenu, setContextMenu] = useState<ContextMenuState>({ visible: false, x: 0, y: 0 });
    const [showPowerOverlay, setShowPowerOverlay] = useState(false);
    const [isLocked, setIsLocked] = useState(false);
    const desktopRef = useRef<HTMLDivElement>(null);

    // Subscribe to config changes (including wallpaper)
    useEffect(() => {
        // Populate window.__TAURI_ENV_USERNAME__ / __TAURI_ENV_HOSTNAME__ /
        // __TAURI_HOME__ once at startup so every component reading them
        // synchronously (TopBar, Mail, About, ...) gets real values
        // instead of the "user" / "localhost" / "~" placeholders.
        SystemBridge.initEnvironment();

        configStore.init().then(cfg => {
            setUserConfig(cfg);
        });
        SystemBridge.getSessionType().then(setSessionType);

        // Subscribe to config changes so wallpaper updates live
        const unsub = configStore.subscribe(cfg => {
            setUserConfig({ ...cfg });
        });

        return unsub;
    }, []);

    // Load wallpaper whenever config.wallpaper changes
    useEffect(() => {
        if (!userConfig?.wallpaper) return;
        const loadWallpaper = async () => {
            try {
                if (userConfig.wallpaper.startsWith('file://')) {
                    let data = await SystemBridge.getWallpaperPreview(userConfig.wallpaper);
                    if (!data) {
                        // Configured wallpaper path doesn't exist on this system —
                        // fall back to the first wallpaper Blue actually found on disk.
                        const available = await SystemBridge.getWallpapers();
                        for (const candidate of available) {
                            if (candidate === userConfig.wallpaper) continue;
                            data = await SystemBridge.getWallpaperPreview(candidate);
                            if (data) break;
                        }
                    }
                    setWallpaperData(data || '');
                } else if (userConfig.wallpaper.startsWith('data:') || userConfig.wallpaper.startsWith('http')) {
                    setWallpaperData(userConfig.wallpaper);
                } else {
                    setWallpaperData('');
                }
            } catch {
                setWallpaperData('');
            }
        };
        loadWallpaper();
    }, [userConfig?.wallpaper]);

    const wm = useWindowManager();

    const [isStartMenuOpen, setIsStartMenuOpen] = useState(false);
    const [isFullScreenStartOpen, setIsFullScreenStartOpen] = useState(false);
    const [isControlCenterOpen, setIsControlCenterOpen] = useState(false);
    const [isNotificationCenterOpen, setIsNotificationCenterOpen] = useState(false);
    const [isClipboardOpen, setIsClipboardOpen] = useState(false);

    const closeAllPanels = useCallback(() => {
        setIsStartMenuOpen(false);
        setIsFullScreenStartOpen(false);
        setIsControlCenterOpen(false);
        setIsNotificationCenterOpen(false);
        setIsClipboardOpen(false);
    }, []);

    useEffect(() => {
        if (userConfig?.desktopPath) {
            loadDesktopItems(userConfig.desktopPath);
        }
    }, [userConfig?.desktopPath]);

    const loadDesktopItems = async (path: string) => {
        const files = await SystemBridge.getFiles(path);
        const items: DesktopItem[] = files.map((file, index) => ({
            id: file.path,
            name: file.name,
            type: file.is_dir ? 'folder' : 'file',
            path: file.path,
            x: 20 + (index % 5) * 100,
                                                                 y: 60 + Math.floor(index / 5) * 100,
                                                                 icon: getIconForFile(file.name, file.is_dir, file.mime_type),
        }));
        setDesktopItems(items);
    };

    // Global event listeners
    useEffect(() => {
        const onOpenTerminal = () => wm.openApp(AppId.TERMINAL);
        const onClosePanels = () => closeAllPanels();
        const onToggleClipboard = () => setIsClipboardOpen(prev => !prev);
        const onScreenshot = () => SystemBridge.takeScreenshot();
        const onLockScreen = () => setIsLocked(true);
        const onShowDesktop = () => {
            // Minimize all windows on current workspace
            wm.windows
            .filter(w => w.workspace === wm.currentWorkspace && !w.isMinimized)
            .forEach(w => wm.minimizeWindow(w.id));
        };
        const onPrintScreen = (e: KeyboardEvent) => {
            if (e.key === 'PrintScreen') { e.preventDefault(); SystemBridge.takeScreenshot(); }
        };

        window.addEventListener('keydown', onPrintScreen, { capture: true });
        window.addEventListener('blue:open-terminal', onOpenTerminal);
        window.addEventListener('blue:close-panels', onClosePanels);
        window.addEventListener('blue:toggle-clipboard', onToggleClipboard);
        window.addEventListener('blue:screenshot', onScreenshot);
        window.addEventListener('blue:lock-screen', onLockScreen);
        window.addEventListener('blue:show-desktop', onShowDesktop);
        window.addEventListener('blue:power-menu', () => setShowPowerOverlay(true));

        return () => {
            window.removeEventListener('keydown', onPrintScreen, { capture: true });
            window.removeEventListener('blue:open-terminal', onOpenTerminal);
            window.removeEventListener('blue:close-panels', onClosePanels);
            window.removeEventListener('blue:toggle-clipboard', onToggleClipboard);
            window.removeEventListener('blue:screenshot', onScreenshot);
            window.removeEventListener('blue:lock-screen', onLockScreen);
            window.removeEventListener('blue:show-desktop', onShowDesktop);
        };
    }, [wm.openApp, closeAllPanels, wm.windows, wm.currentWorkspace, wm.minimizeWindow]);

    // Alt+Tab switcher state
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

    // Check if an app is enabled in config
    const isAppEnabled = useCallback((appId: string): boolean => {
        // Settings and About are always enabled
        if (appId === AppId.SETTINGS || appId === AppId.ABOUT) return true;
        if (!userConfig?.appsEnabled) return true;
        const map: Record<string, string> = {
            [AppId.AI_ASSISTANT]:   'blueAI',
            [AppId.BLUE_CODE]:      'blueCode',
            [AppId.BLUE_SOFTWARE]:  'blueSoftware',
            [AppId.MAIL]:           'mail',
            [AppId.CALCULATOR]:     'calculator',
            [AppId.NOTEPAD]:        'notepad',
            [AppId.SYSTEM_MONITOR]: 'systemMonitor',
            [AppId.EXPLORER]:       'explorer',
            [AppId.TERMINAL]:       'terminal',
            [AppId.BLUE_WEB]:       'blueWeb',
            [AppId.CAMERA]:         'camera',
        };
        const key = map[appId];
        if (key) {
            const enabled = (userConfig.appsEnabled as Record<string, boolean>)[key];
            return enabled === undefined ? true : enabled;
        }
        return true;
    }, [userConfig?.appsEnabled]);

    const handleOpenApp = useCallback(
        (appId: string, isExternal = false, exec?: string) => {
            closeAllPanels();

            // Check if app is disabled
            if (!isAppEnabled(appId)) {
                window.dispatchEvent(new CustomEvent('blue:show-toast', {
                    detail: {
                        id: Date.now().toString(),
                                                     title: 'App Disabled',
                                                     message: `This app is disabled in Settings → Applications`,
                    }
                }));
                return;
            }

            const appDef = APPS[appId as AppId];
            if (appDef?.isExternal || isExternal) {
                const resolvedExec = exec || (appDef?.externalPath ? getExternalAppExec(appId) : appId);
                SystemBridge.launchApp(resolvedExec, appId);
                SystemBridge.recordAppLaunch(appId);
                return;
            }
            wm.openApp(appId, false);
            SystemBridge.recordAppLaunch(appId);
        },
        [wm.openApp, closeAllPanels, isAppEnabled]
    );

    const handleOpenSettings = useCallback(
        (tab?: string) => {
            closeAllPanels();
            wm.openApp(AppId.SETTINGS, false);
            if (tab) {
                setTimeout(
                    () => window.dispatchEvent(new CustomEvent('blue:settings-tab', { detail: tab })),
                           150
                );
            }
        },
        [wm.openApp, closeAllPanels]
    );

    // Desktop mouse interactions
    const handleDesktopMouseDown = useCallback(
        (e: React.MouseEvent) => {
            if (e.target === desktopRef.current) {
                setSelectionBox({ start: { x: e.clientX, y: e.clientY }, end: { x: e.clientX, y: e.clientY } });
                setSelectedItems([]);
                setContextMenu(c => ({ ...c, visible: false }));
                closeAllPanels();
            }
        },
        [closeAllPanels]
    );

    const handleDesktopMouseMove = useCallback(
        (e: React.MouseEvent) => {
            if (selectionBox) {
                setSelectionBox(prev => prev ? { ...prev, end: { x: e.clientX, y: e.clientY } } : null);
            }
        },
        [selectionBox]
    );

    const handleDesktopMouseUp = useCallback(() => {
        if (!selectionBox) return;
        const l = Math.min(selectionBox.start.x, selectionBox.end.x);
        const r = Math.max(selectionBox.start.x, selectionBox.end.x);
        const t = Math.min(selectionBox.start.y, selectionBox.end.y);
        const b = Math.max(selectionBox.start.y, selectionBox.end.y);
        if (r - l > 4 || b - t > 4) {
            setSelectedItems(
                desktopItems
                .filter(i => {
                    const cx = i.x + 32, cy = i.y + 32;
                    return cx >= l && cx <= r && cy >= t && cy <= b;
                })
                .map(i => i.id)
            );
        }
        setSelectionBox(null);
    }, [selectionBox, desktopItems]);

    const handleDesktopItemDoubleClick = (item: DesktopItem) => {
        if (item.type === 'folder') {
            wm.openApp(AppId.EXPLORER, false, item.path);
        } else {
            SystemBridge.launchApp(`xdg-open "${item.path}"`);
        }
    };

    const handleDeleteDesktopItem = useCallback(async (item: DesktopItem) => {
        const ok = await dialog.confirm({
            title: 'Delete item',
            message: `Delete "${item.name}"? This cannot be undone.`,
            confirmLabel: 'Delete',
            danger: true,
        });
        if (!ok || !item.path) return;
        await SystemBridge.deleteFile(item.path).catch(() => {});
        if (userConfig?.desktopPath) loadDesktopItems(userConfig.desktopPath);
    }, [dialog, userConfig, loadDesktopItems]);

    const createNewFolder = async () => {
        setContextMenu(c => ({ ...c, visible: false }));
        const name = await dialog.prompt({
            title: t('startmenu.new_folder') || 'New Folder',
            placeholder: 'Untitled Folder',
            defaultValue: 'New Folder',
            confirmLabel: 'Create',
        });
        if (!name || !userConfig?.desktopPath) return;
        await SystemBridge.createFolder(userConfig.desktopPath, name);
        loadDesktopItems(userConfig.desktopPath);
    };

    const createNewTextFile = async () => {
        setContextMenu(c => ({ ...c, visible: false }));
        const name = await dialog.prompt({
            title: t('startmenu.new_text_file') || 'New Text File',
            placeholder: 'note.txt',
            defaultValue: 'New Document.txt',
            confirmLabel: 'Create',
        });
        if (!name || !userConfig?.desktopPath) return;
        await SystemBridge.createTextFile(userConfig.desktopPath, name, '');
        loadDesktopItems(userConfig.desktopPath);
    };

    const workspaceWindowCounts = Array.from({ length: wm.workspaceCount }, (_, i) =>
    wm.windows.filter(w => w.workspace === i && !w.isMinimized).length
    );

    if (!userConfig) return (
        <div className="w-screen h-screen bg-slate-900 flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
        <div className="w-16 h-16 bg-blue-600 rounded-full flex items-center justify-center shadow-2xl shadow-blue-500/30">
        <span className="text-2xl font-bold text-white">B</span>
        </div>
        <div className="text-white text-sm animate-pulse">Loading Blue Environment...</div>
        </div>
        </div>
    );

    return (
        <div
        ref={desktopRef}
        className="relative w-screen h-screen overflow-hidden select-none theme-text-primary font-sans"
        style={{
            backgroundImage: wallpaperData ? `url(${wallpaperData})` : 'none',
            backgroundSize: 'cover',
            backgroundPosition: 'center',
            backgroundRepeat: 'no-repeat',
            backgroundColor: wallpaperData ? undefined : '#0f172a',
        }}
        onMouseDown={handleDesktopMouseDown}
        onMouseMove={handleDesktopMouseMove}
        onMouseUp={handleDesktopMouseUp}
        onContextMenu={e => {
            e.preventDefault();
            setContextMenu({ visible: true, x: e.clientX, y: e.clientY });
        }}
        >
        {/* Wallpaper overlay for depth */}
        <div className="absolute inset-0 pointer-events-none bg-black/10" />

        <WindowSwitcher windows={wm.windows} selectedIndex={switcherSelectedIndex} isVisible={isSwitcherVisible} />
        <WorkspaceSwitcher currentWorkspace={wm.currentWorkspace} workspaceCount={wm.workspaceCount} windowCounts={workspaceWindowCounts} />

        {/* Desktop icons */}
        {desktopItems.map(item => (
            <div
            key={item.id}
            className={`absolute flex flex-col items-center gap-1 p-2 w-24 rounded-xl border border-transparent transition-all hover:bg-white/10 hover:border-white/10 cursor-pointer ${
                selectedItems.includes(item.id) ? 'bg-blue-600/30 border-blue-500/50' : ''
            }`}
            style={{ left: item.x, top: item.y }}
            onMouseDown={e => {
                e.stopPropagation();
                if (e.ctrlKey) {
                    setSelectedItems(prev =>
                    prev.includes(item.id) ? prev.filter(i => i !== item.id) : [...prev, item.id]
                    );
                } else {
                    setSelectedItems([item.id]);
                }
            }}
            onDoubleClick={() => handleDesktopItemDoubleClick(item)}
            onContextMenu={e => {
                e.stopPropagation();
                setContextMenu({ visible: true, x: e.clientX, y: e.clientY, targetId: item.id });
            }}
            >
            <div className="w-12 h-12 flex items-center justify-center text-white drop-shadow-lg">
            {item.icon ? <item.icon size={40} /> : <FileText size={40} />}
            </div>
            <span className="text-xs text-center text-white font-medium drop-shadow-md line-clamp-2 leading-tight px-1 bg-black/25 rounded-md">
            {item.name}
            </span>
            </div>
        ))}

        {/* Drag selection box */}
        {selectionBox && (
            <div
            className="absolute bg-blue-500/15 border border-blue-400/50 z-10 pointer-events-none rounded"
            style={{
                left: Math.min(selectionBox.start.x, selectionBox.end.x),
                          top: Math.min(selectionBox.start.y, selectionBox.end.y),
                          width: Math.abs(selectionBox.end.x - selectionBox.start.x),
                          height: Math.abs(selectionBox.end.y - selectionBox.start.y),
            }}
            />
        )}

        {/* Desktop context menu */}
        {contextMenu.visible && (
            <div
            className="absolute w-52 bg-slate-900/98 backdrop-blur-xl border border-white/10 rounded-xl shadow-2xl z-50 flex flex-col py-1.5"
            style={{ left: contextMenu.x, top: contextMenu.y }}
            onMouseDown={e => e.stopPropagation()}
            >
            {/* File-specific actions when right-clicked on a desktop item */}
            {contextMenu.targetId && (() => {
                const item = desktopItems.find(i => i.id === contextMenu.targetId);
                if (!item) return null;
                return (<>
                <button onClick={() => { handleDesktopItemDoubleClick(item); setContextMenu(c => ({ ...c, visible: false })); }}
                className="px-3 py-2 hover:bg-white/10 text-sm text-left text-white font-medium transition-colors rounded-lg mx-1">
                {t('filemenu.open')}
                </button>
                <div className="px-3 py-1 text-xs text-slate-500 font-medium">{t('filemenu.open_with')}</div>
                {[
                    { id: AppId.NOTEPAD,    label: 'Notepad',       ext: ['txt','md','json','log','toml','yaml','yml','sh','conf','csv'] },
                    { id: AppId.BLUE_CODE,  label: 'Blue Code',     ext: ['ts','tsx','js','jsx','py','rb','rs','go','c','cpp','h','css','html','xml'] },
                    { id: AppId.BLUE_IMAGES,label: 'Blue Images',   ext: ['png','jpg','jpeg','gif','webp','bmp','svg','avif'] },
                    { id: AppId.BLUE_VIDEOS,label: 'Blue Video',    ext: ['mp4','webm','mkv','avi','mov','flv','wmv','ogv'] },
                    { id: AppId.BLUE_ARCHIVE,label:'Blue Archive',  ext: ['zip','tar','gz','bz2','xz','7z','rar','zst'] },
                ].filter(a => {
                    const ext = (item.name.split('.').pop() || '').toLowerCase();
                    return a.ext.includes(ext);
                }).map(a => (
                    <button key={a.id} onClick={() => {
                        wm.openApp(a.id);
                        setContextMenu(c => ({ ...c, visible: false }));
                    }} className="px-5 py-1.5 hover:bg-white/10 text-xs text-left text-slate-300 hover:text-white transition-colors rounded-lg mx-1">
                    {a.label}
                    </button>
                ))}
                <div className="h-px bg-white/10 my-1 mx-2" />
                <button onClick={() => { handleDeleteDesktopItem(item); setContextMenu(c => ({ ...c, visible: false })); }}
                className="px-3 py-2 hover:bg-red-500/10 hover:text-red-400 text-sm text-left text-slate-300 transition-colors rounded-lg mx-1">
                🗑 {t('filemenu.delete')}
                </button>
                </>);
            })()}
            {/* Empty-desktop actions */}
            {!contextMenu.targetId && (<>
            <button onClick={() => { handleOpenSettings('personalization'); setContextMenu(c => ({ ...c, visible: false })); }}
            className="px-3 py-2 hover:bg-white/10 text-sm text-left text-slate-200 hover:text-white transition-colors rounded-lg mx-1">
            🎨 {t('settings.personalization')}
            </button>
            <button onClick={() => { handleOpenApp(AppId.TERMINAL); setContextMenu(c => ({ ...c, visible: false })); }}
            className="px-3 py-2 hover:bg-white/10 text-sm text-left text-slate-200 hover:text-white transition-colors rounded-lg mx-1">
            💻 {t('terminal.title')}
            </button>
            <div className="h-px bg-white/10 my-1 mx-2" />
            <button onClick={createNewFolder} className="px-3 py-2 hover:bg-white/10 text-sm text-left text-slate-200 hover:text-white transition-colors rounded-lg mx-1">
            📁 {t('startmenu.new_folder')}
            </button>
            <button onClick={createNewTextFile} className="px-3 py-2 hover:bg-white/10 text-sm text-left text-slate-200 hover:text-white transition-colors rounded-lg mx-1">
            📄 {t('startmenu.new_text_file')}
            </button>
            <div className="h-px bg-white/10 my-1 mx-2" />
            <button onClick={() => { if (userConfig?.desktopPath) loadDesktopItems(userConfig.desktopPath); setContextMenu(c => ({ ...c, visible: false })); }}
            className="px-3 py-2 hover:bg-white/10 text-sm text-left text-slate-200 hover:text-white transition-colors rounded-lg mx-1">
            🔄 Refresh Desktop
            </button>
            <button onClick={() => { setShowPowerOverlay(true); setContextMenu(c => ({ ...c, visible: false })); }}
            className="px-3 py-2 hover:bg-red-500/10 hover:text-red-400 text-sm text-left text-slate-200 transition-colors rounded-lg mx-1">
            ⏻ Power Options
            </button>
            <div className="px-3 py-1 mt-1 text-[10px] text-slate-600">{t('startmenu.session')}: {sessionType}</div>
            </>)}
            </div>
        )}

        {/* Application windows */}
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

        {/* Panels / overlays */}
        <div onClick={e => e.stopPropagation()}>
        <StartMenu
        isOpen={isStartMenuOpen || isFullScreenStartOpen}
        isFullScreen={isFullScreenStartOpen}
        onOpenApp={handleOpenApp}
        onClose={closeAllPanels}
        onToggleFullScreen={() => { setIsStartMenuOpen(false); setIsFullScreenStartOpen(true); }}
        />
        <ControlCenter isOpen={isControlCenterOpen} onOpenSettings={handleOpenSettings} />
        <NotificationCenter isOpen={isNotificationCenterOpen} onClose={() => setIsNotificationCenterOpen(false)} />
        {isClipboardOpen && <ClipboardPanel onClose={() => setIsClipboardOpen(false)} />}
        </div>

        {/* Top bar */}
        <div onClick={e => e.stopPropagation()}>
        <TopBar
        openWindows={wm.windows.map(w => ({
            id: w.id,
            appId: w.appId as AppId,
            isMinimized: w.isMinimized,
            isActive: w.id === wm.activeWindowId,
            workspace: w.workspace ?? 0,
        }))}
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
        isClipboardOpen={isClipboardOpen}
        onToggleClipboard={() => setIsClipboardOpen(prev => !prev)}

        />
        </div>

        <ToastContainer />

        {/* Power overlay */}
        {showPowerOverlay && <PowerOverlay onClose={() => setShowPowerOverlay(false)} />}

        {/* Lock screen */}
        {isLocked && <LockScreen onUnlock={() => setIsLocked(false)} />}
        </div>
    );
};

export default function App() {
    return (
        <LanguageProvider>
        <DialogProvider>
        <AppContent />
        </DialogProvider>
        </LanguageProvider>
    );
}
