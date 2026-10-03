<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { AppId } from '../types';
  import { APPS } from '../constants';
  import {
    Search, Wifi, Bell, Command, CloudSun, Cloud, CloudRain, CloudSnow, Sun, Clipboard,
    Droplets, Wind, Gauge, ArrowDown, ArrowUp, Clock, Globe2, Copy, X, Languages,
    Battery, BatteryLow, BatteryMedium, BatteryFull, BatteryCharging, BatteryWarning,
    Zap, Check,
  } from 'lucide-svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { CompositorBridge } from '../utils/compositorBridge';
  import { configStore } from '../utils/configStore';
  import { ICON_COMPONENTS } from '../utils/fileTypeAssociations';
  import { t } from '../stores/language';
  import { createEventDispatcher } from 'svelte';
  import { showContextMenu, type MenuItem } from '../stores/contextMenu';
  import { closeWindow } from '../stores/windowManager';

  export let openWindows: { id: string; appId?: AppId; isMinimized: boolean; isActive: boolean; workspace: number }[] = [];
  export let currentWorkspace = 0;
  export let workspaceCount = 4;
  export let isStartMenuOpen = false;
  export let isClipboardOpen = false;
  export let enabled = true;
  export let position: 'top' | 'bottom' = 'top';
  /** Active shell theme id (see builtinThemes.ts / App.svelte's
   * `activeShellTheme`) — `undefined`/anything other than `'hydra'`
   * today just means "normal panel look", same as before this prop
   * existed. Not a general theming hook; add more `{#if shellThemeId
   * === '...'}` branches here if/when another theme needs its own
   * panel treatment, rather than trying to generalize prematurely for
   * a single data point. */
  export let shellThemeId: string | undefined = undefined;

  const dispatch = createEventDispatcher<{
    openApp: string;
    toggleWindow: string;
    startClick: void;
    startDoubleClick: void;
    toggleControlCenter: void;
    toggleNotifications: void;
    switchWorkspace: number;
    toggleClipboard: void;
  }>();

  // --- Right-click menus ----------------------------------------------------
  // Children call stopPropagation so the panel-level menu doesn't replace theirs.
  function savePinned(next: AppId[]) { pinnedApps = next; configStore.save({ pinnedApps: next }); }

  function movePinned(index: number, delta: number) {
    const target = index + delta;
    if (index < 0 || target < 0 || target >= pinnedApps.length) return;
    const next = pinnedApps.slice();
    const moved = next.splice(index, 1)[0];
    next.splice(target, 0, moved);
    savePinned(next);
  }

  function pinnedAppMenu(e: MouseEvent, appId: AppId) {
    e.stopPropagation();
    const app = APPS[appId];
    const insts = openWindows.filter((w) => w.appId === appId);
    const i = pinnedApps.indexOf(appId);
    const items: MenuItem[] = [
      { label: insts.length ? `Open new window` : `Open ${app?.title ?? ''}`, action: () => dispatch('openApp', appId) },
      ...(insts.length ? [{ label: insts.length > 1 ? `Close all windows (${insts.length})` : 'Close window', danger: true, action: () => insts.forEach((w) => closeWindow(w.id)) } as MenuItem] : []),
      { separator: true },
      { label: 'Move left', disabled: i <= 0, action: () => movePinned(i, -1) },
      { label: 'Move right', disabled: i < 0 || i >= pinnedApps.length - 1, action: () => movePinned(i, 1) },
      { label: 'Unpin from panel', danger: true, disabled: pinnedApps.length <= 1, action: () => savePinned(pinnedApps.filter((x) => x !== appId)) },
    ];
    showContextMenu(e, items);
  }

  function startButtonMenu(e: MouseEvent) {
    e.stopPropagation();
    showContextMenu(e, [
      { label: 'Applications', action: () => dispatch('startClick') },
      { label: 'Applications (full screen)', action: () => dispatch('startDoubleClick') },
      { separator: true },
      { label: 'Terminal', action: () => dispatch('openApp', AppId.TERMINAL) },
      { label: 'Files', action: () => dispatch('openApp', AppId.EXPLORER) },
      { label: 'System Monitor', action: () => dispatch('openApp', AppId.SYSTEM_MONITOR) },
      { label: 'Settings', action: () => dispatch('openApp', AppId.SETTINGS) },
    ]);
  }

  function panelMenu(e: MouseEvent) {
    showContextMenu(e, [
      { label: 'Panel settings…', action: () => dispatch('openApp', AppId.SETTINGS) },
      { separator: true },
      { label: 'Notifications', action: () => dispatch('toggleNotifications') },
      { label: 'Control Center', action: () => dispatch('toggleControlCenter') },
      { label: 'Clipboard history', action: () => dispatch('toggleClipboard') },
      { separator: true },
      ...Array.from({ length: workspaceCount }, (_, w) => ({ label: `Workspace ${w + 1}`, checked: w === currentWorkspace, action: () => dispatch('switchWorkspace', w) } as MenuItem)),
    ]);
  }

  function clockMenu(e: MouseEvent) {
    e.stopPropagation();
    showContextMenu(e, [
      { label: 'Open Calendar', action: () => dispatch('openApp', AppId.BLUE_CALENDAR) },
      { label: 'Copy date and time', action: () => navigator.clipboard.writeText(new Date().toLocaleString()).catch(() => {}) },
      { separator: true },
      { label: 'Date & time settings…', action: () => dispatch('openApp', AppId.SETTINGS) },
    ]);
  }

  // --- IME candidate window indicator --------------------------------------
  // See CompositorBridge.onImeCandidateWindow / protocols/input_method.rs on
  // the compositor side. Purely a status dot — the candidate list itself is
  // always the IME's own composited surface, never drawn here.
  let imeActive = false;
  // `onImeCandidateWindow` resolves via Tauri's async event API, so the
  // real unlisten fn only exists once that promise settles — same pattern
  // as MonitorsSection.svelte's onHdrStateChanged.
  let unsubImePromise: Promise<() => void> | undefined;

  // --- Weather ------------------------------------------------------------
  interface WeatherData {
    temp: string; tempRaw: number; feelsLike: string; code: number; city: string;
    humidity: number | null; windKph: number | null; high: string; low: string;
  }

  function weatherIconFor(code: number) {
    if (code === 0) return { Icon: Sun, cls: 'text-yellow-300' };
    if (code <= 3) return { Icon: CloudSun, cls: 'text-yellow-200' };
    if (code <= 67) return { Icon: CloudRain, cls: 'text-blue-300' };
    if (code <= 77) return { Icon: CloudSnow, cls: 'text-blue-100' };
    return { Icon: Cloud, cls: 'text-slate-300' };
  }

  function weatherLabelFor(code: number): string {
    if (code === 0) return 'Clear sky';
    if (code <= 2) return 'Partly cloudy';
    if (code === 3) return 'Overcast';
    if (code <= 48) return 'Fog';
    if (code <= 57) return 'Drizzle';
    if (code <= 67) return 'Rain';
    if (code <= 77) return 'Snow';
    if (code <= 82) return 'Rain showers';
    if (code <= 86) return 'Snow showers';
    if (code <= 99) return 'Thunderstorm';
    return 'Unknown';
  }

  function fmtTemp(celsius: number, unit: 'celsius' | 'fahrenheit'): string {
    const v = unit === 'fahrenheit' ? celsius * 9 / 5 + 32 : celsius;
    return `${Math.round(v)}°${unit === 'fahrenheit' ? 'F' : 'C'}`;
  }

  let weatherLastError: string | null = null;

  async function fetchWeather(): Promise<WeatherData | null> {
    try {
      const r = await SystemBridge.getWeather(weatherCityOverride || undefined);
      weatherLastError = null;
      return {
        temp: fmtTemp(r.tempC, weatherUnit),
        tempRaw: r.tempC,
        feelsLike: fmtTemp(r.feelsLikeC, weatherUnit),
        code: r.code,
        city: r.city,
        humidity: r.humidity,
        windKph: r.windKph,
        high: typeof r.highC === 'number' ? fmtTemp(r.highC, weatherUnit) : '—',
        low: typeof r.lowC === 'number' ? fmtTemp(r.lowC, weatherUnit) : '—',
      };
    } catch (e) {
      // Surface the failure (dev console) instead of failing silently like
      // the old direct-fetch implementation did — makes it obvious *why*
      // the widget isn't showing (e.g. no network, all geolocation
      // providers down) instead of it just quietly never appearing.
      weatherLastError = e instanceof Error ? e.message : String(e);
      console.warn('[weather] fetch failed:', weatherLastError);
      return null;
    }
  }

  let time = new Date();
  let weather: WeatherData | null = null;
  let weatherEnabled = true;
  let weatherCityOverride = '';
  let weatherUnit: 'celsius' | 'fahrenheit' = 'celsius';
  let showWeatherPopover = false;

  // --- Clipboard hover preview ---------------------------------------------
  let hasClipboardContent = false;

  // --- Battery ------------------------------------------------------------
  // Shown right next to the weather chip. Hidden entirely on machines with
  // no battery (desktops) — `present` comes from /sys/class/power_supply
  // (see `get_battery_status` in system_stats.rs), not a fake "100 %".
  interface BatteryState { present: boolean; percentage: number; charging: boolean; status: string; }
  let battery: BatteryState | null = null;
  let batteryTimer: ReturnType<typeof setInterval>;
  async function loadBattery() {
    try { battery = await SystemBridge.getBatteryStatus(); } catch { /* keep last value */ }
  }
  function batteryIconFor(b: BatteryState) {
    if (b.charging) return BatteryCharging;
    if (b.percentage <= 10) return BatteryWarning;
    if (b.percentage <= 25) return BatteryLow;
    if (b.percentage <= 60) return BatteryMedium;
    return BatteryFull;
  }
  function batteryColor(b: BatteryState): string {
    if (b.charging) return 'text-green-400';
    if (b.percentage <= 15) return 'text-red-400';
    if (b.percentage <= 30) return 'text-amber-400';
    return 'text-slate-300';
  }

  // --- Power mode (power-profiles-daemon) ---------------------------------
  // Click the battery chip → pick Power Saver / Balanced / Performance.
  // `get_power_profiles` returns [] when power-profiles-daemon isn't there,
  // in which case the switcher simply isn't offered (see power.rs).
  interface PowerProfileItem { name: string; active: boolean; icon?: string; description: string; }
  let powerProfiles: PowerProfileItem[] = [];
  let showPowerPopover = false;
  let powerBusy = false;
  let powerError = '';
  let powerTimer: ReturnType<typeof setInterval>;
  const POWER_KEYS: Record<string, { name: string; desc: string }> = {
    'power-saver': { name: 'settings.power.profile.power_saver.name', desc: 'settings.power.profile.power_saver.desc' },
    'balanced': { name: 'settings.power.profile.balanced.name', desc: 'settings.power.profile.balanced.desc' },
    'performance': { name: 'settings.power.profile.performance.name', desc: 'settings.power.profile.performance.desc' },
  };
  $: activePowerProfile = powerProfiles.find((p) => p.active) ?? null;
  function powerIconFor(name: string) { return name === 'performance' ? Zap : name === 'power-saver' ? Battery : Wind; }
  function powerColorFor(name: string) { return name === 'performance' ? 'text-amber-300' : name === 'power-saver' ? 'text-green-400' : 'text-blue-300'; }
  function powerName(p: PowerProfileItem): string { const k = POWER_KEYS[p.name]; return k ? $t(k.name) : p.name; }
  function powerDesc(p: PowerProfileItem): string { const k = POWER_KEYS[p.name]; return k ? $t(k.desc) : p.description; }
  async function loadPowerProfiles() {
    try { powerProfiles = (await SystemBridge.getPowerProfiles()) ?? []; } catch { /* keep last value */ }
  }
  async function togglePowerPopover() {
    showPowerPopover = !showPowerPopover;
    powerError = '';
    if (showPowerPopover) await loadPowerProfiles(); // the mode may have been changed elsewhere
  }
  async function choosePowerProfile(p: PowerProfileItem) {
    if (powerBusy || p.active) { showPowerPopover = false; return; }
    powerBusy = true;
    powerError = '';
    const previous = powerProfiles;
    powerProfiles = powerProfiles.map((x) => ({ ...x, active: x.name === p.name })); // optimistic
    try {
      await SystemBridge.setPowerProfile(p.name);
      await loadPowerProfiles(); // trust what the daemon says, not what we hoped for
      showPowerPopover = false;
    } catch (e) {
      powerProfiles = previous;
      powerError = e instanceof Error ? e.message : String(e);
    } finally {
      powerBusy = false;
    }
  }

  let clipboardHoverPreviewEnabled = true;
  let showClipboardPreview = false;
  let latestClipboardItem: { id: string; content: string; timestamp: number } | null = null;
  let clipboardHoverTimer: ReturnType<typeof setTimeout>;

  async function loadLatestClipboardItem() {
    try {
      const hist = await SystemBridge.getClipboardHistory();
      latestClipboardItem = hist?.[0] ?? null;
    } catch { latestClipboardItem = null; }
  }

  function onClipboardEnter() {
    if (!clipboardHoverPreviewEnabled) return;
    clearTimeout(clipboardHoverTimer);
    clipboardHoverTimer = setTimeout(async () => {
      await loadLatestClipboardItem();
      showClipboardPreview = true;
    }, 300);
  }
  function onClipboardLeave() {
    clearTimeout(clipboardHoverTimer);
    showClipboardPreview = false;
  }

  // --- Network speed + timezone popover ------------------------------------
  let networkHoverInfoEnabled = true;
  let showClockPopover = false;
  let netRxBps = 0;
  let netTxBps = 0;
  let netConnected = false;
  let timezoneName = '';
  let timezoneOffset = '';
  let clockHoverTimer: ReturnType<typeof setTimeout>;

  function fmtBps(bps: number): string {
    if (bps >= 1024 * 1024) return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
    if (bps >= 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
    return `${bps} B/s`;
  }

  async function loadNetworkInfo() {
    try {
      const ifaces = await SystemBridge.invokeCommand<any[]>('get_network_metrics');
      const active = (ifaces ?? []).find((i) => i.connected) ?? ifaces?.[0];
      netRxBps = active?.rx_bps ?? 0;
      netTxBps = active?.tx_bps ?? 0;
      netConnected = !!active?.connected;
    } catch { netRxBps = 0; netTxBps = 0; netConnected = false; }
  }

  function computeTimezone() {
    try {
      timezoneName = Intl.DateTimeFormat().resolvedOptions().timeZone;
      const offsetMin = -new Date().getTimezoneOffset();
      const sign = offsetMin >= 0 ? '+' : '-';
      const abs = Math.abs(offsetMin);
      timezoneOffset = `UTC${sign}${String(Math.floor(abs / 60)).padStart(2, '0')}:${String(abs % 60).padStart(2, '0')}`;
    } catch { timezoneName = ''; timezoneOffset = ''; }
  }

  function onClockEnter() {
    if (!networkHoverInfoEnabled) return;
    clearTimeout(clockHoverTimer);
    clockHoverTimer = setTimeout(() => {
      computeTimezone();
      loadNetworkInfo();
      showClockPopover = true;
    }, 300);
  }
  function onClockLeave() {
    clearTimeout(clockHoverTimer);
    showClockPopover = false;
  }

  let pinnedApps: AppId[] = [AppId.TERMINAL, AppId.EXPLORER, AppId.SYSTEM_MONITOR, AppId.SETTINGS];
  let panelOpacity = 0.95;
  let panelHeight = 48;
  // Settings > Panel > "App Launcher" — see systemBridge.ts's
  // UserConfig.startButtonLabelMode/startButtonIcon doc comments.
  let startButtonLabelMode: 'icon-and-label' | 'icon-only' = 'icon-and-label';
  let startButtonIcon = '';
  $: StartButtonIconComponent = startButtonIcon ? ICON_COMPONENTS[startButtonIcon] : null;

  let clockTimer: ReturnType<typeof setInterval>;
  let weatherTimer: ReturnType<typeof setInterval>;
  let clipboardTimer: ReturnType<typeof setInterval>;
  let unsubConfig: () => void;

  function loadWeather() {
    if (!weatherEnabled) { weather = null; return; }
    fetchWeather().then((w) => { if (w) weather = w; });
  }

  onMount(() => {
    clockTimer = setInterval(() => (time = new Date()), 1000);
    computeTimezone();

    loadWeather();
    weatherTimer = setInterval(loadWeather, 30 * 60 * 1000);

    const checkClipboard = async () => {
      try { hasClipboardContent = await SystemBridge.hasText(); } catch {}
    };
    checkClipboard();
    clipboardTimer = setInterval(checkClipboard, 4000);

    loadBattery();
    batteryTimer = setInterval(loadBattery, 30_000);
    loadPowerProfiles();
    powerTimer = setInterval(loadPowerProfiles, 30_000);

    unsubConfig = configStore.subscribe((cfg) => {
      const pinned = cfg.pinnedApps as AppId[] | undefined;
      if (pinned && Array.isArray(pinned) && pinned.length > 0) pinnedApps = pinned;
      if (typeof cfg.panelOpacity === 'number') panelOpacity = cfg.panelOpacity;
      if (typeof cfg.panelSize === 'number' && cfg.panelSize > 0) panelHeight = cfg.panelSize;
      startButtonLabelMode = cfg.startButtonLabelMode === 'icon-only' ? 'icon-only' : 'icon-and-label';
      startButtonIcon = cfg.startButtonIcon ?? '';

      const prevEnabled = weatherEnabled;
      const prevCity = weatherCityOverride;
      const prevUnit = weatherUnit;
      weatherEnabled = cfg.weatherEnabled ?? true;
      weatherCityOverride = cfg.weatherCity ?? '';
      weatherUnit = cfg.weatherUnit ?? 'celsius';
      clipboardHoverPreviewEnabled = cfg.clipboardHoverPreviewEnabled ?? true;
      networkHoverInfoEnabled = cfg.networkHoverInfoEnabled ?? true;

      if (!weatherEnabled) { weather = null; }
      else if (prevEnabled !== weatherEnabled || prevCity !== weatherCityOverride || prevUnit !== weatherUnit) {
        loadWeather();
      }
    });

    unsubImePromise = CompositorBridge.onImeCandidateWindow((visible) => {
      imeActive = visible;
    });
  });

  onDestroy(() => {
    clearInterval(clockTimer);
    clearInterval(weatherTimer);
    clearInterval(clipboardTimer);
    clearInterval(batteryTimer);
    clearInterval(powerTimer);
    clearTimeout(clipboardHoverTimer);
    clearTimeout(clockHoverTimer);
    unsubConfig?.();
    unsubImePromise?.then((fn) => fn());
  });

  function handleStartClick(e: MouseEvent) {
    if (e.detail === 2) dispatch('startDoubleClick');
    else dispatch('startClick');
  }

  async function copyLatestClipboardItem() {
    if (!latestClipboardItem) return;
    await SystemBridge.copyText(latestClipboardItem.content);
    showClipboardPreview = false;
  }
</script>

{#if enabled}
<div
  class="absolute left-0 right-0 backdrop-blur-sm flex items-center justify-between px-3 select-none {position === 'top' ? 'top-0 border-b' : 'bottom-0 border-t'} {shellThemeId === 'hydra' ? 'border-pink-500/20' : 'border-white/5'}"
  on:contextmenu={panelMenu} role="presentation"
  style="height:{panelHeight}px; z-index:50; {shellThemeId === 'hydra'
    ? `background:linear-gradient(90deg, rgba(236,72,153,${panelOpacity * 0.5}), rgba(139,92,246,${panelOpacity * 0.5}), rgba(59,130,246,${panelOpacity * 0.5})); box-shadow:0 0 24px rgba(236,72,153,0.25);`
    : `background-color:rgba(15, 23, 42, var(--panel-opacity, ${panelOpacity}));`}"
>
  <!-- Left: Start + search -->
  <div class="flex items-center gap-3 w-1/3">
    <button
      on:contextmenu={startButtonMenu}
      on:click={handleStartClick}
      class="panel-icon-btn flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg transition-all group {isStartMenuOpen ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/5 text-slate-300 hover:text-white'}"
      title="Start (double-click for full screen)"
    >
      <div class="relative">
        {#if StartButtonIconComponent}
          <svelte:component this={StartButtonIconComponent} size={18} class="group-hover:rotate-12 transition-transform duration-200" />
        {:else}
          <Command size={18} class="group-hover:rotate-12 transition-transform duration-200" />
        {/if}
        <div class="absolute -top-1 -right-1 w-1.5 h-1.5 theme-accent-gradient rounded-full" />
      </div>
      {#if startButtonLabelMode !== 'icon-only'}
        <span class="font-bold text-sm tracking-tight hidden sm:block">Blue</span>
      {/if}
    </button>
    <div
      class="hidden md:flex items-center gap-2 bg-slate-800/80 hover:bg-slate-700/80 border border-white/5 rounded-full px-3 py-1 text-xs text-slate-400 cursor-text transition-colors w-44"
      on:click={() => dispatch('startClick')} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => dispatch('startClick'))(); } }}
    >
      <Search size={12} />
      <span>Search apps...</span>
    </div>
  </div>

  <!-- Center: pinned apps -->
  <div class="flex items-center justify-center w-1/3">
    <div class="flex items-center gap-1 bg-slate-800/60 border border-white/5 rounded-2xl px-2 py-1 shadow-lg">
      {#each pinnedApps as appId (appId)}
        {@const app = APPS[appId]}
        {#if app}
          {@const openInsts = openWindows.filter((w) => w.appId === appId)}
          {@const isOpen = openInsts.length > 0}
          {@const isActive = openInsts.some((w) => w.isActive && !w.isMinimized)}
          <button
            on:contextmenu={(e) => pinnedAppMenu(e, appId)}
            on:click={() => {
              const inst = openWindows.find((w) => w.appId === appId);
              if (inst) dispatch('toggleWindow', inst.id);
              else dispatch('openApp', appId);
            }}
            class="relative group p-2 rounded-xl transition-all hover:bg-white/10"
            title={app.title}
          >
            {#if typeof app.icon !== 'string'}
              <svelte:component this={app.icon} size={20}
                class="transition-colors duration-200 {isOpen ? 'text-blue-400' : 'text-slate-400 group-hover:text-slate-200'}" />
            {/if}
            {#if isOpen}
              <span class="absolute -bottom-0.5 left-1/2 -translate-x-1/2 h-0.5 rounded-full transition-all {isActive ? 'w-3.5 bg-blue-400' : 'w-1 bg-slate-500'}" />
            {/if}
          </button>
        {/if}
      {/each}
    </div>
  </div>

  <!-- Right -->
  <div class="flex items-center justify-end gap-2 w-1/3">
    <div class="hidden lg:flex items-center gap-1 px-2 py-1 rounded-full hover:bg-white/5 transition-colors">
      {#each Array.from({ length: workspaceCount }, (_, i) => i) as i (i)}
        {@const hasWins = openWindows.some((w) => w.workspace === i && !w.isMinimized)}
        <button on:click={() => dispatch('switchWorkspace', i)} title="Workspace {i + 1}"
          class="transition-all duration-200 rounded-full {i === currentWorkspace ? 'w-4 h-2 bg-blue-400' : `w-2 h-2 ${hasWins ? 'bg-slate-400' : 'bg-slate-600'} hover:bg-slate-300`}" />
      {/each}
    </div>

    {#if imeActive}
      <div class="hidden lg:flex items-center gap-1 px-2 py-1 rounded-full bg-blue-500/20 text-blue-300"
           title={$t('ime.active')}>
        <Languages size={13} />
      </div>
    {/if}

    {#if weather}
      {@const wi = weatherIconFor(weather.code)}
      <div class="hidden lg:block relative">
        <button
          on:click={() => (showWeatherPopover = !showWeatherPopover)}
          class="panel-icon-btn flex items-center gap-1.5 px-2 py-1 rounded-full hover:bg-white/5 transition-colors {showWeatherPopover ? 'bg-white/10' : ''}"
          title="{weather.city}: {weather.temp} — click for details"
        >
          <svelte:component this={wi.Icon} size={14} class={wi.cls} />
          <span class="text-xs font-medium text-slate-200">{weather.temp}</span>
        </button>

        {#if showWeatherPopover}
          <div class="fixed inset-0 z-40" on:click={() => (showWeatherPopover = false)} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => (showWeatherPopover = false))(); } }} />
          <div class="absolute right-0 {position === 'top' ? 'top-full mt-2' : 'bottom-full mb-2'} w-64 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl p-4 z-50">
            <div class="flex items-center justify-between mb-3">
              <div>
                <div class="text-sm font-semibold text-white">{weather.city}</div>
                <div class="text-[11px] text-slate-500">{weatherLabelFor(weather.code)}</div>
              </div>
              <svelte:component this={wi.Icon} size={30} class={wi.cls} />
            </div>
            <div class="text-3xl font-light text-white mb-3 tabular-nums">{weather.temp}</div>
            <div class="grid grid-cols-2 gap-2 text-xs">
              <div class="flex items-center gap-1.5 text-slate-400"><Gauge size={12} /> Feels like <span class="ml-auto text-slate-200">{weather.feelsLike}</span></div>
              <div class="flex items-center gap-1.5 text-slate-400"><Wind size={12} /> Wind <span class="ml-auto text-slate-200">{weather.windKph ?? '—'} km/h</span></div>
              <div class="flex items-center gap-1.5 text-slate-400"><Droplets size={12} /> Humidity <span class="ml-auto text-slate-200">{weather.humidity ?? '—'}%</span></div>
              <div class="flex items-center gap-1.5 text-slate-400">H/L <span class="ml-auto text-slate-200">{weather.high} / {weather.low}</span></div>
            </div>
            <button on:click={() => { dispatch('openApp', AppId.SETTINGS); showWeatherPopover = false; }}
              class="mt-3 w-full text-center text-[11px] text-blue-400 hover:text-blue-300 py-1.5 rounded-lg hover:bg-white/5 transition-colors">
              Weather settings
            </button>
          </div>
        {/if}
      </div>
    {/if}

    {#if battery?.present || powerProfiles.length > 0}
      <div class="relative">
        <button
          on:click={togglePowerPopover}
          disabled={powerProfiles.length === 0}
          class="panel-icon-btn flex items-center gap-1.5 px-2 py-1 rounded-full transition-colors {powerProfiles.length ? 'hover:bg-white/5 cursor-pointer' : 'cursor-default'} {showPowerPopover ? 'bg-white/10' : ''}"
          title="{battery?.present ? `Battery: ${Math.round(battery.percentage)}% — ${battery.status}` : ''}{battery?.present && activePowerProfile ? ' · ' : ''}{activePowerProfile ? `${$t('panel.power_mode')}: ${powerName(activePowerProfile)}` : ''}"
          aria-haspopup="menu" aria-expanded={showPowerPopover}
        >
          {#if battery?.present}
            <svelte:component this={batteryIconFor(battery)} size={15} class={batteryColor(battery)} />
            <span class="text-xs font-medium tabular-nums {battery.percentage <= 15 && !battery.charging ? 'text-red-400' : 'text-slate-200'}">{Math.round(battery.percentage)}%</span>
          {/if}
          {#if activePowerProfile}
            <svelte:component this={powerIconFor(activePowerProfile.name)} size={13} class={powerColorFor(activePowerProfile.name)} />
          {/if}
        </button>

        {#if showPowerPopover}
          <div class="fixed inset-0 z-40" on:click={() => (showPowerPopover = false)} role="button" tabindex="-1" on:keydown={(e) => { if (e.key === 'Escape') showPowerPopover = false; }} />
          <div class="absolute right-0 {position === 'top' ? 'top-full mt-2' : 'bottom-full mb-2'} w-64 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl p-2 z-50" role="menu">
            <div class="px-2 pt-1 pb-1.5 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">{$t('panel.power_mode')}</div>
            {#each powerProfiles as p (p.name)}
              <button
                on:click={() => choosePowerProfile(p)} disabled={powerBusy} role="menuitemradio" aria-checked={p.active}
                class="w-full flex items-center gap-3 px-2 py-2 rounded-xl text-left transition-colors disabled:opacity-60 {p.active ? 'bg-blue-600/20' : 'hover:bg-white/5'}"
              >
                <svelte:component this={powerIconFor(p.name)} size={18} class={powerColorFor(p.name)} />
                <div class="min-w-0 flex-1">
                  <div class="text-xs font-medium text-white">{powerName(p)}</div>
                  <div class="text-[10px] text-slate-400 leading-snug">{powerDesc(p)}</div>
                </div>
                {#if p.active}<Check size={15} class="text-blue-400 shrink-0" />{/if}
              </button>
            {/each}
            {#if powerError}
              <div class="mt-1 px-2 py-1.5 text-[10px] text-red-300 bg-red-500/10 rounded-lg break-words">{powerError}</div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <div class="relative" on:mouseenter={onClipboardEnter} on:mouseleave={onClipboardLeave}>
      <button on:click={() => dispatch('toggleClipboard')}
        class="panel-icon-btn relative p-2 rounded-full transition-colors group {isClipboardOpen ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-300'}"
        title="Clipboard history">
        <Clipboard size={15} class="group-hover:text-white" />
        {#if hasClipboardContent}
          <span class="absolute top-1.5 right-1.5 w-1.5 h-1.5 bg-blue-500 rounded-full" />
        {/if}
      </button>

      {#if showClipboardPreview && clipboardHoverPreviewEnabled}
        <div class="absolute right-0 {position === 'top' ? 'top-full mt-2' : 'bottom-full mb-2'} w-64 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl p-3 z-50">
          <div class="text-[10px] font-semibold text-slate-500 uppercase tracking-wider mb-1.5">Latest clipboard item</div>
          {#if latestClipboardItem}
            <div class="text-xs text-slate-200 break-words line-clamp-4 bg-slate-800/60 rounded-lg p-2 mb-2">{latestClipboardItem.content}</div>
            <button on:click={copyLatestClipboardItem} class="w-full flex items-center justify-center gap-1.5 text-[11px] text-blue-400 hover:text-blue-300 py-1.5 rounded-lg hover:bg-white/5 transition-colors">
              <Copy size={11} /> Copy again
            </button>
          {:else}
            <div class="text-xs text-slate-500 py-2 text-center">Clipboard is empty</div>
          {/if}
        </div>
      {/if}
    </div>

    <div class="relative" on:mouseenter={onClockEnter} on:mouseleave={onClockLeave}>
      <button on:click={() => dispatch('toggleControlCenter')}
        class="panel-icon-btn flex items-center gap-2 px-3 py-1.5 rounded-full hover:bg-white/10 transition-colors border border-transparent hover:border-white/5">
        <Wifi size={13} class="text-slate-300" />
        <span class="text-xs font-medium text-slate-200 tabular-nums">
          {time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </span>
      </button>

      {#if showClockPopover && networkHoverInfoEnabled}
        <div class="absolute right-0 {position === 'top' ? 'top-full mt-2' : 'bottom-full mb-2'} w-56 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl p-3 z-50 text-xs">
          <div class="flex items-center gap-2 mb-2 text-slate-300">
            <Globe2 size={13} class="text-blue-400 shrink-0" />
            <div class="min-w-0">
              <div class="truncate">{timezoneName || 'Unknown timezone'}</div>
              <div class="text-[10px] text-slate-500">{timezoneOffset}</div>
            </div>
          </div>
          <div class="h-px bg-white/10 my-2" />
          <div class="flex items-center gap-2 text-slate-300 mb-1">
            <ArrowDown size={12} class="text-green-400 shrink-0" />
            <span class="flex-1">Download</span>
            <span class="tabular-nums text-slate-200">{fmtBps(netRxBps)}</span>
          </div>
          <div class="flex items-center gap-2 text-slate-300">
            <ArrowUp size={12} class="text-orange-400 shrink-0" />
            <span class="flex-1">Upload</span>
            <span class="tabular-nums text-slate-200">{fmtBps(netTxBps)}</span>
          </div>
          {#if !netConnected}
            <div class="text-[10px] text-slate-500 mt-2">No active connection detected</div>
          {/if}
        </div>
      {/if}
    </div>

    <button on:click={() => dispatch('toggleNotifications')} class="relative p-2 rounded-full hover:bg-white/10 transition-colors group">
      <Bell size={15} class="text-slate-300 group-hover:text-white" />
      <span class="absolute top-1.5 right-1.5 w-1.5 h-1.5 bg-red-500 border border-slate-900 rounded-full" />
    </button>
  </div>
</div>
{/if}
