<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Image as ImageIcon, Wifi, Bluetooth, BatteryCharging, PanelTop,
    Globe, Moon, LayoutGrid, Monitor, Printer, Users, UserCircle, Info, Search, Shield, ShieldCheck,
    Sparkles, Puzzle, Layers,
  } from 'lucide-svelte';
  import { SystemBridge, type ThemeDefinition as SBThemeDefinition, type UserConfig } from '../../../utils/systemBridge';
  import { configStore } from '../../../utils/configStore';
  import { t } from '../../../stores/language';
  import TabButton from './TabButton.svelte';
  import type { SettingsTab } from './types';
  import { getAvailableModes, getCurrentResolution, getCurrentRefreshRate } from './display_helpers';

  import DisplaySection from './sections/DisplaySection.svelte';
  import PersonalizationSection from './sections/PersonalizationSection.svelte';
  import ThemesSection from './sections/ThemesSection.svelte';
  import PluginsSection from './sections/PluginsSection.svelte';
  import NetworkSection from './sections/NetworkSection.svelte';
  import PowerSection from './sections/PowerSection.svelte';
  import PanelSection from './sections/PanelSection.svelte';
  import WeatherSection from './sections/WeatherSection.svelte';
  import LanguageSection from './sections/LanguageSection.svelte';
  import NightLightSection from './sections/NightLightSection.svelte';
  import AppsSection from './sections/AppsSection.svelte';
  import AccountsSection from './sections/AccountsSection.svelte';
  import AboutSection from './sections/AboutSection.svelte';
  import SecuritySection from './sections/SecuritySection.svelte';
  import ParentalControlsSection from './sections/ParentalControlsSection.svelte';
  import DefaultAppsSection from './sections/DefaultAppsSection.svelte';
  import MonitorsSection from '../../settings/MonitorsSection.svelte';
  import PrintersSection from '../../settings/PrintersSection.svelte';
  import UsersSection from '../../settings/UsersSection.svelte';

  interface TabEntry { id: SettingsTab; labelKey: string; icon: any; group: 'Appearance' | 'Network' | 'System' | 'Hardware' | 'Account'; }

  const TABS: TabEntry[] = [
    { id: 'display', labelKey: 'settings.tab.display', icon: ImageIcon, group: 'Appearance' },
    { id: 'personalization', labelKey: 'settings.tab.icons', icon: Layers, group: 'Appearance' },
    { id: 'themes', labelKey: 'settings.tab.themes', icon: Sparkles, group: 'Appearance' },
    { id: 'plugins', labelKey: 'settings.tab.plugins', icon: Puzzle, group: 'System' },
    { id: 'nightLight', labelKey: 'settings.tab.night_light', icon: Moon, group: 'Appearance' },
    { id: 'panel', labelKey: 'settings.tab.panel', icon: PanelTop, group: 'Appearance' },
    { id: 'weather', labelKey: 'settings.tab.weather', icon: Globe, group: 'Appearance' },
    { id: 'language', labelKey: 'settings.tab.language', icon: Globe, group: 'Appearance' },
    { id: 'network', labelKey: 'settings.tab.network', icon: Wifi, group: 'Network' },
    { id: 'monitors', labelKey: 'settings.tab.monitors', icon: Monitor, group: 'Hardware' },
    { id: 'printers', labelKey: 'settings.tab.printers', icon: Printer, group: 'Hardware' },
    { id: 'power', labelKey: 'settings.tab.power', icon: BatteryCharging, group: 'Hardware' },
    { id: 'apps', labelKey: 'settings.tab.applications', icon: LayoutGrid, group: 'System' },
    { id: 'default_apps', labelKey: 'settings.tab.default_apps', icon: LayoutGrid, group: 'System' },
    { id: 'users', labelKey: 'settings.tab.users', icon: Users, group: 'System' },
    { id: 'security', labelKey: 'settings.tab.security', icon: Shield, group: 'Account' },
    { id: 'parental_controls', labelKey: 'settings.tab.parental_controls', icon: ShieldCheck, group: 'Account' },
    { id: 'accounts', labelKey: 'settings.tab.accounts', icon: UserCircle, group: 'Account' },
    { id: 'about', labelKey: 'settings.tab.about', icon: Info, group: 'Account' },
  ];
  const GROUP_ORDER: TabEntry['group'][] = ['Appearance', 'Network', 'Hardware', 'System', 'Account'];
  const GROUP_LABEL_KEYS: Record<TabEntry['group'], string> = {
    Appearance: 'settings.group.appearance',
    Network: 'settings.group.network',
    Hardware: 'settings.group.hardware',
    System: 'settings.group.system',
    Account: 'settings.group.account',
  };

  let activeTab: SettingsTab = 'display';
  let query = '';
  let config: UserConfig | null = null;
  let customThemeCount = 0;

  let wallpapers: string[] = [];
  let wallpaperPreviews = new Map<string, string>();
  let wallpaperPreviewsLoading = new Set<string>();
  let brightness = 80;
  let resolution = '1920x1080';
  let refreshRate = 60;
  let modes: { resolution: string; rates: number[] }[] = [{ resolution: '1920x1080', rates: [60] }];

  onMount(() => {
    configStore.init().then((c) => (config = c));
    const unsub = configStore.subscribe((c) => (config = c));

    SystemBridge.getCustomThemes().then((t: SBThemeDefinition[]) => (customThemeCount = t.length));

    loadWallpapers();

    (async () => {
      const [avail, curRes, curRate] = await Promise.all([getAvailableModes(), getCurrentResolution(), getCurrentRefreshRate()]);
      modes = avail;
      resolution = curRes;
      refreshRate = curRate;
    })();

    return unsub;
  });

  async function onSave(patch: Partial<UserConfig>) { await configStore.save(patch); }

  /// Loads the wallpaper grid's thumbnails with **bounded** concurrency
  /// (a handful at a time) instead of firing every preview request at
  /// once via an unbounded `Promise.all` — the backend now generates
  /// real small thumbnails with disk caching (see
  /// `get_wallpaper_preview` in commands/display.rs), so this isn't
  /// covering for a slow backend anymore, but flooding dozens of
  /// concurrent Tauri IPC calls the instant Settings opens is still
  /// unnecessary load on the webview's IPC channel for no benefit — a
  /// person can only look at a few tiles at once anyway. Each tile
  /// shows its own spinner (`wallpaperPreviewsLoading`) while its
  /// specific preview is in flight, so a slow one (an uncached, huge
  /// original) can't make the *whole* grid look stuck — see
  /// DisplaySection.svelte's `{#if wallpaperPreviewsLoading.has(wp)}`.
  async function loadWallpapers() {
    const list = await SystemBridge.getWallpapers();
    wallpapers = list;

    const previews = new Map<string, string>();
    const CONCURRENCY = 4;
    let cursor = 0;

    async function worker() {
      while (cursor < list.length) {
        const wp = list[cursor++];
        wallpaperPreviewsLoading = new Set(wallpaperPreviewsLoading).add(wp);
        try {
          const data = await SystemBridge.getWallpaperPreview(wp);
          if (data) {
            previews.set(wp, data);
            wallpaperPreviews = new Map(previews); // update incrementally so tiles fill in as they arrive, not all-at-once at the end
          }
        } finally {
          const next = new Set(wallpaperPreviewsLoading);
          next.delete(wp);
          wallpaperPreviewsLoading = next;
        }
      }
    }

    await Promise.all(Array.from({ length: Math.min(CONCURRENCY, list.length) }, worker));
  }

  $: resolutionList = modes.map((m) => m.resolution);
  $: rateList = modes.find((m) => m.resolution === resolution)?.rates ?? [60];
  $: filteredTabs = query.trim() ? TABS.filter((tab) => $t(tab.labelKey).toLowerCase().includes(query.trim().toLowerCase())) : TABS;
</script>

{#if !config}
  <div class="flex h-full bg-slate-900 text-white items-center justify-center"><p class="text-slate-400 text-sm">{$t('settings.loading')}</p></div>
{:else}
  <div class="flex h-full bg-slate-900 text-white overflow-hidden">
    <div class="w-56 shrink-0 bg-slate-950/60 border-r border-white/5 flex flex-col">
      <div class="p-3 pb-2">
        <div class="relative">
          <Search size={13} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
          <input bind:value={query} placeholder={$t('settings.search_placeholder')} class="w-full bg-slate-800 border border-white/10 rounded-lg pl-7 pr-2 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/50" />
        </div>
      </div>
      <nav class="flex-1 overflow-y-auto px-2 pb-3 space-y-3">
        {#each GROUP_ORDER as group (group)}
          {@const groupTabs = filteredTabs.filter((tab) => tab.group === group)}
          {#if groupTabs.length > 0}
            <div>
              <div class="px-2.5 mb-1 text-[10px] font-semibold uppercase tracking-wider text-slate-600">{$t(GROUP_LABEL_KEYS[group])}</div>
              <div class="space-y-0.5">
                {#each groupTabs as tab (tab.id)}
                  <TabButton icon={tab.icon} label={$t(tab.labelKey)} isActive={activeTab === tab.id} on:click={() => (activeTab = tab.id)} />
                {/each}
              </div>
            </div>
          {/if}
        {/each}
        {#if filteredTabs.length === 0}<p class="text-xs text-slate-500 px-2.5 pt-2">{$t('settings.no_match').replace('{query}', query)}</p>{/if}
      </nav>
      <div class="px-3 py-2 border-t border-white/5 text-[10px] text-slate-600">{customThemeCount} custom theme{customThemeCount === 1 ? '' : 's'} installed</div>
    </div>

    <div class="flex-1 overflow-y-auto p-6">
      {#if activeTab === 'display'}
        <DisplaySection {config} {onSave} {wallpapers} {wallpaperPreviews} {wallpaperPreviewsLoading} onReloadWallpapers={loadWallpapers}
          bind:brightness bind:resolution bind:refreshRate {resolutionList} {rateList} />
      {:else if activeTab === 'personalization'}<PersonalizationSection />
      {:else if activeTab === 'themes'}<ThemesSection {config} {onSave} />
      {:else if activeTab === 'plugins'}<PluginsSection {config} {onSave} />
      {:else if activeTab === 'nightLight'}<NightLightSection {config} {onSave} />
      {:else if activeTab === 'panel'}<PanelSection {config} {onSave} />
      {:else if activeTab === 'weather'}<WeatherSection {config} {onSave} />
      {:else if activeTab === 'language'}<LanguageSection />
      {:else if activeTab === 'network'}<NetworkSection />
      {:else if activeTab === 'monitors'}<MonitorsSection />
      {:else if activeTab === 'printers'}<PrintersSection />
      {:else if activeTab === 'power'}<PowerSection />
      {:else if activeTab === 'apps'}<AppsSection {config} {onSave} />
      {:else if activeTab === 'default_apps'}<DefaultAppsSection />
      {:else if activeTab === 'users'}<UsersSection />
      {:else if activeTab === 'accounts'}<AccountsSection {config} {onSave} />
      {:else if activeTab === 'security'}<SecuritySection />
      {:else if activeTab === 'parental_controls'}<ParentalControlsSection />
      {:else if activeTab === 'about'}<AboutSection />
      {/if}
    </div>
  </div>
{/if}
