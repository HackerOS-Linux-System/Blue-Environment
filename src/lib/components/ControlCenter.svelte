<script lang="ts">
  import { Wifi, WifiOff, Bluetooth, BluetoothOff, Volume2, VolumeX, Sun, Moon, BatteryCharging, ChevronRight, ChevronDown, RefreshCw, Speaker, Check, Signal, SignalZero, MonitorCheck, Lock, Unlock, Loader2, Settings as SettingsIcon, Pencil, Trash2 } from 'lucide-svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { CompositorBridge } from '../utils/compositorBridge';
  import { configStore } from '../utils/configStore';
  import { dialogPrompt, dialogAlert, dialogConfirm } from '../stores/dialog';
  import { t } from '../stores/language';
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';

  interface AudioSink { id: number; name: string; description: string; volume: number; muted: boolean; is_default: boolean; }
  interface WifiNetwork { ssid: string; signal: number; secure: boolean; in_use: boolean; outOfRange?: boolean; }
  interface BtDevice { name: string; mac: string; connected: boolean; device_type: string; battery?: number; rssi?: number | null; }

  export let isOpen = false;

  // The Control Center panel itself is always mounted (see App.svelte —
  // `isOpen` just toggles CSS visibility), so `onDestroy` never fires
  // merely from closing it. Without this, the live Wi-Fi signal-strength
  // poll (see `toggleWifiExpanded` below) would keep silently re-scanning
  // in the background forever once started, even with the panel closed.
  $: if (!isOpen) {
    if (wifiPollTimer) { clearInterval(wifiPollTimer); wifiPollTimer = undefined; }
    if (btRssiTimer) { clearInterval(btRssiTimer); btRssiTimer = undefined; }
    wifiExpanded = false;
    btExpanded = false;
  }
  export let panelPosition: 'top' | 'bottom' = 'top';
  export let panelSize = 48;
  export let shellThemeId: string | undefined = undefined;

  const dispatch = createEventDispatcher<{ openSettings: string | undefined }>();

  let volume = 60;
  let brightness = 80;
  let battery = 85;
  let isCharging = false;
  let wifiEnabled = true;
  let wifiSSID = '...';
  let btEnabled = true;
  let darkMode = true;
  let muted = false;
  let sinks: AudioSink[] = [];
  let showSinks = false;

  // ── Inline Wi-Fi panel (expands in place instead of jumping to
  // Settings — clicking the tile body used to always dispatch
  // 'openSettings', which fully navigated away from the Control Center
  // just to flip a network on) ────────────────────────────────────────
  let wifiExpanded = false;
  let wifiNetworks: WifiNetwork[] = [];
  let wifiScanning = false;
  let wifiConnecting: string | null = null;
  let savedWifiConnections: string[] = [];
  let wifiForgetting: string | null = null;
  let wifiRenaming: string | null = null;
  let wifiPollTimer: ReturnType<typeof setInterval> | undefined;

  async function toggleWifiExpanded() {
    if (!wifiEnabled) return; // nothing to scan/connect to with the radio off — the toggle circle (not this expand arrow) is what turns it back on
    wifiExpanded = !wifiExpanded;
    if (wifiExpanded) {
      btExpanded = false;
      await Promise.all([scanWifi(), refreshSavedWifi()]);
      // Live signal-strength updates while the panel is open — a
      // background re-scan every few seconds, not gated on the
      // `wifiScanning` flag so the list quietly refreshes signal %
      // without flashing the "Scanning…" spinner state each time.
      wifiPollTimer = setInterval(() => { scanWifi(true); }, 4000);
    } else if (wifiPollTimer) {
      clearInterval(wifiPollTimer);
      wifiPollTimer = undefined;
    }
  }
  async function scanWifi(silent = false) {
    if (!silent) wifiScanning = true;
    try { wifiNetworks = await SystemBridge.getWifiNetworks(); } finally { if (!silent) wifiScanning = false; }
  }
  async function refreshSavedWifi() {
    savedWifiConnections = await SystemBridge.getSavedWifiConnections();
  }
  // Saved connections that *aren't* currently visible in a scan (e.g.
  // work Wi-Fi while at home) previously had no way to be managed at
  // all — the list only ever showed what was in range right now. These
  // are synthesized as regular `WifiNetwork` entries (`outOfRange:
  // true`, no live signal since there isn't one) so they render with
  // the exact same row UI, just visually muted and sorted after
  // in-range networks.
  $: outOfRangeSaved = savedWifiConnections
    .filter((name) => !wifiNetworks.some((n) => n.ssid === name))
    .map((name): WifiNetwork => ({ ssid: name, signal: 0, secure: true, in_use: false, outOfRange: true }));
  $: displayedNetworks = [...wifiNetworks, ...outOfRangeSaved];

  async function connectWifiNetwork(net: WifiNetwork) {
    if (net.in_use) { await SystemBridge.disconnectWifi(); await scanWifi(); return; }
    if (net.outOfRange) {
      // Saved but not currently visible in a scan — bring the existing
      // profile up by name instead of the scan-based connect flow,
      // which only works for SSIDs the last scan actually saw.
      wifiConnecting = net.ssid;
      try {
        await SystemBridge.connectSavedWifiConnection(net.ssid);
        await Promise.all([scanWifi(), refreshSavedWifi()]);
        await refresh();
      } catch {
        await dialogAlert({ title: $t('settings.wifi.connect_failed_title'), message: $t('settings.wifi.connect_failed_message').replace('{ssid}', net.ssid) });
      } finally {
        wifiConnecting = null;
      }
      return;
    }
    let password = '';
    if (net.secure) {
      const entered = await dialogPrompt({
        title: net.ssid,
        label: $t('settings.wifi.secured'),
        placeholder: $t('settings.wifi.title') + ' ' + $t('settings.common.connect'),
        inputType: 'password',
        confirmLabel: $t('settings.common.connect'),
      });
      if (entered === null) return; // cancelled
      password = entered;
    }
    wifiConnecting = net.ssid;
    try {
      await SystemBridge.connectWifi(net.ssid, password);
      await scanWifi();
      await refreshSavedWifi();
      await refresh();
    } catch {
      await dialogAlert({ title: $t('settings.wifi.connect_failed_title'), message: $t('settings.wifi.connect_failed_message').replace('{ssid}', net.ssid) });
    } finally {
      wifiConnecting = null;
    }
  }
  async function forgetWifiNetwork(net: WifiNetwork, e: Event) {
    e.stopPropagation();
    const ok = await dialogConfirm({
      title: $t('settings.wifi.forget_title') ?? 'Zapomnij sieć',
      message: ($t('settings.wifi.forget_message') ?? 'Usunąć zapisane hasło do „{ssid}"? Będziesz musiał połączyć się ponownie ręcznie.').replace('{ssid}', net.ssid),
      confirmLabel: $t('settings.wifi.forget_action') ?? 'Zapomnij',
      danger: true,
    });
    if (!ok) return;
    wifiForgetting = net.ssid;
    try {
      await SystemBridge.forgetWifiNetwork(net.ssid);
      await Promise.all([scanWifi(), refreshSavedWifi()]);
    } finally {
      wifiForgetting = null;
    }
  }
  async function renameWifiNetwork(net: WifiNetwork, e: Event) {
    e.stopPropagation();
    const newName = await dialogPrompt({
      title: $t('settings.wifi.rename_title') ?? 'Zmień nazwę połączenia',
      defaultValue: net.ssid,
      confirmLabel: $t('settings.common.save') ?? 'Zapisz',
    });
    if (!newName?.trim() || newName.trim() === net.ssid) return;
    wifiRenaming = net.ssid;
    try {
      await SystemBridge.renameSavedWifiConnection(net.ssid, newName.trim());
      await Promise.all([scanWifi(), refreshSavedWifi()]);
    } finally {
      wifiRenaming = null;
    }
  }
  /** Changes the stored password directly (modify + re-apply), without
   * the full disconnect/rescan/manual-reconnect cycle a normal
   * "forget, then reconnect with a new password" flow would need — see
   * `updateSavedWifiPassword`'s doc comment in systemBridge.ts. */
  async function changeWifiPassword(net: WifiNetwork, e: Event) {
    e.stopPropagation();
    const newPassword = await dialogPrompt({
      title: $t('settings.wifi.change_password_title') ?? 'Zmień hasło',
      label: net.ssid,
      inputType: 'password',
      confirmLabel: $t('settings.common.save') ?? 'Zapisz',
    });
    if (newPassword === null || !newPassword) return;
    wifiRenaming = net.ssid;
    try {
      await SystemBridge.updateSavedWifiPassword(net.ssid, newPassword);
      await Promise.all([scanWifi(), refreshSavedWifi()]);
    } finally {
      wifiRenaming = null;
    }
  }

  // ── Inline Bluetooth panel (same idea) ──────────────────────────────
  let btExpanded = false;
  let btDevices: BtDevice[] = [];
  let btScanning = false;
  let btBusy: string | null = null;
  let btRssiTimer: ReturnType<typeof setInterval> | undefined;

  async function toggleBtExpanded() {
    if (!btEnabled) return; // nothing to scan/connect to with the adapter off — the toggle circle (not this expand arrow) is what turns it back on
    btExpanded = !btExpanded;
    if (btExpanded) {
      wifiExpanded = false;
      await scanBt();
      // Live RSSI for connected devices where the controller actually
      // exposes it (see getBluetoothRssi's doc comment) — polled
      // separately from `scanBt` since RSSI is per-device and scanning
      // the whole device list is comparatively slow.
      btRssiTimer = setInterval(refreshBtRssi, 4000);
    } else if (btRssiTimer) {
      clearInterval(btRssiTimer);
      btRssiTimer = undefined;
    }
  }
  async function scanBt() {
    btScanning = true;
    try { btDevices = await SystemBridge.getBluetoothDevices(); await refreshBtRssi(); } finally { btScanning = false; }
  }
  async function refreshBtRssi() {
    const connected = btDevices.filter((d) => d.connected);
    if (!connected.length) return;
    const results = await Promise.all(connected.map((d) => SystemBridge.getBluetoothRssi(d.mac)));
    btDevices = btDevices.map((d) => {
      const idx = connected.indexOf(d);
      return idx >= 0 ? { ...d, rssi: results[idx] } : d;
    });
  }
  async function toggleBtDevice(dev: BtDevice) {
    btBusy = dev.mac;
    try { await SystemBridge.toggleBluetoothDevice(dev.mac); await scanBt(); } finally { btBusy = null; }
  }
  async function forgetBtDevice(dev: BtDevice, e: Event) {
    e.stopPropagation();
    const ok = await dialogConfirm({
      title: $t('settings.bluetooth.forget_title') ?? 'Zapomnij urządzenie',
      message: ($t('settings.bluetooth.forget_message') ?? 'Odparować „{name}"? Będziesz musiał sparować je ponownie, aby użyć go później.').replace('{name}', dev.name),
      confirmLabel: $t('settings.bluetooth.forget_action') ?? 'Zapomnij',
      danger: true,
    });
    if (!ok) return;
    btBusy = dev.mac;
    try { await SystemBridge.bluetoothForget(dev.mac); await scanBt(); } finally { btBusy = null; }
  }

  // ── Dark / Light mode. Previously this button just flipped a local
  // `darkMode` boolean that nothing else ever read — visually it looked
  // like a toggle but it didn't actually change anything. It's now
  // backed by `config.theme` (the same field `App.svelte` already
  // renders onto the root as `data-theme`, and that `app.css` already
  // has real light-mode overrides for under `[data-theme='light-glass']`
  // — see that file), so flipping it here genuinely re-themes the shell. */
  const LIGHT_THEME_ID = 'light-glass';
  const DARK_THEME_ID = 'dark';
  let unsubConfig: (() => void) | undefined;

  async function toggleDarkMode() {
    const next = darkMode ? LIGHT_THEME_ID : DARK_THEME_ID;
    darkMode = !darkMode; // optimistic — subscription below will confirm
    await configStore.save({ theme: next });
  }

  // Android-style mobile data toggle — only shown on hardware that actually
  // has a WWAN/cellular modem (see SystemBridge.hasCellularModem, backed by
  // ModemManager). Laptops without one simply never see this control.
  let hasCellular = false;
  let cellularEnabled = false;
  let cellularCarrier = '';

  // Real HDR status (not a toggle here — that lives in Settings > Monitors,
  // see MonitorsSection.svelte). This tile just reflects what the
  // compositor actually reports back via HdrStateChanged — honestly
  // "off" until protocols/color_management.rs's render-side tone-mapping
  // stub is filled in, even right after the user enables it, per that
  // module's doc comment. Reflecting the real (not optimistic) state
  // here is the whole point: no fake "HDR On" badge for a picture that
  // hasn't actually changed.
  let hdrActive = false;
  let unsubHdrPromise: Promise<() => void> | undefined;

  onMount(async () => {
    hasCellular = await SystemBridge.hasCellularModem().catch(() => false);
    if (hasCellular) refreshCellular();
    unsubHdrPromise = CompositorBridge.onHdrStateChanged((_output, active) => {
      hdrActive = active;
    });
    const cfg = await configStore.load();
    darkMode = cfg.theme !== LIGHT_THEME_ID;
    unsubConfig = configStore.subscribe((cfg2) => { darkMode = cfg2.theme !== LIGHT_THEME_ID; });
  });

  async function refreshCellular() {
    const status = await SystemBridge.getCellularStatus().catch(() => null);
    if (status) { cellularEnabled = status.connected; cellularCarrier = status.carrier; }
  }

  async function toggleCellular() {
    const next = !cellularEnabled;
    cellularEnabled = next;
    await SystemBridge.setCellularEnabled(next).catch(() => {});
    setTimeout(refreshCellular, 1500);
  }

  async function refresh() {
    if (!isOpen) return;
    const stats = await SystemBridge.getSystemStats();
    volume = stats.volume;
    brightness = stats.brightness >= 0 ? stats.brightness : 80;
    battery = stats.battery;
    isCharging = stats.isCharging;
    wifiSSID = stats.wifiSSID;
    // Previously derived from `stats.wifiSSID !== 'Disconnected'` — i.e.
    // "am I currently connected to a network", which is a different
    // question from "is the radio powered on" and was getting
    // overwritten back to a wrong value on every refresh regardless of
    // what the on/off toggle had just set. A powered-on radio with no
    // active connection (the normal state right after turning Wi-Fi on,
    // before picking a network) would incorrectly show as "Off" here,
    // and — since nothing anywhere checked this flag before allowing a
    // scan/connect — a user could still browse and connect to networks
    // while it displayed "Off": the toggle was purely decorative.
    wifiEnabled = await SystemBridge.getWifiRadioEnabled();
    btEnabled = await SystemBridge.getBluetoothPowered();
    const audioSinks = await SystemBridge.getAudioSinks();
    sinks = audioSinks;
    const def = (audioSinks as AudioSink[]).find((s) => s.is_default);
    if (def) { volume = def.volume; muted = def.muted; }
  }

  $: if (isOpen) refresh();

  async function handleVolume(val: number) { volume = val; await SystemBridge.setVolume(val); }
  async function handleBrightness(val: number) { brightness = val; await SystemBridge.setBrightness(val); }
  async function handleToggleMute() {
    const def = sinks.find((s) => s.is_default);
    if (def) { await SystemBridge.toggleSinkMute(def.name); muted = !muted; }
  }
  async function handleToggleWifi() {
    const next = !wifiEnabled;
    wifiEnabled = next;
    await SystemBridge.toggleWifi(next);
    if (!next) {
      wifiSSID = 'Disconnected';
      wifiExpanded = false; // nothing to browse/connect to with the radio off — see the guard in the expanded panel too
      if (wifiPollTimer) { clearInterval(wifiPollTimer); wifiPollTimer = undefined; }
    }
  }
  /** Was previously just `btEnabled = !btEnabled` inline in the
   * template — a bare local-variable flip with no call to
   * `settings_bluetooth_toggle` at all, so the adapter's real power
   * state never actually changed no matter what the switch displayed.
   * That command already existed and was already registered; nothing
   * was calling it. */
  async function handleToggleBluetooth() {
    const next = !btEnabled;
    btEnabled = next;
    await SystemBridge.setBluetoothPowered(next);
    if (!next) {
      btExpanded = false; // nothing to scan/connect to with the adapter off
      if (btRssiTimer) { clearInterval(btRssiTimer); btRssiTimer = undefined; }
    }
  }
  async function handleSinkSelect(sink: AudioSink) {
    await SystemBridge.setDefaultSink(sink.name);
    sinks = sinks.map((s) => ({ ...s, is_default: s.name === sink.name }));
    showSinks = false;
  }

  $: batteryColor = battery < 20 ? 'text-red-400' : battery < 50 ? 'text-yellow-400' : 'text-green-400';
  $: defaultSink = sinks.find((s) => s.is_default);

  onDestroy(() => { unsubHdrPromise?.then((fn) => fn()); unsubConfig?.(); if (wifiPollTimer) clearInterval(wifiPollTimer); if (btRssiTimer) clearInterval(btRssiTimer); });
</script>

{#if isOpen}
  <div class="absolute right-4 w-80 border rounded-2xl shadow-2xl p-3 z-50 backdrop-blur-xl {shellThemeId === 'hydra' ? 'bg-slate-900/98 border-pink-500/30' : 'bg-slate-900/98 border-white/10'}"
    style="{panelPosition === 'top' ? `top:${panelSize + 8}px;` : `bottom:${panelSize + 8}px;`} {shellThemeId === 'hydra' ? 'box-shadow: 0 0 40px rgba(236,72,153,0.25), 0 25px 50px -12px rgba(0,0,0,0.7);' : ''}"
  >
    <div class="grid grid-cols-2 gap-2 mb-2">
      <button on:click={toggleWifiExpanded} class="p-3 rounded-xl flex items-center gap-2 transition-all text-left group relative {wifiEnabled ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}">
        <div class="p-1.5 rounded-full bg-white/20 shrink-0" on:click={(e) => { e.stopPropagation(); handleToggleWifi(); }} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); ((e) => { e.stopPropagation(); handleToggleWifi(); })(e); } }}>
          {#if wifiEnabled}<Wifi size={14} />{:else}<WifiOff size={14} />{/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="text-xs font-bold leading-none">Wi-Fi</div>
          <div class="text-[10px] opacity-70 truncate mt-0.5">{wifiEnabled ? wifiSSID : 'Off'}</div>
        </div>
        <ChevronDown size={12} class="opacity-60 transition-transform shrink-0 {wifiExpanded ? 'rotate-180' : ''}" />
      </button>
      <button on:click={toggleBtExpanded} class="p-3 rounded-xl flex items-center gap-2 transition-all text-left group {btEnabled ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}">
        <div class="p-1.5 rounded-full bg-white/20 shrink-0" on:click={(e) => { e.stopPropagation(); handleToggleBluetooth(); }} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); ((e) => { e.stopPropagation(); handleToggleBluetooth(); })(e); } }}>
          {#if btEnabled}<Bluetooth size={14} />{:else}<BluetoothOff size={14} />{/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="text-xs font-bold leading-none">Bluetooth</div>
          <div class="text-[10px] opacity-70 mt-0.5">{btEnabled ? 'On' : 'Off'}</div>
        </div>
        <ChevronDown size={12} class="opacity-60 transition-transform shrink-0 {btExpanded ? 'rotate-180' : ''}" />
      </button>
    </div>

    {#if wifiExpanded}
      <div class="bg-slate-800 border border-white/5 rounded-xl mb-2 overflow-hidden">
        <div class="flex items-center justify-between px-3 py-2 border-b border-white/5">
          <span class="text-[11px] font-medium text-slate-300">{$t('settings.wifi.title')}</span>
          <div class="flex items-center gap-1">
            <button on:click={() => scanWifi()} disabled={wifiScanning} class="p-1 rounded-full hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><RefreshCw size={11} class={wifiScanning ? 'animate-spin' : ''} /></button>
            <button on:click={() => dispatch('openSettings', 'wifi')} title={$t('control_center.open_settings')} class="p-1 rounded-full hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><SettingsIcon size={11} /></button>
          </div>
        </div>
        <div class="max-h-44 overflow-y-auto">
          {#if wifiScanning && displayedNetworks.length === 0}
            <div class="p-3 text-center flex items-center justify-center gap-2 text-slate-500 text-xs"><Loader2 size={13} class="animate-spin" /> {$t('settings.common.scanning')}</div>
          {:else if displayedNetworks.length === 0}
            <div class="p-3 text-center text-slate-500 text-xs">{$t('settings.wifi.no_networks')}</div>
          {/if}
          {#each displayedNetworks as net, i (net.ssid)}
            {@const isSaved = savedWifiConnections.includes(net.ssid)}
            <div class="group relative flex items-center border-t border-white/5 first:border-0 transition-colors {net.in_use ? 'bg-blue-600/10' : 'hover:bg-white/5'} {net.outOfRange ? 'opacity-50' : ''}">
              <button on:click={() => connectWifiNetwork(net)} disabled={wifiConnecting !== null}
                class="flex-1 min-w-0 flex items-center gap-2 px-3 py-2 text-left">
                <Wifi size={13} class={net.outOfRange ? 'text-slate-500 shrink-0' : net.signal > 60 ? 'text-green-400 shrink-0' : 'text-yellow-400 shrink-0'} />
                <div class="min-w-0 flex-1">
                  <div class="text-xs text-white truncate flex items-center gap-1.5">
                    {net.ssid}
                    {#if net.in_use}<Check size={10} class="text-green-400 shrink-0" />{/if}
                  </div>
                  <div class="text-[10px] text-slate-400 flex items-center gap-1">
                    {#if net.outOfRange}
                      <span>{$t('settings.wifi.out_of_range') ?? 'Poza zasięgiem'}</span>
                    {:else}
                      {#if net.secure}<Lock size={9} />{:else}<Unlock size={9} />{/if}
                      {net.signal}%
                      {#if isSaved}<span class="text-slate-500">· {$t('settings.wifi.saved') ?? 'zapisana'}</span>{/if}
                    {/if}
                  </div>
                </div>
              </button>
              {#if wifiConnecting === net.ssid || wifiForgetting === net.ssid || wifiRenaming === net.ssid}
                <Loader2 size={12} class="animate-spin text-slate-400 shrink-0 mr-3" />
              {:else}
                {#if net.in_use}<span class="text-[10px] text-red-400 shrink-0 mr-2">{$t('settings.common.disconnect')}</span>{/if}
                {#if isSaved}
                  <div class="hidden group-hover:flex items-center gap-0.5 shrink-0 mr-2">
                    <button on:click={(e) => changeWifiPassword(net, e)} title={$t('settings.wifi.change_password_title') ?? 'Zmień hasło'} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><Lock size={11} /></button>
                    <button on:click={(e) => renameWifiNetwork(net, e)} title={$t('settings.wifi.rename_title') ?? 'Zmień nazwę'} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><Pencil size={11} /></button>
                    <button on:click={(e) => forgetWifiNetwork(net, e)} title={$t('settings.wifi.forget_action') ?? 'Zapomnij'} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-red-400 transition-colors"><Trash2 size={11} /></button>
                  </div>
                {/if}
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if btExpanded}
      <div class="bg-slate-800 border border-white/5 rounded-xl mb-2 overflow-hidden">
        <div class="flex items-center justify-between px-3 py-2 border-b border-white/5">
          <span class="text-[11px] font-medium text-slate-300">{$t('settings.bluetooth.title')}</span>
          <div class="flex items-center gap-1">
            <button on:click={scanBt} disabled={btScanning} class="p-1 rounded-full hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><RefreshCw size={11} class={btScanning ? 'animate-spin' : ''} /></button>
            <button on:click={() => dispatch('openSettings', 'bluetooth')} title={$t('control_center.open_settings') ?? 'Settings'} class="p-1 rounded-full hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><SettingsIcon size={11} /></button>
          </div>
        </div>
        <div class="max-h-44 overflow-y-auto">
          {#if btScanning && btDevices.length === 0}
            <div class="p-3 text-center flex items-center justify-center gap-2 text-slate-500 text-xs"><Loader2 size={13} class="animate-spin" /> {$t('settings.common.scanning')}</div>
          {:else if btDevices.length === 0}
            <div class="p-3 text-center text-slate-500 text-xs">{$t('settings.bluetooth.no_devices')}</div>
          {/if}
          {#each btDevices as dev, i (i)}
            <div class="group relative flex items-center border-t border-white/5 first:border-0 transition-colors hover:bg-white/5">
              <button on:click={() => toggleBtDevice(dev)} disabled={btBusy !== null}
                class="flex-1 min-w-0 flex items-center gap-2 px-3 py-2 text-left">
                <Bluetooth size={13} class="text-blue-400 shrink-0" />
                <div class="min-w-0 flex-1">
                  <div class="text-xs text-white truncate">{dev.name}</div>
                  <div class="text-[10px] text-slate-400">{dev.device_type} · {dev.connected ? $t('settings.common.connected') : $t('settings.common.disconnected')}{dev.battery != null ? ` · ${dev.battery}%` : ''}{dev.connected && dev.rssi != null ? ` · ${dev.rssi} dBm` : ''}</div>
                </div>
              </button>
              {#if btBusy === dev.mac}
                <Loader2 size={12} class="animate-spin text-slate-400 shrink-0 mr-3" />
              {:else}
                <span class="text-[10px] shrink-0 mr-2 {dev.connected ? 'text-red-400' : 'text-blue-400'}">{dev.connected ? $t('settings.common.disconnect') : $t('settings.common.connect')}</span>
                <button on:click={(e) => forgetBtDevice(dev, e)} title={$t('settings.bluetooth.forget_action') ?? 'Zapomnij'} class="hidden group-hover:block p-1 rounded hover:bg-white/10 text-slate-400 hover:text-red-400 transition-colors mr-2 shrink-0"><Trash2 size={11} /></button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <div class="grid grid-cols-2 gap-2 mb-2">
      {#if hasCellular}
        <button on:click={toggleCellular} class="p-3 rounded-xl flex items-center gap-2 transition-all text-left {cellularEnabled ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}">
          <div class="p-1.5 rounded-full bg-white/20 shrink-0">
            {#if cellularEnabled}<Signal size={14} />{:else}<SignalZero size={14} />{/if}
          </div>
          <div class="min-w-0 flex-1">
            <div class="text-xs font-bold leading-none">Mobile Data</div>
            <div class="text-[10px] opacity-70 truncate mt-0.5">{cellularEnabled ? (cellularCarrier || 'Connected') : 'Off'}</div>
          </div>
        </button>
      {/if}
      {#if hdrActive}
        <button on:click={() => dispatch('openSettings', 'monitors')} class="p-3 rounded-xl flex items-center gap-2 transition-all text-left bg-blue-600 text-white">
          <div class="p-1.5 rounded-full bg-white/20 shrink-0">
            <MonitorCheck size={14} />
          </div>
          <div class="min-w-0 flex-1">
            <div class="text-xs font-bold leading-none">{$t('monitors.hdr')}</div>
            <div class="text-[10px] opacity-70 truncate mt-0.5">{$t('control_center.hdr_active')}</div>
          </div>
        </button>
      {/if}
    </div>
    <div class="grid grid-cols-2 gap-2 mb-2">
      <button on:click={toggleDarkMode} class="p-3 rounded-xl flex items-center gap-2 transition-all {darkMode ? 'bg-slate-700 text-white' : 'bg-amber-400/20 text-amber-300'}">
        {#if darkMode}<Moon size={16} />{:else}<Sun size={16} />{/if}
        <span class="text-xs font-bold">{darkMode ? 'Dark' : 'Light'}</span>
      </button>
      <div class="bg-slate-800 rounded-xl p-3">
        <div class="flex items-center justify-between mb-0.5">
          <span class="text-[10px] text-slate-400 font-medium">Battery</span>
          {#if isCharging}<BatteryCharging size={12} class="text-green-400" />{/if}
        </div>
        <div class="text-xl font-bold {batteryColor}">{Math.round(battery)}%</div>
        <div class="text-[10px] text-slate-500 mt-0.5">{isCharging ? 'Charging' : 'On battery'}</div>
      </div>
    </div>
    <div class="bg-slate-800 rounded-xl p-3 mb-2">
      <div class="flex items-center justify-between mb-2">
        <div class="flex items-center gap-2"><Speaker size={13} class="text-slate-400" /><span class="text-xs font-medium text-slate-300">Output</span></div>
        <button on:click={() => (showSinks = !showSinks)} class="text-[10px] text-slate-500 hover:text-white transition-colors flex items-center gap-1">
          {defaultSink ? defaultSink.description.slice(0, 18) : 'Default'}
          <ChevronRight size={10} class="transition-transform {showSinks ? 'rotate-90' : ''}" />
        </button>
      </div>
      {#if showSinks && sinks.length > 0}
        <div class="space-y-1 mb-2 border-t border-white/5 pt-2">
          {#each sinks as sink (sink.id)}
            <button on:click={() => handleSinkSelect(sink)} class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left text-xs transition-colors {sink.is_default ? 'bg-blue-600/30 text-blue-300' : 'hover:bg-white/5 text-slate-400'}">
              {#if sink.is_default}<Check size={10} />{/if}
              <span class="truncate">{sink.description}</span>
            </button>
          {/each}
        </div>
      {/if}
      <div class="flex items-center gap-2">
        <button on:click={handleToggleMute} class="text-slate-400 hover:text-white transition-colors shrink-0">
          {#if muted}<VolumeX size={16} />{:else}<Volume2 size={16} />{/if}
        </button>
        <input type="range" min="0" max="100" value={muted ? 0 : volume}
          on:input={(e) => handleVolume(parseInt(e.currentTarget.value))}
          class="flex-1 h-1.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
        <span class="text-[10px] text-slate-400 w-7 text-right">{muted ? 0 : volume}%</span>
      </div>
    </div>
    {#if brightness >= 0}
      <div class="flex items-center gap-2 px-3 py-2 bg-slate-800 rounded-xl mb-2">
        <Sun size={14} class="text-slate-400 shrink-0" />
        <input type="range" min="5" max="100" value={brightness}
          on:input={(e) => handleBrightness(parseInt(e.currentTarget.value))}
          class="flex-1 h-1.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-yellow-400" />
        <span class="text-[10px] text-slate-400 w-7 text-right">{brightness}%</span>
      </div>
    {/if}
    <div class="flex items-center justify-between pt-1">
      <button on:click={() => dispatch('openSettings', undefined)} class="text-xs text-slate-500 hover:text-white transition-colors">All settings →</button>
      <button on:click={refresh} class="p-1.5 rounded-full hover:bg-white/10 text-slate-500 hover:text-white transition-colors"><RefreshCw size={12} /></button>
    </div>
  </div>
{/if}
