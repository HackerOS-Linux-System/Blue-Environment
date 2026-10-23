<script lang="ts">
  /**
   * Settings for the TV shell: the handful of things people actually change from
   * the sofa — language, Wi-Fi, Bluetooth, volume, brightness, theme and
   * HDMI-CEC — as big, focusable controls that work with a remote or game pad.
   * Everything else is one press away ("All settings" opens the regular
   * Settings app, whose buttons are reachable with the same D-pad navigation).
   *
   * Stands in for the desktop Settings app in the TV shell
   * (`TvShell.svelte` → `AppViewport` `overrides`).
   */
  import { onMount } from 'svelte';
  import { Wifi, Bluetooth, Volume2, Sun, Moon, Tv, Languages, ArrowRight } from 'lucide-svelte';
  import { language, t, setLanguage, SUPPORTED_LANGUAGES } from '../lib/stores/language';
  import { SystemBridge } from '../lib/utils/systemBridge';
  import { configStore } from '../lib/utils/configStore';
  import SettingsApp from '../lib/components/apps/Settings-App/SettingsApp.svelte';

  interface Props {
    windowId?: string;
  }

  let { windowId = '' }: Props = $props();

  let advanced = $state(false);
  let wifi = $state(false); let bt = $state(false); let volume = $state(50); let brightness = $state(-1);
  let theme = $state('dark');
  let cec = $state<{ adapters: string[]; tool_installed: boolean } | null>(null);
  let cecMessage = $state(''); let cecError = $state(false); let cecBusy = $state(false);

  onMount(async () => {
    const [stats, w, b, cfg, c] = await Promise.all([
      SystemBridge.getSystemStats().catch(() => null),
      SystemBridge.getWifiRadioEnabled().catch(() => false),
      SystemBridge.getBluetoothPowered().catch(() => false),
      configStore.init().catch(() => null),
      SystemBridge.invokeCommand<{ adapters: string[]; tool_installed: boolean }>('tv_cec_status').catch(() => null),
    ]);
    if (stats) { volume = stats.volume; brightness = stats.brightness; }
    wifi = w; bt = b; cec = c;
    if (cfg?.theme) theme = cfg.theme;
    (document.querySelector(`[data-app-viewport="${windowId}"] [data-tv-focusable]`) as HTMLElement | null)?.focus();
  });

  async function toggleWifi() { const n = !wifi; wifi = n; await SystemBridge.toggleWifi(n).catch(() => { wifi = !n; }); }
  async function toggleBt() { const n = !bt; bt = n; await SystemBridge.setBluetoothPowered(n).catch(() => { bt = !n; }); }
  const setVolume = (e: Event) => { volume = Number((e.currentTarget as HTMLInputElement).value); SystemBridge.setVolume(volume).catch(() => {}); };
  let timer: ReturnType<typeof setTimeout>;
  const setBrightness = (e: Event) => {
    brightness = Number((e.currentTarget as HTMLInputElement).value);
    clearTimeout(timer); timer = setTimeout(() => SystemBridge.setBrightness(brightness).catch(() => {}), 80);
  };
  async function toggleTheme() {
    theme = theme === 'light-glass' ? 'dark' : 'light-glass';
    await configStore.save({ theme } as any);
  }
  async function enableCec() {
    if (!cec?.adapters.length) return;
    cecBusy = true; cecMessage = ''; cecError = false;
    try { await SystemBridge.invokeCommand('tv_cec_enable', { device: cec.adapters[0] }); cecMessage = $t('tvset.cec_ok'); }
    catch (e) { cecError = true; cecMessage = String(e); }
    cecBusy = false;
  }
</script>

{#if advanced}
  <div class="h-full flex flex-col">
    <div class="shrink-0 p-4">
      <button data-tv-focusable class="tv-tile px-6 py-3 rounded-2xl bg-white/10 text-lg" onclick={() => (advanced = false)}>← {$t('altui.back')}</button>
    </div>
    <div class="flex-1 min-h-0"><SettingsApp /></div>
  </div>
{:else}
  <div class="h-full overflow-y-auto px-16 py-10 text-white">
    <h1 class="text-4xl font-semibold mb-8">{$t('altui.settings')}</h1>

    <section class="mb-8">
      <h2 class="flex items-center gap-3 text-xl text-white/70 mb-3"><Languages size={24} /> {$t('settings.language')}</h2>
      <div class="grid grid-cols-4 gap-4 p-2 -m-2">
        {#each SUPPORTED_LANGUAGES as lang (lang.code)}
          <button data-tv-focusable data-tv-key={`lang:${lang.code}`}
            class="tv-tile h-16 rounded-2xl text-lg {$language === lang.code ? 'bg-blue-600' : 'bg-white/10'}" onclick={() => setLanguage(lang.code)}>{lang.nativeName}</button>
        {/each}
      </div>
    </section>

    <section class="mb-8 grid grid-cols-3 gap-4 p-2 -m-2">
      <button data-tv-focusable class="tv-tile h-24 rounded-2xl flex items-center justify-center gap-3 text-xl {wifi ? 'bg-blue-600' : 'bg-white/10'}" onclick={toggleWifi}><Wifi size={28} />{$t('altui.wifi')}</button>
      <button data-tv-focusable class="tv-tile h-24 rounded-2xl flex items-center justify-center gap-3 text-xl {bt ? 'bg-blue-600' : 'bg-white/10'}" onclick={toggleBt}><Bluetooth size={28} />{$t('altui.bluetooth')}</button>
      <button data-tv-focusable class="tv-tile h-24 rounded-2xl flex items-center justify-center gap-3 text-xl bg-white/10" onclick={toggleTheme}>
        {#if theme === 'light-glass'}<Sun size={28} />{:else}<Moon size={28} />{/if}{$t('tvset.theme')}
      </button>
    </section>

    <section class="mb-8 space-y-4">
      <label class="flex items-center gap-5 text-xl"><Volume2 size={30} class="shrink-0" /><span class="w-40 shrink-0">{$t('osd.volume')}</span>
        <input data-tv-focusable type="range" min="0" max="100" step="5" value={volume} oninput={setVolume} class="flex-1 h-12" /></label>
      {#if brightness >= 0}
        <label class="flex items-center gap-5 text-xl"><Sun size={30} class="shrink-0" /><span class="w-40 shrink-0">{$t('osd.brightness')}</span>
          <input data-tv-focusable type="range" min="1" max="100" step="5" value={brightness} oninput={setBrightness} class="flex-1 h-12" /></label>
      {/if}
    </section>

    <section class="mb-8">
      <h2 class="flex items-center gap-3 text-xl text-white/70 mb-3"><Tv size={24} /> HDMI-CEC</h2>
      {#if !cec || cec.adapters.length === 0}
        <p class="text-lg text-white/60">{$t('tvset.cec_none')}</p>
      {:else if !cec.tool_installed}
        <p class="text-lg text-amber-300">{$t('tvset.cec_tool')}</p>
      {:else}
        <p class="text-lg text-white/60 mb-3">{$t('tvset.cec_desc')}</p>
        <button data-tv-focusable class="tv-tile px-8 h-16 rounded-2xl bg-white/10 text-xl disabled:opacity-40" disabled={cecBusy} onclick={enableCec}>{$t('tvset.cec_enable')}</button>
        {#if cecMessage}<p class="mt-3 text-lg {cecError ? 'text-red-400' : 'text-emerald-400'}">{cecMessage}</p>{/if}
      {/if}
    </section>

    <button data-tv-focusable class="tv-tile px-8 h-16 rounded-2xl bg-white/10 text-xl flex items-center gap-3" onclick={() => (advanced = true)}>{$t('tvset.all_settings')} <ArrowRight size={24} /></button>
  </div>
{/if}
