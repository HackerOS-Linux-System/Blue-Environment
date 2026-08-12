<script lang="ts">
  import { Wifi, WifiOff, Bluetooth, BluetoothOff, Volume2, VolumeX, Sun, Moon, BatteryCharging, ChevronRight, RefreshCw, Speaker, Check, Signal, SignalZero, MonitorCheck } from 'lucide-svelte';
  import { SystemBridge } from '../utils/systemBridge';
  import { CompositorBridge } from '../utils/compositorBridge';
  import { t } from '../stores/language';
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';

  interface AudioSink { id: number; name: string; description: string; volume: number; muted: boolean; is_default: boolean; }

  export let isOpen = false;

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
    wifiEnabled = stats.wifiSSID !== 'Disconnected' && stats.wifiSSID !== '';
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
    if (!next) wifiSSID = 'Disconnected';
  }
  async function handleSinkSelect(sink: AudioSink) {
    await SystemBridge.setDefaultSink(sink.name);
    sinks = sinks.map((s) => ({ ...s, is_default: s.name === sink.name }));
    showSinks = false;
  }

  $: batteryColor = battery < 20 ? 'text-red-400' : battery < 50 ? 'text-yellow-400' : 'text-green-400';
  $: defaultSink = sinks.find((s) => s.is_default);

  onDestroy(() => { unsubHdrPromise?.then((fn) => fn()); });
</script>

{#if isOpen}
  <div class="absolute top-14 right-4 w-80 bg-slate-900/98 border border-white/10 rounded-2xl shadow-2xl p-3 z-50 backdrop-blur-xl">
    <div class="grid grid-cols-2 gap-2 mb-2">
      <button on:click={() => dispatch('openSettings', 'wifi')} class="p-3 rounded-xl flex items-center gap-2 transition-all text-left group relative {wifiEnabled ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}">
        <div class="p-1.5 rounded-full bg-white/20 shrink-0" on:click={(e) => { e.stopPropagation(); handleToggleWifi(); }}>
          {#if wifiEnabled}<Wifi size={14} />{:else}<WifiOff size={14} />{/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="text-xs font-bold leading-none">Wi-Fi</div>
          <div class="text-[10px] opacity-70 truncate mt-0.5">{wifiEnabled ? wifiSSID : 'Off'}</div>
        </div>
        <ChevronRight size={12} class="opacity-0 group-hover:opacity-60 transition-opacity shrink-0" />
      </button>
      <button on:click={() => dispatch('openSettings', 'bluetooth')} class="p-3 rounded-xl flex items-center gap-2 transition-all text-left group {btEnabled ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400'}">
        <div class="p-1.5 rounded-full bg-white/20 shrink-0" on:click={(e) => { e.stopPropagation(); btEnabled = !btEnabled; }}>
          {#if btEnabled}<Bluetooth size={14} />{:else}<BluetoothOff size={14} />{/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="text-xs font-bold leading-none">Bluetooth</div>
          <div class="text-[10px] opacity-70 mt-0.5">{btEnabled ? 'On' : 'Off'}</div>
        </div>
        <ChevronRight size={12} class="opacity-0 group-hover:opacity-60 transition-opacity shrink-0" />
      </button>
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
      <button on:click={() => (darkMode = !darkMode)} class="p-3 rounded-xl flex items-center gap-2 transition-all {darkMode ? 'bg-slate-700 text-white' : 'bg-amber-400/20 text-amber-300'}">
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
