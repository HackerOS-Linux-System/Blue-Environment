<script lang="ts">
  import { self } from 'svelte/legacy';

  import { createEventDispatcher, onDestroy, onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { Wifi, Bluetooth, Settings, Power, Volume2, Sun, X, Trash2 } from 'lucide-svelte';
  import { t } from '../lib/stores/language';
  import { notificationManager } from '../lib/utils/notificationManager';
  import { SystemBridge } from '../lib/utils/systemBridge';
  import { classifySwipe, type SwipeDir } from '../lib/altShell/gestures';
  import type { Notification } from '../lib/types';

  interface Props {
    tab?: 'notifications' | 'quick';
  }

  let { tab = $bindable('notifications') }: Props = $props();

  const dispatch = createEventDispatcher<{ dismiss: void; settings: void; power: void; read: void }>();

  let notes = $state<Notification[]>([]);
  const unsub = notificationManager.subscribe((n) => { notes = n; });
  onDestroy(unsub);

  let wifi = $state(false); let bt = $state(false); let volume = $state(50); let brightness = $state(80);
  onMount(async () => {
    const [stats, w, b] = await Promise.all([
      SystemBridge.getSystemStats().catch(() => null),
      SystemBridge.getWifiRadioEnabled().catch(() => false),
      SystemBridge.getBluetoothPowered().catch(() => false),
    ]);
    if (stats) { volume = stats.volume; if (stats.brightness >= 0) brightness = stats.brightness; }
    wifi = w; bt = b;
    // Opening the shade counts as having seen the notifications.
    dispatch('read');
  });

  async function toggleWifi() { const next = !wifi; wifi = next; await SystemBridge.toggleWifi(next).catch(() => { wifi = !next; }); }
  async function toggleBt() { const next = !bt; bt = next; await SystemBridge.setBluetoothPowered(next).catch(() => { bt = !next; }); }
  const setVolume = (e: Event) => { volume = Number((e.currentTarget as HTMLInputElement).value); SystemBridge.setVolume(volume).catch(() => {}); };
  let brightTimer: ReturnType<typeof setTimeout>;
  const setBrightness = (e: Event) => {
    brightness = Number((e.currentTarget as HTMLInputElement).value);
    clearTimeout(brightTimer);
    brightTimer = setTimeout(() => SystemBridge.setBrightness(brightness).catch(() => {}), 80);
  };

  // Swipe the sheet up to close it.
  let sx = 0; let sy = 0;
  function down(e: PointerEvent) { sx = e.clientX; sy = e.clientY; }
  function up(e: PointerEvent) {
    const s: SwipeDir | null = classifySwipe(e.clientX - sx, e.clientY - sy, { minDistance: 60 });
    if (s === 'up') dispatch('dismiss');
  }
</script>

<div class="absolute inset-0 z-40 bg-black/50" role="presentation" onclick={self(() => dispatch('dismiss'))}>
  <div class="phone-safe-top bg-slate-900/95 backdrop-blur-xl text-white rounded-b-3xl shadow-2xl max-h-[88%] overflow-y-auto"
    transition:fly={{ y: -400, duration: 220 }} onpointerdown={down} onpointerup={up} role="presentation">
    <div class="flex gap-1 p-3">
      <button class="flex-1 h-9 rounded-full text-sm {tab === 'notifications' ? 'bg-white/15' : 'text-white/60'}" onclick={() => (tab = 'notifications')}>{$t('altui.notifications')}</button>
      <button class="flex-1 h-9 rounded-full text-sm {tab === 'quick' ? 'bg-white/15' : 'text-white/60'}" onclick={() => (tab = 'quick')}>{$t('altui.quick_settings')}</button>
    </div>

    {#if tab === 'quick'}
      <div class="px-4 pb-4 space-y-4">
        <div class="grid grid-cols-4 gap-3">
          <button class="h-16 rounded-2xl flex flex-col items-center justify-center gap-1 text-[11px] {wifi ? 'bg-blue-600' : 'bg-white/10'}" onclick={toggleWifi}><Wifi size={20} />{$t('altui.wifi')}</button>
          <button class="h-16 rounded-2xl flex flex-col items-center justify-center gap-1 text-[11px] {bt ? 'bg-blue-600' : 'bg-white/10'}" onclick={toggleBt}><Bluetooth size={20} />{$t('altui.bluetooth')}</button>
          <button class="h-16 rounded-2xl flex flex-col items-center justify-center gap-1 text-[11px] bg-white/10" onclick={() => dispatch('settings')}><Settings size={20} />{$t('altui.settings')}</button>
          <button class="h-16 rounded-2xl flex flex-col items-center justify-center gap-1 text-[11px] bg-white/10" onclick={() => dispatch('power')}><Power size={20} />{$t('startmenu.power')}</button>
        </div>
        <label class="flex items-center gap-3 text-sm"><Volume2 size={18} class="shrink-0" /><span class="sr-only">{$t('osd.volume')}</span>
          <input type="range" min="0" max="100" value={volume} oninput={setVolume} class="flex-1 h-8" /></label>
        <label class="flex items-center gap-3 text-sm"><Sun size={18} class="shrink-0" /><span class="sr-only">{$t('osd.brightness')}</span>
          <input type="range" min="1" max="100" value={brightness} oninput={setBrightness} class="flex-1 h-8" /></label>
      </div>
    {:else}
      <div class="px-4 pb-4 space-y-2">
        {#each notes as n (n.id)}
          <div class="flex items-start gap-3 bg-white/10 rounded-2xl px-4 py-3">
            <div class="flex-1 min-w-0">
              <div class="text-sm font-medium truncate">{n.title}</div>
              {#if n.message || n.body}<div class="text-xs text-white/70 mt-0.5 line-clamp-3">{n.message ?? n.body}</div>{/if}
            </div>
            <button class="p-1 rounded-full active:bg-white/20" aria-label={$t('startmenu.close')} onclick={() => notificationManager.remove(n.id)}><X size={14} /></button>
          </div>
        {:else}
          <p class="text-center text-white/60 text-sm py-6">{$t('altui.no_notifications')}</p>
        {/each}
        {#if notes.length}
          <button class="mx-auto flex items-center gap-1.5 px-4 h-9 rounded-full bg-white/10 text-xs" onclick={() => notificationManager.clearAll()}><Trash2 size={13} />{$t('altui.clear_all')}</button>
        {/if}
      </div>
    {/if}
    <div class="flex justify-center pb-2"><span class="w-10 h-1 rounded-full bg-white/30"></span></div>
  </div>
</div>
