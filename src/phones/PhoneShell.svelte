<script lang="ts">
  /**
   * Blue Environment — PHONE shell.
   *
   * One app at a time, full screen, with a status bar on top and a three-button
   * navigation bar at the bottom (Back / Home / Recents; swipe up on the bar
   * also goes Home). The home screen has a clock, a search field, a paged app
   * grid (swipe left/right) and a dock of pinned apps. Pulling down the status
   * bar opens notifications and quick settings.
   *
   * It reuses the desktop shell's window manager and app registry, so every
   * Blue app works unchanged — they simply fill the screen.
   */
  import { onDestroy, onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { windows, activeWindowId, focusWindow, closeWindow, openApp } from '../lib/stores/windowManager';
  import { APPS } from '../lib/constants';
  import { AppId } from '../lib/types';
  import { startAltShell } from '../lib/altShell/boot';
  import { listLauncherApps, resolvePinned, type RecentItem } from '../lib/altShell/apps';
  import { goHome, dispatchBack } from '../lib/altShell/shellUtils';
  import { notificationManager } from '../lib/utils/notificationManager';
  import { normalizeTopBar } from '../lib/utils/topBarConfig';
  import { SystemBridge } from '../lib/utils/systemBridge';
  import ShellFrame from '../lib/altShell/ShellFrame.svelte';
  import AppViewport from '../lib/altShell/AppViewport.svelte';
  import PowerMenu from '../lib/components/PowerMenu.svelte';
  import type { PowerAction, Notification } from '../lib/types';
  import StatusBar from './StatusBar.svelte';
  import NavBar from './NavBar.svelte';
  import HomeScreen from './HomeScreen.svelte';
  import Recents from './Recents.svelte';
  import Shade from './Shade.svelte';

  let wallpaper = $state('');
  let theme = $state('dark');
  let shellThemeId = $state<string | undefined>();
  let systemThemeId = $state<string | null | undefined>();
  let appsEnabled = $state<Record<string, boolean>>({});
  let pinnedIds = $state<string[] | undefined>();
  let use24h = $state(true);
  let showTransferProgress = $state(true);
  let keyboardOnFocus = $state(true);

  const stop = startAltShell((cfg) => {
    if (cfg.wallpaper) wallpaper = cfg.wallpaper;
    if (cfg.theme) theme = cfg.theme;
    shellThemeId = cfg.shellThemeId;
    systemThemeId = cfg.systemThemeId;
    if (cfg.appsEnabled) appsEnabled = cfg.appsEnabled;
    pinnedIds = cfg.pinnedApps;
    use24h = normalizeTopBar(cfg.topBar).clock24h;
    showTransferProgress = cfg.showTransferProgress !== false;
    // A phone has no hardware keyboard: default to ON unless explicitly switched off.
    keyboardOnFocus = cfg.onscreenKeyboardEnabled !== false;
  });
  onDestroy(stop);

  let now = $state(new Date());
  let clock: ReturnType<typeof setInterval>;
  onMount(() => { clock = setInterval(() => (now = new Date()), 10000); });
  onDestroy(() => clearInterval(clock));

  type Panel = 'none' | 'recents' | 'shade' | 'power';
  let panel = $state<Panel>('none');
  let shadeTab = $state<'notifications' | 'quick'>('notifications');

  let apps = $derived(listLauncherApps(APPS as any, appsEnabled));
  let dock = $derived(resolvePinned(pinnedIds, apps, 4));

  let running = $derived($windows.filter((w) => !w.isExternal).sort((a, b) => b.zIndex - a.zIndex));
  let appOpen = $derived(running.some((w) => w.id === $activeWindowId && !w.isMinimized));
  let recents = $derived(running.map<RecentItem>((w) => ({
    id: w.id, title: w.title, icon: (APPS as any)[w.appId]?.icon, active: w.id === $activeWindowId && !w.isMinimized,
  })));

  let unread = $state(0);
  let allNotes: Notification[] = [];
  const unsubNotes = notificationManager.subscribe((n) => { allNotes = n; unread = n.filter((x) => !x.read).length; });
  onDestroy(unsubNotes);

  function launch(id: string) { panel = 'none'; openApp(id); }
  function home() { panel = 'none'; goHome(); }
  function back() {
    if (panel !== 'none') { panel = 'none'; return; }
    if (dispatchBack()) return; // an app handled it
    if (appOpen) goHome();
  }
  function toggleRecents() { panel = panel === 'recents' ? 'none' : 'recents'; }
  function openFromRecents(id: string) { panel = 'none'; focusWindow(id); }
  function clearAll() { for (const w of running) closeWindow(w.id); panel = 'none'; }
  // Opening the shade means the person has seen the notifications.
  function markRead() { for (const n of allNotes) if (!n.read) notificationManager.markRead(n.id); }
  function onPower(e: CustomEvent<PowerAction>) { panel = 'none'; SystemBridge.powerAction(e.detail); }
  function openSettings() { panel = 'none'; openApp(AppId.SETTINGS); }

  function onKey(e: KeyboardEvent) { if (e.key === 'Escape') { e.preventDefault(); back(); } }
</script>

<svelte:window onkeydown={onKey} />

<ShellFrame variant="phones" {wallpaper} {shellThemeId} {systemThemeId} {theme} {keyboardOnFocus} {showTransferProgress}>
  <div class="absolute inset-0 flex flex-col">
    <StatusBar {now} {use24h} {unread} on:pull={() => { shadeTab = 'notifications'; panel = 'shade'; }} />

    <main class="relative flex-1 min-h-0 flex flex-col">
      {#if !appOpen}
        <HomeScreen {apps} {dock} {now} {use24h} on:launch={(e) => launch(e.detail)} />
      {/if}
      <AppViewport />

      {#if panel === 'recents'}
        <Recents items={recents} on:open={(e) => openFromRecents(e.detail)} on:close={(e) => closeWindow(e.detail)}
          on:clear={clearAll} on:dismiss={() => (panel = 'none')} />
      {/if}
    </main>

    <NavBar on:back={back} on:home={home} on:recents={toggleRecents} />
  </div>

  {#if panel === 'shade'}
    <Shade tab={shadeTab} on:dismiss={() => (panel = 'none')} on:settings={openSettings} on:power={() => (panel = 'power')}
      on:read={markRead} />
  {/if}
  {#if panel === 'power'}
    <div class="absolute inset-0 z-50" transition:fade={{ duration: 120 }}>
      <PowerMenu zIndex={60} on:action={onPower} on:close={() => (panel = 'none')} />
    </div>
  {/if}
</ShellFrame>
