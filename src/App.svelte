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
  import BlueInstallerApp from './lib/components/apps/Blue-Installer/BlueInstallerApp.svelte';
  import { isLiveMode, liveModeChecked, checkLiveMode } from './lib/utils/liveMode';
  import { SystemBridge, toAssetUrl } from './lib/utils/systemBridge';

  let wallpaper = 'file:///usr/share/Blue-Environment/wallpapers/default.png';
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

  let cleanupKeyboard: () => void;

  onMount(() => {
    checkLiveMode();
    initLanguage();
    startExternalWindowPolling();
    startParentalControlsUsageTracking();

    configStore.init().then((cfg) => {
      if (cfg.wallpaper) wallpaper = cfg.wallpaper;
      if (cfg.theme) theme = cfg.theme;
      if (cfg.appsEnabled) appsEnabled = cfg.appsEnabled;
      if (cfg.desktopPath) desktopPath = cfg.desktopPath;
      if (typeof cfg.panelEnabled === 'boolean') panelEnabled = cfg.panelEnabled;
      if (cfg.panelPosition === 'top' || cfg.panelPosition === 'bottom') panelPosition = cfg.panelPosition;
      if (typeof cfg.panelSize === 'number' && cfg.panelSize > 0) panelSize = cfg.panelSize;
    });
    const unsubConfig = configStore.subscribe((cfg) => {
      if (cfg.wallpaper) wallpaper = cfg.wallpaper;
      if (cfg.theme) theme = cfg.theme;
      if (cfg.appsEnabled) appsEnabled = cfg.appsEnabled;
      if (cfg.desktopPath) desktopPath = cfg.desktopPath;
      if (typeof cfg.panelEnabled === 'boolean') panelEnabled = cfg.panelEnabled;
      if (cfg.panelPosition === 'top' || cfg.panelPosition === 'bottom') panelPosition = cfg.panelPosition;
      if (typeof cfg.panelSize === 'number' && cfg.panelSize > 0) panelSize = cfg.panelSize;
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
<div
  class="relative w-full h-full overflow-hidden select-none"
  data-theme={theme}
  style="background-image:url({toAssetUrl(wallpaper)}); background-size:cover; background-position:center;"
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
    position={panelPosition}
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
    {panelPosition}
    panelSize={barHeight}
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
      {panelPosition}
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

  <ControlCenter isOpen={isControlCenterOpen} on:openSettings={() => { openApp(AppId.SETTINGS); isControlCenterOpen = false; }} />
  <NotificationCenter isOpen={isNotificationsOpen} on:close={() => (isNotificationsOpen = false)} />
  {#if isClipboardOpen}
    <ClipboardPanel on:close={() => (isClipboardOpen = false)} />
  {/if}

  {#if showPowerMenu}
    <PowerMenu on:action={handlePower} on:close={() => (showPowerMenu = false)} />
  {/if}

  <ToastContainer />
  <DialogHost />
</div>
{/if}
