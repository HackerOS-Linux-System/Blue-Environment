<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { Wifi, Bluetooth, Settings, Power, Volume2, Sun, X } from 'lucide-svelte';
  import { t } from '../lib/stores/language';
  import { SystemBridge } from '../lib/utils/systemBridge';

  const dispatch = createEventDispatcher<{ close: void; settings: void; power: void }>();

  let wifi = false; let bt = false; let volume = 50; let brightness = -1;
  onMount(async () => {
    const [stats, w, b] = await Promise.all([
      SystemBridge.getSystemStats().catch(() => null),
      SystemBridge.getWifiRadioEnabled().catch(() => false),
      SystemBridge.getBluetoothPowered().catch(() => false),
    ]);
    if (stats) { volume = stats.volume; brightness = stats.brightness; }
    wifi = w; bt = b;
    // Put the focus into the panel as soon as it is open.
    (document.querySelector('[data-tv-menu] [data-tv-focusable]') as HTMLElement | null)?.focus();
  });

  async function toggleWifi() { const next = !wifi; wifi = next; await SystemBridge.toggleWifi(next).catch(() => { wifi = !next; }); }
  async function toggleBt() { const next = !bt; bt = next; await SystemBridge.setBluetoothPowered(next).catch(() => { bt = !next; }); }
  const setVolume = (e: Event) => { volume = Number((e.currentTarget as HTMLInputElement).value); SystemBridge.setVolume(volume).catch(() => {}); };
  let timer: ReturnType<typeof setTimeout>;
  const setBrightness = (e: Event) => {
    brightness = Number((e.currentTarget as HTMLInputElement).value);
    clearTimeout(timer); timer = setTimeout(() => SystemBridge.setBrightness(brightness).catch(() => {}), 80);
  };
</script>

<div class="absolute inset-0 z-30 bg-black/50" role="presentation" on:click|self={() => dispatch('close')}>
  <aside data-tv-menu class="absolute right-0 top-0 bottom-0 w-[28rem] bg-slate-900/95 backdrop-blur-xl text-white p-8 flex flex-col gap-6 shadow-2xl"
    transition:fly={{ x: 440, duration: 200 }}>
    <div class="flex items-center justify-between">
      <h2 class="text-2xl font-semibold">{$t('altui.quick_settings')}</h2>
      <button data-tv-focusable class="tv-tile p-3 rounded-full bg-white/10" aria-label={$t('startmenu.close')} on:click={() => dispatch('close')}><X size={24} /></button>
    </div>

    <label class="flex items-center gap-4 text-lg"><Volume2 size={28} class="shrink-0" /><span class="sr-only">{$t('osd.volume')}</span>
      <input data-tv-focusable type="range" min="0" max="100" step="5" value={volume} on:input={setVolume} class="flex-1 h-10" /></label>
    {#if brightness >= 0}
      <label class="flex items-center gap-4 text-lg"><Sun size={28} class="shrink-0" /><span class="sr-only">{$t('osd.brightness')}</span>
        <input data-tv-focusable type="range" min="1" max="100" step="5" value={brightness} on:input={setBrightness} class="flex-1 h-10" /></label>
    {/if}

    <div class="grid grid-cols-2 gap-4">
      <button data-tv-focusable class="tv-tile h-24 rounded-2xl flex items-center justify-center gap-3 text-lg {wifi ? 'bg-blue-600' : 'bg-white/10'}" on:click={toggleWifi}><Wifi size={26} />{$t('altui.wifi')}</button>
      <button data-tv-focusable class="tv-tile h-24 rounded-2xl flex items-center justify-center gap-3 text-lg {bt ? 'bg-blue-600' : 'bg-white/10'}" on:click={toggleBt}><Bluetooth size={26} />{$t('altui.bluetooth')}</button>
      <button data-tv-focusable class="tv-tile h-24 rounded-2xl flex items-center justify-center gap-3 text-lg bg-white/10" on:click={() => dispatch('settings')}><Settings size={26} />{$t('altui.settings')}</button>
      <button data-tv-focusable class="tv-tile h-24 rounded-2xl flex items-center justify-center gap-3 text-lg bg-white/10" on:click={() => dispatch('power')}><Power size={26} />{$t('startmenu.power')}</button>
    </div>
  </aside>
</div>
