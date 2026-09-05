<script lang="ts">
  // Blue Connect — LAN device discovery + pairing, KDE-Connect-style.
  // See src-tauri/src/BlueConnect/mod.rs's module doc for exactly what
  // "real" means here: genuine UDP broadcast discovery on KDE Connect's
  // real port (can see real KDE Connect/GSConnect devices), and mutual
  // TLS pairing gated behind an explicit Short Authentication String
  // (SAS) confirmation on both ends (see tls.rs's compute_sas doc) —
  // meaningfully pairs with another Blue Connect instance; this UI is
  // honest about that KDE-Connect-protocol-compatibility distinction
  // rather than presenting every discovered device as equally pairable.
  import { onMount, onDestroy } from 'svelte';
  import { RefreshCw, Smartphone, Tablet, Monitor, Laptop, Tv, HelpCircle, Link2, Unlink, Radio, Loader2, Info, ShieldCheck, X } from 'lucide-svelte';
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

  /** The SAS confirmation dialog's state — shown for both roles:
   * `role: 'initiator'` (we called bcRequestPairing and are waiting for
   * the other person) shows the code with no buttons, just "waiting…";
   * `role: 'incoming'` (someone is pairing with us) shows the code with
   * Accept/Reject buttons wired to bcConfirmIncomingPairing. */
  let sasDialog: { role: 'initiator' | 'incoming'; deviceId: string; deviceName: string; sas: string } | null = null;
  let unlistenSas: (() => void) | null = null;
  let unlistenIncoming: (() => void) | null = null;

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
      // The `blue-connect://pairing-sas` event (subscribed in onMount)
      // fires the SAS dialog for the 'initiator' role while this await
      // is in flight — nothing else to do here but wait for the
      // eventual accept/reject/timeout outcome.
      const res = await SystemBridge.bcRequestPairing(device.id);
      if (!res.ok) {
        error = res.error ?? `Failed to pair with ${device.name}`;
      } else {
        await loadKnownDevices();
      }
    } finally {
      pairingDeviceId = null;
      if (sasDialog?.deviceId === device.id) sasDialog = null;
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
      // The `blue-connect://pairing-request` event fires the SAS
      // dialog for the 'incoming' role while this await is in flight;
      // that dialog's Accept/Reject buttons call
      // bcConfirmIncomingPairing, which is what actually lets this
      // pending bcListenForPairing call resolve.
      const { deviceId, error: err } = await SystemBridge.bcListenForPairing(30);
      if (deviceId) {
        await loadKnownDevices();
      } else if (err) {
        // A plain timeout-with-nobody-trying-to-pair resolves with
        // `deviceId: null` and no `error` at all (see
        // bc_listen_for_pairing's `Ok(None)` case) — only a genuine
        // outcome (declined, TLS failure, ...) sets `err`, so only that
        // case is worth surfacing.
        error = err;
      }
    } finally {
      listeningForPairing = false;
      sasDialog = null;
    }
  }

  async function respondToIncoming(accept: boolean) {
    if (!sasDialog) return;
    await SystemBridge.bcConfirmIncomingPairing(accept);
    sasDialog = null;
  }

  onMount(() => {
    loadKnownDevices();
    (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        unlistenSas = await listen('blue-connect://pairing-sas', (e: any) => {
          const { deviceId, deviceName, sas } = e.payload ?? {};
          if (deviceId) sasDialog = { role: 'initiator', deviceId, deviceName, sas };
        });
        unlistenIncoming = await listen('blue-connect://pairing-request', (e: any) => {
          const { deviceId, deviceName, sas } = e.payload ?? {};
          if (deviceId) sasDialog = { role: 'incoming', deviceId, deviceName, sas };
        });
      } catch {
        /* not running under Tauri — pairing simply won't be available */
      }
    })();
  });

  onDestroy(() => {
    unlistenSas?.();
    unlistenIncoming?.();
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

  {#if sasDialog}
    <div class="absolute inset-0 bg-black/60 flex items-center justify-center z-20">
      <div class="bg-slate-900 border border-white/10 rounded-xl w-80 p-5 flex flex-col gap-4">
        <div class="flex items-center gap-2">
          <ShieldCheck class="w-5 h-5 text-emerald-400 shrink-0" />
          <h3 class="font-medium text-sm">
            {sasDialog.role === 'incoming' ? `Pairing request from ${sasDialog.deviceName}` : `Pairing with ${sasDialog.deviceName}`}
          </h3>
        </div>
        <p class="text-[11px] text-slate-400 leading-relaxed">
          {#if sasDialog.role === 'incoming'}
            Compare this code with the one shown on {sasDialog.deviceName}. If they match, it's genuinely that device — if they don't, something on the network is impersonating it.
          {:else}
            Ask {sasDialog.deviceName} to show its pairing code and compare it with the one below. Waiting for it to accept…
          {/if}
        </p>
        <div class="text-center py-3 bg-slate-800/60 rounded-lg">
          <span class="text-3xl font-mono font-bold tracking-[0.3em] text-emerald-300">{sasDialog.sas}</span>
        </div>
        {#if sasDialog.role === 'incoming'}
          <div class="flex gap-2">
            <button
              on:click={() => respondToIncoming(false)}
              class="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs bg-slate-800 hover:bg-slate-700 text-slate-300 transition-colors"
            >
              <X class="w-3.5 h-3.5" /> Reject
            </button>
            <button
              on:click={() => respondToIncoming(true)}
              class="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs bg-emerald-600 hover:bg-emerald-500 text-white font-medium transition-colors"
            >
              <ShieldCheck class="w-3.5 h-3.5" /> Codes match — Accept
            </button>
          </div>
        {:else}
          <div class="flex items-center justify-center gap-2 text-xs text-slate-500">
            <Loader2 class="w-3.5 h-3.5 animate-spin" /> Waiting for confirmation…
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if error}
    <div class="absolute bottom-3 right-3 bg-red-500/90 text-white text-xs px-3 py-2 rounded shadow-lg">{error}</div>
  {/if}
</div>
