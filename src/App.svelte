<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { configStore } from './lib/utils/configStore';
  import { initLanguage } from './lib/stores/language';
  import {
    windows, visibleWindows, activeWindowId, currentWorkspace, workspaceCount,
    openApp, closeWindow, focusWindow, minimizeWindow, maximizeWindow, togglePiP,
    moveWindow, resizeWindow, toggleWindowFromTaskbar, switchWorkspace, syncWorkspaceFromCompositor,
    startExternalWindowPolling, stopExternalWindowPolling,
    startParentalControlsUsageTracking, stopParentalControlsUsageTracking,
    externalWindows, getSwitcherItems,
  } from './lib/stores/windowManager';
  import { initKeyboardShortcuts } from './lib/stores/keyboardShortcuts';
  import { shellOverlayOpen, blockingOverlayOpen } from './lib/stores/overlayState';
  import { CLOSE_POPUPS_EVENT } from './lib/utils/popups';
  import { startCalendarReminders } from './lib/components/apps/Blue-Calendar-App/calendarReminders';
  import { closeContextMenu } from './lib/stores/contextMenu';
  import { hasCompletedWelcome } from './lib/components/apps/Blue-Welcome-App/welcome';
  import { createNotificationsStore } from './lib/components/apps/Blue-Notifications-App/notificationsStore';
  import OnscreenKeyboard from './lib/components/OnscreenKeyboard.svelte';
  import { APPS } from './lib/constants';
  import CommunityAppHost from './lib/components/apps/Community-App-Host/CommunityAppHost.svelte';
  import { Package as CommunityAppIcon } from 'lucide-svelte';
  import { AppId } from './lib/types';
  import TopBar from './lib/components/TopBar.svelte';
  import Desktop from './lib/components/Desktop.svelte';
  import StartMenu from './lib/components/StartMenu.svelte';
  import WindowComponent from './lib/components/Window.svelte';
  import WindowSwitcher from './lib/components/WindowSwitcher.svelte';
  import PowerMenu from './lib/components/PowerMenu.svelte';
  import ErrorBoundary from './lib/components/ErrorBoundary.svelte';
  import ControlCenter from './lib/components/ControlCenter.svelte';
  import NotificationCenter from './lib/components/NotificationCenter.svelte';
  import ClipboardPanel from './lib/components/ClipboardPanel.svelte';
  import ToastContainer from './lib/components/ToastContainer.svelte';
  import ContextMenu from './lib/components/ContextMenu.svelte';
  import { installGlobalContextMenu } from './lib/utils/globalContextMenu';
  import WorkspaceSwitcher from './lib/components/WorkspaceSwitcher.svelte';
  import HotCorners from './lib/components/HotCorners.svelte';
  import { normalizeHotCorners, DEFAULT_HOT_CORNERS, type HotCornersConfig, type HotCornerSlot } from './lib/utils/hotCorners';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import DialogHost from './lib/components/DialogHost.svelte';
  import ConflictDialog from './lib/components/ConflictDialog.svelte';
  import TransferProgress from './lib/components/TransferProgress.svelte';
  import { translate } from './lib/stores/language';
  import BlueFilePicker from './lib/components/BlueFilePicker.svelte';
  import ShellThemeStyle from './lib/components/ShellThemeStyle.svelte';
  import SystemThemeStyle from './lib/components/SystemThemeStyle.svelte';
  import BlueInstallerApp from './lib/components/apps/Blue-Installer/BlueInstallerApp.svelte';
  import { isLiveMode, liveModeChecked, liveView, checkLiveMode } from './lib/utils/liveMode';
  import { hideSplash, hideSplashAfter } from './lib/utils/splash';
  import LiveInstallerReturn from './lib/components/LiveInstallerReturn.svelte';
  import { resolveActiveShellTheme } from './lib/data/builtinThemes';
  import { SystemBridge, toAssetUrl } from './lib/utils/systemBridge';
  import { CompositorBridge, takeScreenshotUnified, isNativeBackend } from './lib/utils/compositorBridge';
  import { notificationManager } from './lib/utils/notificationManager';

  // Empty on purpose (was: a hardcoded `file:///usr/share/Blue-
  // Environment/wallpapers/default.png` path used unconditionally,
  // regardless of whether that file actually existed on the running
  // system — confirmed as a real, reproduced bug: a 404 for exactly
  // this asset:// path, persisting for the whole session whenever
  // `resolveDefaultWallpaper()` doesn't resolve to something real
  // before this initial value is ever overwritten below). An empty
  // string here means `background-image: url()` — CSS quietly no-ops
  // on an empty url(), so the `background: linear-gradient(...)`
  // fallback in this element's own style (see below) shows through
  // instead of a broken-image icon.
  // Which compositor backend the shell runs on (hackeros-comp | labwc | sway | wayfire) —
  // exposed as `data-backend` on the root element for CSS/debugging.
  let backendActive = '';
  let wallpaper = '';
  let theme = 'dark';
  let desktopPath = 'HOME/Desktop';
  let appsEnabled: Record<string, boolean> = {};

  // Panel (top bar) settings — previously `panelEnabled`/`panelPosition`
  // existed in the config type/defaults but nothing actually read them;
  // toggling them in Settings did nothing. Now wired through so windows
  // reserve space on the correct edge (or none, if the panel is
  // disabled — safe to do since the Super-key shortcut still opens the
  // start menu with no panel visible at all, see keyboardShortcuts.ts).
  let panelEnabled = true;
  let panelPosition: 'top' | 'bottom' = 'top';
  let panelSize = 48;
  $: barHeight = panelEnabled ? panelSize : 0;
  let shellThemeId: string | undefined;

  // On-screen keyboard — see OnscreenKeyboard.svelte's module doc for
  // what this can and can't type into. `enabled` is the Settings
  // toggle (opt-in, default off); `visible` is the moment-to-moment
  // show/hide state, auto-shown whenever a text-editable element inside
  // the shell gains focus while enabled, and auto-hidden when focus
  // moves to something non-editable (clicking the keyboard's own keys
  // never triggers this, since each key handler uses
  // `on:mousedown|preventDefault` specifically so it never steals DOM
  // focus away from the field being typed into).
  let onscreenKeyboardEnabled = false;
  let onscreenKeyboardVisible = false;
  $: if (!onscreenKeyboardEnabled) onscreenKeyboardVisible = false;
  function isTextEditableTarget(el: Element | null): boolean {
    if (!el) return false;
    if (el.tagName === 'TEXTAREA') return true;
    if (el.tagName === 'INPUT') {
      const type = (el as HTMLInputElement).type;
      return ['text', 'search', 'email', 'url', 'tel', 'password', 'number'].includes(type);
    }
    return (el as HTMLElement).isContentEditable === true;
  }
  function onGlobalFocusIn(e: FocusEvent) {
    if (!onscreenKeyboardEnabled) return;
    onscreenKeyboardVisible = isTextEditableTarget(e.target as Element | null);
  }
  let systemThemeId: string | null | undefined;

  // Active shell theme (Hydra etc. — see builtinThemes.ts) overrides the
  // person's own wallpaper/panel-position choices while selected, same
  // way `data-shell-theme`'s CSS overrides work in ShellThemeStyle.svelte
  // — `resolveActiveShellTheme` is the single shared source of truth
  // both use for "is there a real override active right now".
  $: activeShellTheme = resolveActiveShellTheme(shellThemeId);
  $: effectiveWallpaper = activeShellTheme?.wallpaper || wallpaper;

  // Wallpaper loading with a safety net. The primary path is the `asset:`
  // protocol (cheap, streamed by the webview). If that can't load the file
  // (scope mismatch, unusual webview build...) we fall back to reading the
  // file through a Rust command and feeding it as a data: URL, instead of
  // silently showing only the gradient like before.
  let wallpaperCss = '';
  let wallpaperToken = 0;
  async function resolveWallpaperCss(path: string) {
    const token = ++wallpaperToken;
    if (!path) { wallpaperCss = ''; return; }
    const assetUrl = toAssetUrl(path);
    const ok = await new Promise<boolean>((res) => {
      const img = new Image();
      img.onload = () => res(true);
      img.onerror = () => res(false);
      img.src = assetUrl;
    });
    if (token !== wallpaperToken) return;
    if (ok) { wallpaperCss = assetUrl; return; }
    const data = await SystemBridge.getWallpaperDataUrl(path);
    if (token !== wallpaperToken) return;
    wallpaperCss = data ?? '';
  }
  $: resolveWallpaperCss(effectiveWallpaper);
  // Theme layout's panelPosition can in principle be 'left'/'right' too
  // (richer future themes) — TopBar only understands 'top'/'bottom'
  // today, so only override with the theme's own choice when it's one
  // of those two; otherwise fall back to the person's setting rather
  // than silently doing nothing with a 'left'/'right' theme value.
  $: effectivePanelPosition =
    activeShellTheme && (activeShellTheme.layout.panelPosition === 'top' || activeShellTheme.layout.panelPosition === 'bottom')
      ? activeShellTheme.layout.panelPosition
      : panelPosition;

  function getAppDef(appId: string) {
    // Installed Blue Store apps/plugins have no static APPS entry (they're
    // discovered at run time, not compile time) — see windowManager.ts's
    // `openCommunityApp`/`openApp` doc for the `community:` convention.
    // Only `.component` is used from this synthetic object: the window's
    // title bar already reads `win.title` directly (set at launch time
    // from the package's own name), not `appDef.title`.
    if (appId.startsWith('community:')) {
      return { id: appId, title: 'App', icon: CommunityAppIcon, component: CommunityAppHost, isExternal: false } as (typeof APPS)[AppId];
    }
    return APPS[appId as AppId];
  }

  let isStartMenuOpen = false;
  let isStartMenuFullScreen = false;
  let hotCornersCfg: HotCornersConfig = DEFAULT_HOT_CORNERS;
  let superDoubleTapMs = 350;
  let deviceNotifications = true;
  let showTransferProgress = true;
  let lastSuperTap = 0;
  let cornerHandler: (slot: HotCornerSlot) => void = () => {};
  let isClipboardOpen = false;
  let isControlCenterOpen = false;
  let isNotificationsOpen = false;
  let showPowerMenu = false;

  let switcherVisible = false;
  let switcherIndex = 0;

  // Mirrors the overlay booleans above into `shellOverlayOpen` — see
  // `overlayState.ts`'s doc comment for why (BlueWebApp.svelte's
  // embedded-webview visibility gating needs to observe this from
  // outside App.svelte, and a plain component-local `let` can't be
  // imported elsewhere).
  $: shellOverlayOpen.set(
    isStartMenuOpen || isClipboardOpen || isControlCenterOpen ||
    isNotificationsOpen || showPowerMenu || switcherVisible
  );

  // ── Auto-dismiss of transient popups ────────────────────────────────────
  // Clicking an app, Alt+Tab, focusing a native window… must hide the Start
  // dropdown / Control Center (Wi-Fi) / notifications / clipboard / power menu.
  // The FULL-SCREEN app drawer is a mode, not a popup — it stays until closed.
  function closePopupsNow() {
    if (!isStartMenuFullScreen) isStartMenuOpen = false;
    isControlCenterOpen = false;
    isNotificationsOpen = false;
    isClipboardOpen = false;
    showPowerMenu = false;
    closeContextMenu();
  }
  // Remember when a popup last opened: a focus/blur wobble that arrives in the same
  // instant (the compositor raising the shell for Super) must not close it again.
  let anyPopupOpen = false;
  let popupOpenedAt = 0;
  $: {
    const open = isStartMenuOpen || isControlCenterOpen || isNotificationsOpen || isClipboardOpen || showPowerMenu;
    if (open && !anyPopupOpen) popupOpenedAt = Date.now();
    anyPopupOpen = open;
  }
  // Alt+Tab switcher appearing = the user is leaving whatever popup was open.
  $: if (switcherVisible) closePopupsNow();

  let cleanupKeyboard: () => void;

  // Kill the webview's native right-click menu (Back/Forward/Reload/Inspect)
  // everywhere and replace it with shell menus — see globalContextMenu.ts.
  // Calendar reminders fire from the shell itself, so they work with the Calendar window closed.
  onMount(() => startCalendarReminders());

  onMount(() => installGlobalContextMenu({
    minimize: (id) => minimizeWindow(id),
    maximize: (id) => maximizeWindow(id),
    close: (id) => closeWindow(id),
  }));

  onMount(() => {
    // Startup splash (index.html) stays up until BOTH the live-mode check and
    // the first config load are done, i.e. until the shell knows what it is
    // going to show — then fades out. The timeout is a safety net only.
    const liveCheck = checkLiveMode();
    hideSplashAfter(10000);
    initLanguage();
    startExternalWindowPolling();
    startParentalControlsUsageTracking();
    document.addEventListener('focusin', onGlobalFocusIn);

    // First-run wizard — see welcome.ts's doc comment. Deferred one tick
    // (not called synchronously before anything else) so it opens as an
    // ordinary window on top of an already-initializing desktop rather
    // than racing window-manager/config setup that hasn't run yet.
    // Not in a live session: there the person is here to install (or just
    // look around), not to be welcomed to an installed system.
    if (!hasCompletedWelcome()) {
      liveCheck.then((live) => {
        if (!live) setTimeout(() => openApp(AppId.BLUE_WELCOME), 300);
      });
    }

    // Blue Notifications' feed-watcher polling — starts once here,
    // unconditionally, independent of whether a Blue Notifications
    // window is ever opened this session. See notificationsStore.ts's
    // `startPolling` doc comment for exactly what "background" does and
    // doesn't mean (no OS daemon, just timers living as long as this
    // shell process does).
    createNotificationsStore().startPolling();

    const configReady = configStore.init();
    Promise.allSettled([liveCheck, configReady]).then(() => hideSplash());
    configReady.then((cfg) => {
      if (cfg.wallpaper) wallpaper = cfg.wallpaper;
      if (cfg.theme) theme = cfg.theme;
      if (cfg.appsEnabled) appsEnabled = cfg.appsEnabled;
      if (cfg.desktopPath) desktopPath = cfg.desktopPath;
      if (typeof cfg.panelEnabled === 'boolean') panelEnabled = cfg.panelEnabled;
      if (cfg.panelPosition === 'top' || cfg.panelPosition === 'bottom') panelPosition = cfg.panelPosition;
      if (typeof cfg.panelSize === 'number' && cfg.panelSize > 0) panelSize = cfg.panelSize;
      shellThemeId = cfg.shellThemeId;
      systemThemeId = cfg.systemThemeId;
      onscreenKeyboardEnabled = cfg.onscreenKeyboardEnabled ?? false;
      hotCornersCfg = normalizeHotCorners(cfg.hotCorners);
      deviceNotifications = cfg.deviceNotifications !== false;
      showTransferProgress = cfg.showTransferProgress !== false;
      superDoubleTapMs = typeof cfg.superDoubleTapMs === 'number' ? Math.min(800, Math.max(150, cfg.superDoubleTapMs)) : 350;
    });
    const unsubConfig = configStore.subscribe((cfg) => {
      if (cfg.wallpaper) wallpaper = cfg.wallpaper;
      if (cfg.theme) theme = cfg.theme;
      if (cfg.appsEnabled) appsEnabled = cfg.appsEnabled;
      if (cfg.desktopPath) desktopPath = cfg.desktopPath;
      if (typeof cfg.panelEnabled === 'boolean') panelEnabled = cfg.panelEnabled;
      if (cfg.panelPosition === 'top' || cfg.panelPosition === 'bottom') panelPosition = cfg.panelPosition;
      if (typeof cfg.panelSize === 'number' && cfg.panelSize > 0) panelSize = cfg.panelSize;
      shellThemeId = cfg.shellThemeId;
      systemThemeId = cfg.systemThemeId;
      onscreenKeyboardEnabled = cfg.onscreenKeyboardEnabled ?? false;
      hotCornersCfg = normalizeHotCorners(cfg.hotCorners);
      deviceNotifications = cfg.deviceNotifications !== false;
      showTransferProgress = cfg.showTransferProgress !== false;
      superDoubleTapMs = typeof cfg.superDoubleTapMs === 'number' ? Math.min(800, Math.max(150, cfg.superDoubleTapMs)) : 350;
    });

    cleanupKeyboard = initKeyboardShortcuts({
      onToggleStartMenu: () => (isStartMenuOpen = !isStartMenuOpen),
      onOpenFullScreenMenu: () => { isStartMenuOpen = true; isStartMenuFullScreen = true; },
      onToggleControlCenter: () => (isControlCenterOpen = !isControlCenterOpen),
      onSuperTap: () => superTap(),
      isSwitcherVisible: () => switcherVisible,
      switcherIndex: () => switcherIndex,
      setSwitcherVisible: (v) => (switcherVisible = v),
      setSwitcherIndex: (updater) => (switcherIndex = updater(switcherIndex)),
    });

    // Win: pierwszy tap otwiera/zamyka menu; DRUGI tap w oknie czasowym
    // (domyślnie 350 ms) rozwija je do pełnego ekranu ("Win + Win").
    // Brak opóźnienia przy pierwszym tapie — menu pokazuje się od razu.
    const superTap = () => {
      const now = Date.now();
      if (isStartMenuOpen && !isStartMenuFullScreen && now - lastSuperTap <= superDoubleTapMs) {
        isStartMenuFullScreen = true;
      } else if (isStartMenuOpen) {
        isStartMenuOpen = false;
        isStartMenuFullScreen = false;
      } else {
        isStartMenuOpen = true;
        isStartMenuFullScreen = false;
      }
      lastSuperTap = now;
    };
    const runCornerAction = (slot: HotCornerSlot) => {
      switch (slot.action) {
        case 'start-menu': isStartMenuOpen = true; isStartMenuFullScreen = false; break;
        case 'fullscreen-menu': isStartMenuOpen = true; isStartMenuFullScreen = true; break;
        case 'show-desktop': showDesktop(); break;
        case 'workspace-next': switchWorkspace(get(currentWorkspace) + 1); break;
        case 'workspace-prev': switchWorkspace(get(currentWorkspace) - 1); break;
        case 'window-switcher': stepSwitcher(1); setTimeout(() => { if (switcherVisible) { /* commit przy ruchu myszy */ } }, 0); break;
        case 'control-center': isControlCenterOpen = !isControlCenterOpen; break;
        case 'notifications': isNotificationsOpen = !isNotificationsOpen; break;
        case 'lock': CompositorBridge.lockScreen(); break;
        case 'screenshot': takeScreenshot(); break;
        case 'terminal': openApp(AppId.TERMINAL); break;
        case 'open-app': if (slot.appId) openApp(slot.appId); break;
      }
    };
    cornerHandler = runCornerAction;

    const closePanels = () => { isStartMenuOpen = false; isControlCenterOpen = false; isNotificationsOpen = false; isClipboardOpen = false; showPowerMenu = false; };
    const toggleClip = () => (isClipboardOpen = !isClipboardOpen);
    const openTerm = () => openApp(AppId.TERMINAL);
    const showDesktop = () => {
      for (const w of get(windows)) minimizeWindow(w.id);
      isNativeBackend().then((native) => { if (native) CompositorBridge.showDesktopNative(); });
    };
    // Alt+Tab pressed while the Blue shell has focus: the compositor consumes the key
    // (its keybind) and forwards it here instead, so step Blue's own
    // switcher exactly as the Tab key handler would. Alt release — which the
    // webview still receives — commits the choice (keyboardShortcuts.ts).
    const stepSwitcher = (dir: 1 | -1) => {
      const wins = switcherItems;
      if (wins.length === 0) return;
      if (!switcherVisible) {
        const cur = wins.findIndex((w) => w.id === get(activeWindowId));
        switcherIndex = cur === -1 ? (dir > 0 ? 0 : wins.length - 1) : (cur + dir + wins.length) % wins.length;
        switcherVisible = true;
      } else {
        switcherIndex = (switcherIndex + dir + wins.length) % wins.length;
      }
    };
    const takeScreenshot = async () => {
      const path = await takeScreenshotUnified('full').catch(() => null);
      notificationManager.add({
        title: path ? 'Screenshot saved' : 'Screenshot failed',
        message: path || 'No screenshot tool available (install grim).',
        appId: AppId.BLUE_SCREEN, icon: '',
      });
    };
    window.addEventListener('blue:close-panels', closePanels);
    window.addEventListener(CLOSE_POPUPS_EVENT, closePopupsNow);
    // Focus moved to another app (native window, or a Blue Web page — those are separate
    // native surfaces that never send DOM events to the shell).
    const onWindowBlur = () => { if (Date.now() - popupOpenedAt > 400) closePopupsNow(); };
    window.addEventListener('blur', onWindowBlur);
    // Another window became the active one (click, Alt+Tab commit, taskbar, newly opened app).
    let lastActive = get(activeWindowId);
    const unsubActive = activeWindowId.subscribe((id) => {
      if (id !== lastActive) { lastActive = id; if (id && Date.now() - popupOpenedAt > 150) closePopupsNow(); }
    });
    window.addEventListener('blue:toggle-clipboard', toggleClip);
    window.addEventListener('blue:open-terminal', openTerm);
    // These three were dispatched by keyboardShortcuts.ts (Print, Super+L,
    // Super+D) but nothing ever listened for them.
    window.addEventListener('blue:show-desktop', showDesktop);
    window.addEventListener('blue:screenshot', takeScreenshot);
    const lockScreen = () => { CompositorBridge.lockScreen(); };
    window.addEventListener('blue:lock-screen', lockScreen);

    // ── Compositor-level shortcuts ───────────────────────────────────────
    // `blue-environment --ctl <cmd>` (labwc/sway/wayfire keybinds) and HackerOS-Comp's
    // own `toggle_start_menu` both end up here, so shortcuts work while a
    // native app has keyboard focus and this webview receives no key events.
    const unlistenShell = CompositorBridge.onShellCommand((cmd, arg) => {
      switch (cmd) {
        case 'toggle-start-menu': isStartMenuOpen = !isStartMenuOpen; if (!isStartMenuOpen) isStartMenuFullScreen = false; break;
        case 'super-tap': superTap(); break;
        case 'workspace-sync': if (arg) syncWorkspaceFromCompositor(arg); break;
        case 'workspace-next': switchWorkspace(get(currentWorkspace) + 1); break;
        case 'workspace-prev': switchWorkspace(get(currentWorkspace) - 1); break;
        case 'fullscreen-menu': isStartMenuOpen = true; isStartMenuFullScreen = true; break;
        case 'toggle-control-center': isControlCenterOpen = !isControlCenterOpen; break;
        case 'toggle-notifications': isNotificationsOpen = !isNotificationsOpen; break;
        case 'toggle-clipboard': isClipboardOpen = !isClipboardOpen; break;
        case 'open-terminal': openApp(AppId.TERMINAL); break;
        case 'screenshot': takeScreenshot(); break;
        case 'lock': lockScreen(); break;
        case 'show-desktop': showDesktop(); break;
        case 'close-panels': closePanels(); break;
        case 'switcher-next': stepSwitcher(1); break;
        case 'switcher-prev': stepSwitcher(-1); break;
        case 'open-app': if (arg) openApp(arg); break;
      }
    });

    // ── Aktualizacje powłoki (shell_update.rs) ─────────────────────────────
    // Ciche sprawdzanie w tle; użytkownik widzi coś dopiero, gdy jest nowa wersja.
    const unlistenUpdAvail = listen<{ version: string; current: string }>('shell-update-available', (ev) => {
      const v = ev.payload.version;
      notificationManager.add({
        title: translate('supd.n_avail_title'),
        message: translate('supd.n_avail_msg', { v, c: ev.payload.current }),
        appId: 'settings', icon: '',
        actions: [{ label: translate('supd.n_ignore'), action: `shell-update:ignore:${v}` }, { label: translate('supd.n_update'), action: `shell-update:install:${v}` }],
      });
      window.dispatchEvent(new CustomEvent('blue:show-toast', { detail: { title: translate('supd.n_avail_title'), message: v } }));
    });
    const unlistenUpdStaged = listen<{ version: string }>('shell-update-staged', (ev) => {
      notificationManager.add({
        title: translate('supd.n_staged_title'),
        message: translate('supd.n_staged_msg', { v: ev.payload.version }),
        appId: 'settings', icon: '',
      });
    });
    const onNotifAction = (e: Event) => {
      const action = String((e as CustomEvent).detail?.action ?? '');
      const m = action.match(/^shell-update:(ignore|install):(.+)$/);
      if (!m) return;
      if (m[1] === 'ignore') invoke('shell_update_ignore', { version: m[2] }).catch(() => {});
      else invoke('shell_update_install', { version: m[2] }).catch((err) =>
        notificationManager.add({ title: translate('supd.n_failed'), message: String(err), appId: 'settings', icon: '' }));
    };
    window.addEventListener('blue:notification-action', onNotifAction);

    // ── Urządzenia (devices.rs): pendrive, dysk, karta SD, sprzęt USB ──────
    // Jak „Powiadamianie o urządzeniach" w KDE: powiadomienie z akcjami
    // Otwórz / Wysuń. Wyłączane w Ustawieniach → Urządzenia.
    interface StorageEv { name: string; label: string; sizeBytes: number; fstype: string; mountpoint: string | null; model: string; vendor: string }
    interface UsbEv { id: string; vendorId: string; productId: string; manufacturer: string; product: string; class: string }
    const fmtSize = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${Math.round(b / 1e6)} MB` : '');
    const unlistenStorageAdd = listen<StorageEv>('device:storage-added', (ev) => {
      if (!deviceNotifications) return;
      const d = ev.payload;
      const title = d.label || [d.vendor, d.model].filter(Boolean).join(' ') || d.name;
      const detail = [fmtSize(d.sizeBytes), d.fstype].filter(Boolean).join(' · ');
      notificationManager.add({
        title: translate('device.storage_title'),
        message: detail ? `${title} (${detail})` : title,
        appId: 'explorer', icon: '',
        actions: [{ label: translate('device.open'), action: `device:open:${d.name}:${title}` }, { label: translate('device.eject'), action: `device:eject:${d.name}:${title}` }],
      });
      window.dispatchEvent(new CustomEvent('blue:show-toast', { detail: { title: translate('device.storage_title'), message: title } }));
    });
    const unlistenStorageRemoved = listen<{ name: string }>('device:storage-removed', (ev) => {
      if (!deviceNotifications) return;
      window.dispatchEvent(new CustomEvent('blue:show-toast', { detail: { title: translate('device.removed'), message: ev.payload.name } }));
    });
    const unlistenUsbAdd = listen<UsbEv>('device:usb-added', (ev) => {
      const d = ev.payload;
      // Pamięć masowa (klasa 08) dostaje własne powiadomienie z akcjami (storage-added).
      if (!deviceNotifications || d.class === '08') return;
      const name = [d.manufacturer, d.product].filter(Boolean).join(' ') || `${d.vendorId}:${d.productId}`;
      notificationManager.add({ title: translate('device.usb_title'), message: name, appId: 'settings', icon: '' });
      window.dispatchEvent(new CustomEvent('blue:show-toast', { detail: { title: translate('device.usb_title'), message: name } }));
    });
    const onDeviceAction = async (e: Event) => {
      const m = String((e as CustomEvent).detail?.action ?? '').match(/^device:(open|eject):([A-Za-z0-9]+):?(.*)$/);
      if (!m) return;
      const [, kind, dev, title] = m;
      try {
        if (kind === 'open') {
          const mountpoint = await invoke<string>('device_mount', { name: dev });
          openApp(AppId.EXPLORER, false, undefined, { initialPath: mountpoint }, title || dev);
        } else {
          await invoke('device_eject', { name: dev });
          notificationManager.add({ title: translate('device.ejected'), message: title || dev, appId: 'explorer', icon: '' });
        }
      } catch (err) {
        notificationManager.add({ title: translate(kind === 'open' ? 'device.mount_failed' : 'device.eject_failed'), message: String(err), appId: 'explorer', icon: '' });
      }
    };
    window.addEventListener('blue:notification-action', onDeviceAction);

    // Shell overlays are drawn by the shell, which on every native backend sits beneath
    // native windows — tuck those away while any overlay is open.
    const unsubOverlayPeek = blockingOverlayOpen.subscribe((open) => {
      isNativeBackend().then((native) => { if (native) CompositorBridge.peekNatives(open); });
    });

    const unlistenLaunchFailed = CompositorBridge.onLaunchFailed((command, detail) => {
      notificationManager.add({
        title: 'Could not start application',
        message: `${command}\n${detail}`,
        appId: 'settings', icon: '',
      });
    });

    SystemBridge.getBackendInfo().then((info) => { if (info) backendActive = info.active; });

    return () => {
      unlistenShell.then((f) => f());
      unlistenUpdAvail.then((f) => f());
      unlistenStorageAdd.then((f) => f());
      unlistenStorageRemoved.then((f) => f());
      unlistenUsbAdd.then((f) => f());
      window.removeEventListener('blue:notification-action', onDeviceAction);
      unlistenUpdStaged.then((f) => f());
      window.removeEventListener('blue:notification-action', onNotifAction);
      unlistenLaunchFailed.then((f) => f());
      unsubOverlayPeek();
      unsubConfig();
      window.removeEventListener('blue:close-panels', closePanels);
      window.removeEventListener(CLOSE_POPUPS_EVENT, closePopupsNow);
      window.removeEventListener('blur', onWindowBlur);
      unsubActive();
      window.removeEventListener('blue:toggle-clipboard', toggleClip);
      window.removeEventListener('blue:open-terminal', openTerm);
      window.removeEventListener('blue:show-desktop', showDesktop);
      window.removeEventListener('blue:screenshot', takeScreenshot);
      window.removeEventListener('blue:lock-screen', lockScreen);
    };
  });

  onDestroy(() => {
    stopExternalWindowPolling();
    stopParentalControlsUsageTracking();
    cleanupKeyboard?.();
    document.removeEventListener('focusin', onGlobalFocusIn);
  });

  function handlePower(e: CustomEvent) {
    showPowerMenu = false;
    SystemBridge.powerAction(e.detail);
  }

  $: openWindowSummaries = [
    ...$windows.map((w) => ({
      id: w.id,
      appId: w.appId as AppId | undefined,
      isMinimized: w.isMinimized,
      isActive: w.id === $activeWindowId,
      workspace: w.workspace,
    })),
    // External (native, non-Blue-Environment) windows have no `appId`
    // in this shell's own registry, so they can never match a pinned
    // app's icon in TopBar's center dock — but they still belong in
    // this list for the workspace-dot indicator (`hasWins` in
    // TopBar.svelte), which only checks `workspace`/`isMinimized`, not
    // `appId`. Without this, switching to a workspace that only has an
    // external app open would show that workspace's dot as empty.
    ...$externalWindows.map((w) => ({
      id: w.id,
      appId: undefined as AppId | undefined,
      isMinimized: w.isMinimized,
      isActive: false,
      workspace: w.desktop ?? 0,
    })),
  ];

  $: windowCounts = Array.from({ length: $workspaceCount }, (_, i) => $windows.filter((w) => w.workspace === i).length);

  // `getSwitcherItems` takes both lists as plain arguments (see its own
  // doc in windowManager.ts for why) — referencing `$windows`/
  // `$externalWindows` directly here is what makes this recompute
  // whenever either list changes.
  $: switcherItems = getSwitcherItems($windows, $externalWindows);

  // StartMenu (both the popup dropdown and the fullscreen app drawer) is
  // meant to always sit above every window, including a maximized/
  // fullscreen/PiP one — that's the whole point of it being a modal
  // overlay you explicitly opened. A hardcoded z-index (it used to be
  // Tailwind's z-40/z-50) breaks the moment the session's window
  // zIndex counter — which only ever increments, on every open/focus/
  // restore — climbs past that fixed number, which happens after
  // perfectly ordinary usage (well under 40 focus events). This instead
  // always stays a fixed margin above whatever the highest window
  // zIndex currently is, so it can never be silently overtaken again.
  $: startMenuZIndex = Math.max(50, ...$windows.map((w) => w.zIndex)) + 100;
  // The desktop backdrop's fallback gradient (used whenever no
  // wallpaper image is set) was hardcoded to a dark navy gradient
  // regardless of light/dark mode — the very first thing rendered, and
  // wrong for anyone on light mode with no wallpaper picked yet.
  $: fallbackBgColor = theme === 'light-glass' ? '#f1f5f9' : '#0f172a';
  $: fallbackGradient = theme === 'light-glass'
    ? 'linear-gradient(160deg, #f1f5f9, #e2e8f0)'
    : 'linear-gradient(160deg, #0f172a, #1e293b)';
</script>

{#if $liveModeChecked && $isLiveMode}
  <!-- Live session: the installer stays mounted (just hidden) while the
       classic desktop is shown, so nothing entered so far is lost. -->
  <BlueInstallerApp hidden={$liveView === 'desktop'} />
{/if}
{#if $liveModeChecked && (!$isLiveMode || $liveView === 'desktop')}
<ShellThemeStyle {shellThemeId} />
<SystemThemeStyle {systemThemeId} />
<div
  class="relative w-full h-full overflow-hidden select-none"
  data-theme={theme}
  data-backend={backendActive}
  style="background-color:{fallbackBgColor}; font-family:var(--shell-font, inherit);"
  on:click|self={() => { isStartMenuOpen = false; isControlCenterOpen = false; isNotificationsOpen = false; }}
>
  <!-- Dedicated wallpaper layer, separate from the content below it, so
       a theme's `wallpaperBlur` (extras.wallpaperBlur — see
       ShellThemeStyle.svelte / customThemes.ts) blurs only the
       wallpaper image itself. `filter: blur()` on this container's
       *parent* would blur the desktop icons, TopBar and every open
       window too, since CSS filters apply to an element's entire
       subtree — this absolutely-positioned, negative-inset sibling
       layer (scaled up slightly via the inset trick below to hide the
       blurred edge that would otherwise show at the viewport border)
       keeps the blur contained to just the background. -->
  <div
    class="absolute pointer-events-none"
    style="inset:-40px; background-size:cover; background-position:center; background-image:{wallpaperCss ? `url('${wallpaperCss}'), ` : ''}{fallbackGradient}; filter:blur(var(--wallpaper-blur, 0px));"
    aria-hidden="true"
  ></div>

  <Desktop {desktopPath} on:closeMenus={closePopupsNow} />

  <TopBar
    openWindows={openWindowSummaries}
    currentWorkspace={$currentWorkspace}
    workspaceCount={$workspaceCount}
    {isStartMenuOpen}
    {isClipboardOpen}
    enabled={panelEnabled}
    position={effectivePanelPosition}
    shellThemeId={activeShellTheme?.id}
    on:openApp={(e) => openApp(e.detail)}
    on:toggleWindow={(e) => toggleWindowFromTaskbar(e.detail)}
    on:startClick={() => (isStartMenuOpen = !isStartMenuOpen)}
    on:startDoubleClick={() => { isStartMenuOpen = true; isStartMenuFullScreen = true; }}
    on:toggleControlCenter={() => (isControlCenterOpen = !isControlCenterOpen)}
    on:toggleNotifications={() => (isNotificationsOpen = !isNotificationsOpen)}
    on:switchWorkspace={(e) => switchWorkspace(e.detail)}
    on:toggleClipboard={() => (isClipboardOpen = !isClipboardOpen)}
  />

  <StartMenu
    isOpen={isStartMenuOpen}
    isFullScreen={isStartMenuFullScreen}
    {appsEnabled}
    zIndex={startMenuZIndex}
    panelPosition={effectivePanelPosition}
    panelSize={barHeight}
    shellThemeId={activeShellTheme?.id}
    on:openApp={(e) => openApp(e.detail.appId, e.detail.isExternal, e.detail.exec)}
    on:close={() => { isStartMenuOpen = false; isStartMenuFullScreen = false; }}
    on:toggleFullScreen={() => (isStartMenuFullScreen = !isStartMenuFullScreen)}
  />

  {#each $visibleWindows as win (win.id)}
    {@const appDef = getAppDef(win.appId)}
    <WindowComponent
      {win}
      isActive={win.id === $activeWindowId}
      {barHeight}
      panelPosition={effectivePanelPosition}
      shellThemeId={activeShellTheme?.id}
      on:close={(e) => closeWindow(e.detail)}
      on:minimize={(e) => minimizeWindow(e.detail)}
      on:maximize={(e) => maximizeWindow(e.detail)}
      on:pip={(e) => togglePiP(e.detail)}
      on:focus={(e) => focusWindow(e.detail)}
      on:move={(e) => moveWindow(e.detail.id, e.detail.x, e.detail.y)}
      on:resize={(e) => resizeWindow(e.detail.id, e.detail.width, e.detail.height)}
    >
      {#if appDef?.component}
        <ErrorBoundary component={appDef.component} appTitle={win.title} props={{ windowId: win.id, ...win.launchArgs }} />
      {:else}
        <div class="flex items-center justify-center h-full theme-bg-primary theme-text-secondary text-sm">
          External app — managed by the compositor
        </div>
      {/if}
    </WindowComponent>
  {/each}

  <WindowSwitcher windows={switcherItems} selectedIndex={switcherIndex} isVisible={switcherVisible} />
  <HotCorners config={hotCornersCfg} on:trigger={(e) => cornerHandler(e.detail)} />
  <WorkspaceSwitcher currentWorkspace={$currentWorkspace} workspaceCount={$workspaceCount} {windowCounts} />

  <ControlCenter isOpen={isControlCenterOpen} panelPosition={effectivePanelPosition} panelSize={barHeight} shellThemeId={activeShellTheme?.id} on:openSettings={() => { openApp(AppId.SETTINGS); isControlCenterOpen = false; }} />
  <NotificationCenter isOpen={isNotificationsOpen} panelPosition={effectivePanelPosition} panelSize={barHeight} shellThemeId={activeShellTheme?.id} on:close={() => (isNotificationsOpen = false)} />
  {#if isClipboardOpen}
    <ClipboardPanel on:close={() => (isClipboardOpen = false)} />
  {/if}

  {#if showPowerMenu}
    <PowerMenu shellThemeId={activeShellTheme?.id} on:action={handlePower} on:close={() => (showPowerMenu = false)} />
  {/if}

  {#if $isLiveMode}
    <LiveInstallerReturn zIndex={startMenuZIndex - 10} panelPosition={effectivePanelPosition} panelSize={barHeight} />
  {/if}

  <ToastContainer />
  <ContextMenu />
  <DialogHost />
  <ConflictDialog />
  <TransferProgress enabled={showTransferProgress} />
  <BlueFilePicker />
  <OnscreenKeyboard bind:visible={onscreenKeyboardVisible} />
</div>
{/if}
