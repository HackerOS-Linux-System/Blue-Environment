<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { configStore } from './lib/utils/configStore';
  import { initLanguage } from './lib/stores/language';
  import {
    windows, visibleWindows, activeWindowId, currentWorkspace, workspaceCount,
    openApp, closeWindow, focusWindow, minimizeWindow, maximizeWindow, togglePiP,
    moveWindow, resizeWindow, toggleWindowFromTaskbar, switchWorkspace,
    startExternalWindowPolling, stopExternalWindowPolling,
    startParentalControlsUsageTracking, stopParentalControlsUsageTracking,
  } from './lib/stores/windowManager';
  import { initKeyboardShortcuts } from './lib/stores/keyboardShortcuts';
  import { shellOverlayOpen } from './lib/stores/overlayState';
  import { hasCompletedWelcome } from './lib/components/apps/Blue-Welcome-App/welcome';
  import { createNotificationsStore } from './lib/components/apps/Blue-Notifications-App/notificationsStore';
  import { APPS } from './lib/constants';
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
  import WorkspaceSwitcher from './lib/components/WorkspaceSwitcher.svelte';
  import DialogHost from './lib/components/DialogHost.svelte';
  import ShellThemeStyle from './lib/components/ShellThemeStyle.svelte';
  import SystemThemeStyle from './lib/components/SystemThemeStyle.svelte';
  import BlueInstallerApp from './lib/components/apps/Blue-Installer/BlueInstallerApp.svelte';
  import { isLiveMode, liveModeChecked, checkLiveMode } from './lib/utils/liveMode';
  import { resolveActiveShellTheme } from './lib/data/builtinThemes';
  import { SystemBridge, toAssetUrl } from './lib/utils/systemBridge';

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
  let systemThemeId: string | null | undefined;

  // Active shell theme (Hydra etc. — see builtinThemes.ts) overrides the
  // person's own wallpaper/panel-position choices while selected, same
  // way `data-shell-theme`'s CSS overrides work in ShellThemeStyle.svelte
  // — `resolveActiveShellTheme` is the single shared source of truth
  // both use for "is there a real override active right now".
  $: activeShellTheme = resolveActiveShellTheme(shellThemeId);
  $: effectiveWallpaper = activeShellTheme?.wallpaper || wallpaper;
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
    return APPS[appId as AppId];
  }

  let isStartMenuOpen = false;
  let isStartMenuFullScreen = false;
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

  let cleanupKeyboard: () => void;

  onMount(() => {
    checkLiveMode();
    initLanguage();
    startExternalWindowPolling();
    startParentalControlsUsageTracking();

    // First-run wizard — see welcome.ts's doc comment. Deferred one tick
    // (not called synchronously before anything else) so it opens as an
    // ordinary window on top of an already-initializing desktop rather
    // than racing window-manager/config setup that hasn't run yet.
    if (!hasCompletedWelcome()) {
      setTimeout(() => openApp(AppId.BLUE_WELCOME), 300);
    }

    // Blue Notifications' feed-watcher polling — starts once here,
    // unconditionally, independent of whether a Blue Notifications
    // window is ever opened this session. See notificationsStore.ts's
    // `startPolling` doc comment for exactly what "background" does and
    // doesn't mean (no OS daemon, just timers living as long as this
    // shell process does).
    createNotificationsStore().startPolling();

    configStore.init().then((cfg) => {
      if (cfg.wallpaper) wallpaper = cfg.wallpaper;
      if (cfg.theme) theme = cfg.theme;
      if (cfg.appsEnabled) appsEnabled = cfg.appsEnabled;
      if (cfg.desktopPath) desktopPath = cfg.desktopPath;
      if (typeof cfg.panelEnabled === 'boolean') panelEnabled = cfg.panelEnabled;
      if (cfg.panelPosition === 'top' || cfg.panelPosition === 'bottom') panelPosition = cfg.panelPosition;
      if (typeof cfg.panelSize === 'number' && cfg.panelSize > 0) panelSize = cfg.panelSize;
      shellThemeId = cfg.shellThemeId;
      systemThemeId = cfg.systemThemeId;
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
    });

    cleanupKeyboard = initKeyboardShortcuts({
      onToggleStartMenu: () => (isStartMenuOpen = !isStartMenuOpen),
      onOpenFullScreenMenu: () => { isStartMenuOpen = true; isStartMenuFullScreen = true; },
      onToggleControlCenter: () => (isControlCenterOpen = !isControlCenterOpen),
      isSwitcherVisible: () => switcherVisible,
      switcherIndex: () => switcherIndex,
      setSwitcherVisible: (v) => (switcherVisible = v),
      setSwitcherIndex: (updater) => (switcherIndex = updater(switcherIndex)),
    });

    const closePanels = () => { isStartMenuOpen = false; isControlCenterOpen = false; isNotificationsOpen = false; isClipboardOpen = false; showPowerMenu = false; };
    const toggleClip = () => (isClipboardOpen = !isClipboardOpen);
    const openTerm = () => openApp(AppId.TERMINAL);
    window.addEventListener('blue:close-panels', closePanels);
    window.addEventListener('blue:toggle-clipboard', toggleClip);
    window.addEventListener('blue:open-terminal', openTerm);

    return () => {
      unsubConfig();
      window.removeEventListener('blue:close-panels', closePanels);
      window.removeEventListener('blue:toggle-clipboard', toggleClip);
      window.removeEventListener('blue:open-terminal', openTerm);
    };
  });

  onDestroy(() => {
    stopExternalWindowPolling();
    stopParentalControlsUsageTracking();
    cleanupKeyboard?.();
  });

  function handlePower(e: CustomEvent) {
    showPowerMenu = false;
    SystemBridge.powerAction(e.detail);
  }

  $: openWindowSummaries = $windows.map((w) => ({
    id: w.id,
    appId: w.appId as AppId,
    isMinimized: w.isMinimized,
    isActive: w.id === $activeWindowId,
    workspace: w.workspace,
  }));

  $: windowCounts = Array.from({ length: $workspaceCount }, (_, i) => $windows.filter((w) => w.workspace === i).length);

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
</script>

{#if $liveModeChecked && $isLiveMode}
  <BlueInstallerApp />
{:else if $liveModeChecked}
<ShellThemeStyle {shellThemeId} />
<SystemThemeStyle {systemThemeId} />
<div
  class="relative w-full h-full overflow-hidden select-none"
  data-theme={theme}
  style="background-size:cover; background-position:center; background-color:#0f172a; background-image:{effectiveWallpaper ? `url(${toAssetUrl(effectiveWallpaper)}), ` : ''}linear-gradient(160deg, #0f172a, #1e293b);"
  on:click|self={() => { isStartMenuOpen = false; isControlCenterOpen = false; isNotificationsOpen = false; }}
>
  <Desktop {desktopPath} on:closeMenus={() => { isStartMenuOpen = false; isControlCenterOpen = false; isNotificationsOpen = false; isClipboardOpen = false; showPowerMenu = false; }} />

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

  <WindowSwitcher windows={$windows} selectedIndex={switcherIndex} isVisible={switcherVisible} />
  <WorkspaceSwitcher currentWorkspace={$currentWorkspace} workspaceCount={$workspaceCount} {windowCounts} />

  <ControlCenter isOpen={isControlCenterOpen} panelPosition={effectivePanelPosition} panelSize={barHeight} shellThemeId={activeShellTheme?.id} on:openSettings={() => { openApp(AppId.SETTINGS); isControlCenterOpen = false; }} />
  <NotificationCenter isOpen={isNotificationsOpen} panelPosition={effectivePanelPosition} panelSize={barHeight} shellThemeId={activeShellTheme?.id} on:close={() => (isNotificationsOpen = false)} />
  {#if isClipboardOpen}
    <ClipboardPanel on:close={() => (isClipboardOpen = false)} />
  {/if}

  {#if showPowerMenu}
    <PowerMenu shellThemeId={activeShellTheme?.id} on:action={handlePower} on:close={() => (showPowerMenu = false)} />
  {/if}

  <ToastContainer />
  <DialogHost />
</div>
{/if}
