<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { AppId } from '../types';
  import { APPS } from '../constants';
  import {
    Search, Wifi, Bell, Command, CloudSun, Cloud, CloudRain, CloudSnow, Sun, Clipboard,
    Droplets, Wind, Gauge, ArrowDown, ArrowUp, Clock, Globe2, Copy, X,
  } from 'lucide-svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { configStore } from '../utils/configStore';
  import { createEventDispatcher } from 'svelte';

  export let openWindows: { id: string; appId: AppId; isMinimized: boolean; isActive: boolean; workspace: number }[] = [];
  export let currentWorkspace = 0;
  export let workspaceCount = 4;
  export let isStartMenuOpen = false;
  export let isClipboardOpen = false;

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

  async function fetchWeather(): Promise<WeatherData | null> {
    try {
      let latitude: number | undefined;
      let longitude: number | undefined;
      let city = 'Unknown';

      if (weatherCityOverride) {
        // Manual city override — geocode it first.
        const geoRes = await fetch(
          `https://geocoding-api.open-meteo.com/v1/search?count=1&name=${encodeURIComponent(weatherCityOverride)}`,
          { signal: AbortSignal.timeout(4000) },
        );
        const geo = await geoRes.json();
        const hit = geo?.results?.[0];
        if (!hit) return null;
        latitude = hit.latitude; longitude = hit.longitude; city = hit.name;
      } else {
        const geoRes = await fetch('https://ipapi.co/json/', { signal: AbortSignal.timeout(3000) });
        const geo = await geoRes.json();
        latitude = geo.latitude; longitude = geo.longitude; city = geo.city ?? 'Unknown';
      }
      if (!latitude || !longitude) return null;

      const url = `https://api.open-meteo.com/v1/forecast?latitude=${latitude}&longitude=${longitude}` +
        `&current_weather=true&hourly=relativehumidity_2m&daily=temperature_2m_max,temperature_2m_min&timezone=auto`;
      const wxRes = await fetch(url, { signal: AbortSignal.timeout(4000) });
      const wx = await wxRes.json();
      if (!wx.current_weather) return null;

      const nowHour = new Date().getHours();
      const humidity: number | null = Array.isArray(wx?.hourly?.relativehumidity_2m)
        ? wx.hourly.relativehumidity_2m[nowHour] ?? null : null;
      const high = wx?.daily?.temperature_2m_max?.[0];
      const low = wx?.daily?.temperature_2m_min?.[0];

      const tempRaw = wx.current_weather.temperature;
      return {
        temp: fmtTemp(tempRaw, weatherUnit),
        tempRaw,
        feelsLike: fmtTemp(tempRaw, weatherUnit), // open-meteo current_weather has no apparent temp on the free tier
        code: wx.current_weather.weathercode,
        city,
        humidity,
        windKph: wx.current_weather.windspeed ?? null,
        high: typeof high === 'number' ? fmtTemp(high, weatherUnit) : '—',
        low: typeof low === 'number' ? fmtTemp(low, weatherUnit) : '—',
      };
    } catch {
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

    unsubConfig = configStore.subscribe((cfg) => {
      const pinned = (cfg as any).pinnedApps as AppId[] | undefined;
      if (pinned && Array.isArray(pinned) && pinned.length > 0) pinnedApps = pinned;
      if (typeof cfg.panelOpacity === 'number') panelOpacity = cfg.panelOpacity;
      if (typeof cfg.panelSize === 'number' && cfg.panelSize > 0) panelHeight = cfg.panelSize;

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
  });

  onDestroy(() => {
    clearInterval(clockTimer);
    clearInterval(weatherTimer);
    clearInterval(clipboardTimer);
    clearTimeout(clipboardHoverTimer);
    clearTimeout(clockHoverTimer);
    unsubConfig?.();
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

<div
  class="absolute top-0 left-0 right-0 backdrop-blur-sm border-b border-white/5 flex items-center justify-between px-3 z-50 select-none"
  style="height:{panelHeight}px; background-color:rgba(15, 23, 42, {panelOpacity});"
>
  <!-- Left: Start + search -->
  <div class="flex items-center gap-3 w-1/3">
    <button
      on:click={handleStartClick}
      class="flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg transition-all group {isStartMenuOpen ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/5 text-slate-300 hover:text-white'}"
      title="Start (double-click for full screen)"
    >
      <div class="relative">
        <Command size={18} class="group-hover:rotate-12 transition-transform duration-200" />
        <div class="absolute -top-1 -right-1 w-1.5 h-1.5 bg-blue-500 rounded-full" />
      </div>
      <span class="font-bold text-sm tracking-tight hidden sm:block">Blue</span>
    </button>
    <div
      class="hidden md:flex items-center gap-2 bg-slate-800/80 hover:bg-slate-700/80 border border-white/5 rounded-full px-3 py-1 text-xs text-slate-400 cursor-text transition-colors w-44"
      on:click={() => dispatch('startClick')}
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

    {#if weather}
      {@const wi = weatherIconFor(weather.code)}
      <div class="hidden lg:block relative">
        <button
          on:click={() => (showWeatherPopover = !showWeatherPopover)}
          class="flex items-center gap-1.5 px-2 py-1 rounded-full hover:bg-white/5 transition-colors {showWeatherPopover ? 'bg-white/10' : ''}"
          title="{weather.city}: {weather.temp} — click for details"
        >
          <svelte:component this={wi.Icon} size={14} class={wi.cls} />
          <span class="text-xs font-medium text-slate-200">{weather.temp}</span>
        </button>

        {#if showWeatherPopover}
          <div class="fixed inset-0 z-40" on:click={() => (showWeatherPopover = false)} />
          <div class="absolute right-0 top-full mt-2 w-64 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl p-4 z-50">
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

    <div class="relative" on:mouseenter={onClipboardEnter} on:mouseleave={onClipboardLeave}>
      <button on:click={() => dispatch('toggleClipboard')}
        class="relative p-2 rounded-full transition-colors group {isClipboardOpen ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-300'}"
        title="Clipboard history">
        <Clipboard size={15} class="group-hover:text-white" />
        {#if hasClipboardContent}
          <span class="absolute top-1.5 right-1.5 w-1.5 h-1.5 bg-blue-500 rounded-full" />
        {/if}
      </button>

      {#if showClipboardPreview && clipboardHoverPreviewEnabled}
        <div class="absolute right-0 top-full mt-2 w-64 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl p-3 z-50">
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
        class="flex items-center gap-2 px-3 py-1.5 rounded-full hover:bg-white/10 transition-colors border border-transparent hover:border-white/5">
        <Wifi size={13} class="text-slate-300" />
        <span class="text-xs font-medium text-slate-200 tabular-nums">
          {time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </span>
      </button>

      {#if showClockPopover && networkHoverInfoEnabled}
        <div class="absolute right-0 top-full mt-2 w-56 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl p-3 z-50 text-xs">
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
