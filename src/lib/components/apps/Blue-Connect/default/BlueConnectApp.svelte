<script lang="ts">
  // Blue Connect — LAN device discovery + pairing, KDE-Connect-style.
  // See src-tauri/src/BlueConnect/mod.rs's module doc for exactly what
  // "real" means here: genuine UDP broadcast discovery on KDE Connect's
  // real port (can see real KDE Connect/GSConnect devices), but
  // plaintext TCP pairing with no TLS — meaningfully pairs only with
  // another Blue Connect instance. This UI is honest about that
  // distinction rather than presenting every discovered device as
  // equally pairable.
  import { onMount, onDestroy } from 'svelte';
  import { RefreshCw, Smartphone, Tablet, Monitor, Laptop, Tv, HelpCircle, Link2, Unlink, Radio, Loader2, Info } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import LoadingSpinner from '../../../LoadingSpinner.svelte';
  import type { DiscoveredDevice, DeviceType } from './types';
  import { DEVICE_TYPE_LABELS } from './types';

  export let windowId: string;

  let devices: DiscoveredDevice[] = [];
  let discovering = false;
  let listeningForPairing = false;
  let pairingDeviceId: string | null = null;
  let error: string | null = null;

  const ICONS: Record<DeviceType, typeof Smartphone> = {
    phone: Smartphone,
    tablet: Tablet,
    desktop: Monitor,
    laptop: Laptop,
    tv: Tv,
    unknown: HelpCircle,
  };

  async function loadKnownDevices() {
    devices = await SystemBridge.bcGetDevices();
  }

  async function runDiscovery() {
    discovering = true;
    error = null;
    try {
      // Broadcasting + waiting for replies takes a few real seconds —
      // this button disables itself and shows a spinner for that
      // whole window rather than the shell looking like it did
      // nothing (see the project's own "don't let a slow backend call
      // freeze things silently" note).
      await SystemBridge.bcStartDiscovery(3);
      await loadKnownDevices();
    } finally {
      discovering = false;
    }
  }

  async function pair(device: DiscoveredDevice) {
    pairingDeviceId = device.id;
    error = null;
    try {
      const res = await SystemBridge.bcRequestPairing(device.id);
      if (!res.ok) {
        error = res.error ?? `Failed to pair with ${device.name}`;
      } else {
        await loadKnownDevices();
      }
    } finally {
      pairingDeviceId = null;
    }
  }

  async function forget(device: DiscoveredDevice) {
    await SystemBridge.bcForgetDevice(device.id);
    await loadKnownDevices();
  }

  async function toggleListening() {
    if (listeningForPairing) return; // one listen cycle already in flight
    listeningForPairing = true;
    try {
      const incomingId = await SystemBridge.bcListenForPairing(30);
      if (incomingId) await loadKnownDevices();
    } finally {
      listeningForPairing = false;
    }
  }

  onMount(() => {
    loadKnownDevices();
  });
</script>

<div class="relative flex flex-col h-full bg-slate-950 text-slate-100 text-sm">
  <div class="px-4 py-3 border-b border-white/10 flex items-center justify-between">
    <div>
      <h1 class="font-medium">Blue Connect</h1>
      <p class="text-[11px] text-slate-500">Discover and pair devices on your local network</p>
    </div>
    <div class="flex items-center gap-2">
      <button
        on:click={toggleListening}
        disabled={listeningForPairing}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-colors {listeningForPairing ? 'bg-emerald-600/30 text-emerald-300' : 'bg-slate-800 hover:bg-slate-700 text-slate-300'}"
        title="Listen for an incoming pairing request from another device"
      >
        {#if listeningForPairing}<Loader2 class="w-3.5 h-3.5 animate-spin" /> Listening…{:else}<Radio class="w-3.5 h-3.5" /> Listen{/if}
      </button>
      <button
        on:click={runDiscovery}
        disabled={discovering}
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-50 transition-colors"
      >
        {#if discovering}<Loader2 class="w-3.5 h-3.5 animate-spin" /> Discovering…{:else}<RefreshCw class="w-3.5 h-3.5" /> Discover devices{/if}
      </button>
    </div>
  </div>

  <div class="px-4 py-2 border-b border-white/5 flex items-start gap-1.5 text-[11px] text-slate-500">
    <Info class="w-3 h-3 shrink-0 mt-0.5" />
    Pairing here uses a plain, unencrypted handshake — it can see real KDE Connect/GSConnect devices during discovery, but only pairs safely with another Blue Connect instance. See mod.rs for why.
  </div>

  <div class="flex-1 overflow-y-auto p-4">
    {#if discovering && devices.length === 0}
      <LoadingSpinner label="Broadcasting on the local network…" />
    {:else if devices.length === 0}
      <div class="flex flex-col items-center gap-2 py-16 text-center text-slate-500">
        <Radio class="w-8 h-8 opacity-30" />
        <p class="text-sm">No devices found yet.</p>
        <p class="text-xs max-w-xs">Click "Discover devices" to broadcast on your local network, or "Listen" to wait for another device to pair with you.</p>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-3">
        {#each devices as device (device.id)}
          <div class="rounded-xl border border-white/10 bg-slate-900/60 p-3 flex items-start gap-3">
            <div class="w-10 h-10 rounded-lg bg-slate-800 flex items-center justify-center shrink-0">
              <svelte:component this={ICONS[device.deviceType]} class="w-5 h-5 text-blue-400" />
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="font-medium truncate">{device.name}</span>
                {#if device.paired}<span class="text-[9px] uppercase tracking-wide px-1.5 py-0.5 rounded bg-emerald-600/20 text-emerald-400 shrink-0">Paired</span>{/if}
              </div>
              <p class="text-[11px] text-slate-500 mt-0.5">{DEVICE_TYPE_LABELS[device.deviceType]} · {device.address}</p>
              <div class="mt-2">
                {#if device.paired}
                  <button on:click={() => forget(device)} class="flex items-center gap-1 text-xs text-red-400 hover:text-red-300">
                    <Unlink class="w-3 h-3" /> Unpair
                  </button>
                {:else}
                  <button
                    on:click={() => pair(device)}
                    disabled={pairingDeviceId === device.id}
                    class="flex items-center gap-1 text-xs text-blue-400 hover:text-blue-300 disabled:opacity-50"
                  >
                    {#if pairingDeviceId === device.id}<Loader2 class="w-3 h-3 animate-spin" /> Pairing…{:else}<Link2 class="w-3 h-3" /> Pair{/if}
                  </button>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if error}
    <div class="absolute bottom-3 right-3 bg-red-500/90 text-white text-xs px-3 py-2 rounded shadow-lg">{error}</div>
  {/if}
</div>
