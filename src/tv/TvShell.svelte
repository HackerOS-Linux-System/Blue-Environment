<script lang="ts">
  import { run } from 'svelte/legacy';

  /**
   * Blue Environment — TV shell ("10-foot UI").
   *
   * Built for a remote or a game pad: big tiles, an unmistakable focus ring and
   * directional navigation. Arrow keys / D-pad / left stick move the focus
   * between tiles (see lib/altShell/spatialNav.ts); Enter / A activates;
   * Esc, Backspace or B goes back (an app can handle it first by cancelling the
   * `blue:back` event); Home / Start / Guide returns to the home screen;
   * Menu / Y / the context-menu key opens quick settings (volume, Wi-Fi, power).
   *
   * Apps run full screen and reuse the desktop shell's window manager, so every
   * Blue app works; inside an app the D-pad moves between controls.
   */
  import { onDestroy, onMount, tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import { windows, activeWindowId, focusWindow, openApp } from '../lib/stores/windowManager';
  import { APPS } from '../lib/constants';
  import { AppId, type PowerAction } from '../lib/types';
  import { startAltShell } from '../lib/altShell/boot';
  import { listLauncherApps, type RecentItem } from '../lib/altShell/apps';
  import { goHome, dispatchBack } from '../lib/altShell/shellUtils';
  import { moveFocus, type Dir } from '../lib/altShell/spatialNav';
  import { PadTracker, type PadAction } from '../lib/altShell/gamepad';
  import { normalizeTopBar } from '../lib/utils/topBarConfig';
  import { SystemBridge } from '../lib/utils/systemBridge';
  import { t } from '../lib/stores/language';
  import ShellFrame from '../lib/altShell/ShellFrame.svelte';
  import AppViewport from '../lib/altShell/AppViewport.svelte';
  import PowerMenu from '../lib/components/PowerMenu.svelte';
  import TvHome from './TvHome.svelte';
  import TvMenu from './TvMenu.svelte';
  import TvSettings from './TvSettings.svelte';
  import { FocusMemory, pickRestoreIndex } from '../lib/altShell/focusMemory';
  import { remoteAction } from '../lib/altShell/remoteKeys';
  import { initRemoteReceiver } from '../lib/utils/remoteReceiver';
  import { FOCUSABLE_SELECTOR } from '../lib/altShell/spatialNav';

  let wallpaper = $state('');
  let theme = $state('dark');
  let shellThemeId = $state<string | undefined>();
  let systemThemeId = $state<string | null | undefined>();
  let appsEnabled = $state<Record<string, boolean>>({});
  let use24h = $state(true);
  let showTransferProgress = $state(true);

  const stop = startAltShell((cfg) => {
    if (cfg.wallpaper) wallpaper = cfg.wallpaper;
    if (cfg.theme) theme = cfg.theme;
    shellThemeId = cfg.shellThemeId;
    systemThemeId = cfg.systemThemeId;
    if (cfg.appsEnabled) appsEnabled = cfg.appsEnabled;
    use24h = normalizeTopBar(cfg.topBar).clock24h;
    showTransferProgress = cfg.showTransferProgress !== false;
  });
  onDestroy(stop);

  let now = $state(new Date());
  let clock: ReturnType<typeof setInterval>;
  let raf = 0;
  const pad = new PadTracker();

  type Panel = 'none' | 'menu' | 'power';
  let panel = $state<Panel>('none');

  let apps = $derived(listLauncherApps(APPS as any, appsEnabled));
  let running = $derived($windows.filter((w) => !w.isExternal).sort((a, b) => b.zIndex - a.zIndex));
  let appOpen = $derived(running.some((w) => w.id === $activeWindowId && !w.isMinimized));
  let runningItems = $derived(running.map<RecentItem>((w) => ({
    id: w.id, title: w.title, icon: (APPS as any)[w.appId]?.icon, active: w.id === $activeWindowId && !w.isMinimized,
  })));

  let rootEl = $state() as HTMLElement;

  /** The desktop Settings app is swapped for a remote-friendly one in this shell. */
  const overrides = { [AppId.SETTINGS]: TvSettings };

  // ── Focus memory ─────────────────────────────────────────────────────
  // Closing an app lands on the tile that launched it; coming back to an app
  // lands on the control you left. Keyed per screen ("home", "app:<windowId>").
  const memory = new FocusMemory();
  const focusablesIn = (region: HTMLElement | null) =>
    region ? Array.from(region.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter((el) => el.getBoundingClientRect().width > 0) : [];
  const keyOf = (el: HTMLElement) => el.closest<HTMLElement>('[data-tv-key]')?.dataset.tvKey ?? null;

  function rememberFocus(scope: string, region: HTMLElement | null) {
    const active = document.activeElement as HTMLElement | null;
    const list = focusablesIn(region);
    const idx = active ? list.findIndex((el) => el === active || el.contains(active)) : -1;
    if (idx >= 0) memory.remember(scope, keyOf(list[idx]), idx);
  }
  function restoreFocus(scope: string, region: HTMLElement | null) {
    const list = focusablesIn(region);
    const i = pickRestoreIndex(list.map(keyOf), memory.recall(scope));
    if (i >= 0) list[i].focus();
  }
  /** Which screen is showing right now. */
  const currentScope = () => (appOpen ? `app:${$activeWindowId}` : 'home');

  // Remember continuously (cheap: one lookup per focus change).
  function onFocusIn() { if (panel === 'none') rememberFocus(currentScope(), activeRegion()); }

  /** The region directional input currently acts on. */
  function activeRegion(): HTMLElement | null {
    if (!rootEl) return null;
    if (panel === 'menu') return rootEl.querySelector<HTMLElement>('[data-tv-menu]');
    if (panel === 'power') return rootEl.querySelector<HTMLElement>('[data-tv-power]');
    if (appOpen) return rootEl.querySelector<HTMLElement>(`[data-app-viewport="${$activeWindowId}"]`);
    return rootEl.querySelector<HTMLElement>('[data-tv-home]');
  }

  async function restoreFocusOnScreen() {
    await tick();
    if (panel !== 'none') return;
    restoreFocus(currentScope(), activeRegion());
  }
  // Whenever a screen comes (back) into view — home, or an app — put the focus
  // where it was, or on the first control the first time.
  run(() => {
    if (panel === 'none') { void appOpen; void $activeWindowId; restoreFocusOnScreen(); }
  });

  function go(dir: Dir) {
    const region = activeRegion();
    if (!region) return;
    const el = document.activeElement as HTMLInputElement | null;
    // A focused slider: left/right adjust it, up/down leave it.
    if (el?.tagName === 'INPUT' && el.type === 'range' && (dir === 'left' || dir === 'right')) {
      if (dir === 'right') el.stepUp(); else el.stepDown();
      el.dispatchEvent(new Event('input', { bubbles: true }));
      return;
    }
    moveFocus(region, dir);
  }

  function select() { (document.activeElement as HTMLElement | null)?.click?.(); }
  function home() { panel = 'none'; goHome(); }
  function back() {
    if (panel !== 'none') { panel = 'none'; return; }
    if (dispatchBack()) return;
    if (appOpen) goHome();
  }
  function toggleMenu() { panel = panel === 'menu' ? 'none' : 'menu'; }

  function perform(a: PadAction) {
    document.body.classList.add('tv-nav-active');
    switch (a) {
      case 'up': case 'down': case 'left': case 'right': go(a); break;
      case 'select': select(); break;
      case 'back': back(); break;
      case 'home': home(); break;
      case 'menu': toggleMenu(); break;
      default: break;
    }
  }

  // ── keyboard / remote ────────────────────────────────────────────────
  function isEditable(el: EventTarget | null): boolean {
    const e = el as HTMLElement | null;
    if (!e) return false;
    return e.tagName === 'TEXTAREA' || (e.tagName === 'INPUT' && !['range', 'button', 'checkbox', 'radio'].includes((e as HTMLInputElement).type)) || e.isContentEditable;
  }
  function onKey(e: KeyboardEvent) {
    document.body.classList.add('tv-nav-active');
    const remote = remoteAction(e);
    if (remote === 'home') { e.preventDefault(); home(); return; }
    if (remote === 'menu') { e.preventDefault(); toggleMenu(); return; }
    if (remote === 'back') { e.preventDefault(); back(); return; }
    // Media keys: handled by whichever app listens (e.g. a player); the shell
    // just republishes them as one event so apps need no TV-specific key tables.
    if (remote === 'play_pause' || remote === 'next' || remote === 'prev' || remote === 'stop') {
      window.dispatchEvent(new CustomEvent('blue:media-key', { detail: remote }));
      return;
    }
    if (e.key === 'Backspace' && !isEditable(e.target)) { e.preventDefault(); back(); return; }
    // On the home screen and in panels the arrows move between tiles. Inside
    // an app they are left to the app (it may use them itself).
    if (appOpen && panel === 'none') return;
    const dir: Dir | null = e.key === 'ArrowUp' ? 'up' : e.key === 'ArrowDown' ? 'down' : e.key === 'ArrowLeft' ? 'left' : e.key === 'ArrowRight' ? 'right' : null;
    if (dir && !isEditable(e.target)) { e.preventDefault(); go(dir); }
  }

  // ── game pad ─────────────────────────────────────────────────────────
  function poll(ts: number) {
    const pads = navigator.getGamepads ? Array.from(navigator.getGamepads()).filter(Boolean) as Gamepad[] : [];
    const gp = pads[0];
    if (gp) {
      const actions = pad.step(ts, { buttons: gp.buttons.map((b) => b.pressed), axes: Array.from(gp.axes) });
      for (const a of actions) perform(a);
    } else pad.reset();
    raf = requestAnimationFrame(poll);
  }

  // Blue Connect → Remote: a paired phone / controller drives the TV through the
  // same code path as a game pad (see utils/remoteReceiver.ts + BlueConnect/remote.rs).
  function onRemoteAction(e: Event) {
    const d = (e as CustomEvent<{ action: PadAction; handled: boolean }>).detail;
    if (!d) return;
    d.handled = true;
    perform(d.action);
  }
  let stopRemote: (() => void) | undefined;

  onMount(() => {
    window.addEventListener('blue:remote-action', onRemoteAction);
    void initRemoteReceiver().then((un) => (stopRemote = un)).catch(() => {});
    clock = setInterval(() => (now = new Date()), 10000);
    raf = requestAnimationFrame(poll);
    const showPointer = () => document.body.classList.remove('tv-nav-active');
    window.addEventListener('mousemove', showPointer);
    return () => window.removeEventListener('mousemove', showPointer);
  });
  onDestroy(() => { window.removeEventListener('blue:remote-action', onRemoteAction); stopRemote?.(); clearInterval(clock); cancelAnimationFrame(raf); document.body.classList.remove('tv-nav-active'); });

  function launch(id: string) { panel = 'none'; openApp(id); }
  function resume(id: string) { panel = 'none'; focusWindow(id); }
  function onPower(e: CustomEvent<PowerAction>) { panel = 'none'; SystemBridge.powerAction(e.detail); }
  function openSettings() { panel = 'none'; openApp(AppId.SETTINGS); }
</script>

<svelte:window onkeydown={onKey} onfocusin={onFocusIn} />

<ShellFrame variant="tv" {wallpaper} {shellThemeId} {systemThemeId} {theme} keyboardOnFocus={true} {showTransferProgress}>
  <div class="absolute inset-0" bind:this={rootEl}>
    <div class="absolute inset-0 bg-black/35 pointer-events-none"></div>
    {#if !appOpen}
      <TvHome {apps} running={runningItems} {now} {use24h} on:launch={(e) => launch(e.detail)} on:resume={(e) => resume(e.detail)} />
    {/if}
    <AppViewport {overrides} />

    {#if !appOpen && panel === 'none'}
      <div class="absolute bottom-0 inset-x-0 z-20 px-12 py-4 flex gap-8 text-sm text-white/80 bg-gradient-to-t from-black/60 to-transparent pointer-events-none" transition:fade={{ duration: 120 }}>
        <span><kbd class="px-2 py-0.5 rounded bg-white/20">A</kbd> {$t('altui.tv_select')}</span>
        <span><kbd class="px-2 py-0.5 rounded bg-white/20">B</kbd> {$t('altui.tv_back')}</span>
        <span><kbd class="px-2 py-0.5 rounded bg-white/20">Y</kbd> {$t('altui.tv_menu')}</span>
        <span><kbd class="px-2 py-0.5 rounded bg-white/20">Start</kbd> {$t('altui.tv_home')}</span>
      </div>
    {/if}

    {#if panel === 'menu'}
      <TvMenu on:close={() => (panel = 'none')} on:settings={openSettings} on:power={() => (panel = 'power')} />
    {/if}
    {#if panel === 'power'}
      <div class="absolute inset-0 z-50" data-tv-power transition:fade={{ duration: 120 }}>
        <PowerMenu zIndex={60} on:action={onPower} on:close={() => (panel = 'none')} />
      </div>
    {/if}
  </div>
</ShellFrame>
