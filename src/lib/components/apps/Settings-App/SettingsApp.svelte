<script lang="ts">
  import { onMount } from 'svelte';
  import {
    Image as ImageIcon, Palette, Wifi, Bluetooth, BatteryCharging, PanelTop,
    Globe, Moon, LayoutGrid, Monitor, Printer, Users, UserCircle, Info, Search, Shield,
  } from 'lucide-svelte';
  import { SystemBridge, type ThemeDefinition as SBThemeDefinition, type UserConfig } from '../../../utils/systemBridge';
  import { configStore } from '../../../utils/configStore';
  import TabButton from './TabButton.svelte';
  import type { SettingsTab } from './types';
  import { getAvailableModes, getCurrentResolution, getCurrentRefreshRate } from './display_helpers';

  import DisplaySection from './sections/DisplaySection.svelte';
  import PersonalizationSection from './sections/PersonalizationSection.svelte';
  import NetworkSection from './sections/NetworkSection.svelte';
  import PowerSection from './sections/PowerSection.svelte';
  import PanelSection from './sections/PanelSection.svelte';
  import LanguageSection from './sections/LanguageSection.svelte';
  import NightLightSection from './sections/NightLightSection.svelte';
  import AppsSection from './sections/AppsSection.svelte';
  import AccountsSection from './sections/AccountsSection.svelte';
  import AboutSection from './sections/AboutSection.svelte';
  import SecuritySection from './sections/SecuritySection.svelte';
  import DefaultAppsSection from './sections/DefaultAppsSection.svelte';
  import MonitorsSection from '../../settings/MonitorsSection.svelte';
  import PrintersSection from '../../settings/PrintersSection.svelte';
  import UsersSection from '../../settings/UsersSection.svelte';

  interface TabEntry { id: SettingsTab; label: string; icon: any; group: 'Appearance' | 'Network' | 'System' | 'Hardware' | 'Account'; }

  const TABS: TabEntry[] = [
    { id: 'display', label: 'Display', icon: ImageIcon, group: 'Appearance' },
    { id: 'personalization', label: 'Personalization', icon: Palette, group: 'Appearance' },
    { id: 'nightLight', label: 'Night Light', icon: Moon, group: 'Appearance' },
    { id: 'panel', label: 'Panel', icon: PanelTop, group: 'Appearance' },
    { id: 'language', label: 'Language', icon: Globe, group: 'Appearance' },
    { id: 'network', label: 'Network', icon: Wifi, group: 'Network' },
    { id: 'monitors', label: 'Monitors', icon: Monitor, group: 'Hardware' },
    { id: 'printers', label: 'Printers', icon: Printer, group: 'Hardware' },
    { id: 'power', label: 'Power', icon: BatteryCharging, group: 'Hardware' },
    { id: 'apps', label: 'Applications', icon: LayoutGrid, group: 'System' },
    { id: 'default_apps', label: 'Default Apps', icon: LayoutGrid, group: 'System' },
    { id: 'users', label: 'Users', icon: Users, group: 'System' },
    { id: 'security', label: 'Security', icon: Shield, group: 'Account' },
    { id: 'accounts', label: 'Accounts', icon: UserCircle, group: 'Account' },
    { id: 'about', label: 'About', icon: Info, group: 'Account' },
  ];
  const GROUP_ORDER: TabEntry['group'][] = ['Appearance', 'Network', 'Hardware', 'System', 'Account'];

  let activeTab: SettingsTab = 'display';
  let query = '';
  let config: UserConfig | null = null;
  let customThemeCount = 0;

  let wallpapers: string[] = [];
  let wallpaperPreviews = new Map<string, string>();
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

  async function loadWallpapers() {
    const list = await SystemBridge.getWallpapers();
    wallpapers = list;
    const previews = new Map<string, string>();
    await Promise.all(list.map(async (wp) => { const data = await SystemBridge.getWallpaperPreview(wp); if (data) previews.set(wp, data); }));
    wallpaperPreviews = previews;
  }

  $: resolutionList = modes.map((m) => m.resolution);
  $: rateList = modes.find((m) => m.resolution === resolution)?.rates ?? [60];
  $: filteredTabs = query.trim() ? TABS.filter((t) => t.label.toLowerCase().includes(query.trim().toLowerCase())) : TABS;
</script>

{#if !config}
  <div class="flex h-full bg-slate-900 text-white items-center justify-center"><p class="text-slate-400 text-sm">Loading settings…</p></div>
{:else}
  <div class="flex h-full bg-slate-900 text-white overflow-hidden">
    <div class="w-56 shrink-0 bg-slate-950/60 border-r border-white/5 flex flex-col">
      <div class="p-3 pb-2">
        <div class="relative">
          <Search size={13} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
          <input bind:value={query} placeholder="Find a setting…" class="w-full bg-slate-800 border border-white/10 rounded-lg pl-7 pr-2 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/50" />
        </div>
      </div>
      <nav class="flex-1 overflow-y-auto px-2 pb-3 space-y-3">
        {#each GROUP_ORDER as group (group)}
          {@const groupTabs = filteredTabs.filter((t) => t.group === group)}
          {#if groupTabs.length > 0}
            <div>
              <div class="px-2.5 mb-1 text-[10px] font-semibold uppercase tracking-wider text-slate-600">{group}</div>
              <div class="space-y-0.5">
                {#each groupTabs as tab (tab.id)}
                  <TabButton icon={tab.icon} label={tab.label} isActive={activeTab === tab.id} on:click={() => (activeTab = tab.id)} />
                {/each}
              </div>
            </div>
          {/if}
        {/each}
        {#if filteredTabs.length === 0}<p class="text-xs text-slate-500 px-2.5 pt-2">No settings match "{query}".</p>{/if}
      </nav>
      <div class="px-3 py-2 border-t border-white/5 text-[10px] text-slate-600">{customThemeCount} custom theme{customThemeCount === 1 ? '' : 's'} installed</div>
    </div>

    <div class="flex-1 overflow-y-auto p-6">
      {#if activeTab === 'display'}
        <DisplaySection {config} {onSave} {wallpapers} {wallpaperPreviews} onReloadWallpapers={loadWallpapers}
          bind:brightness bind:resolution bind:refreshRate {resolutionList} {rateList} />
      {:else if activeTab === 'personalization'}<PersonalizationSection />
      {:else if activeTab === 'nightLight'}<NightLightSection {config} {onSave} />
      {:else if activeTab === 'panel'}<PanelSection {config} {onSave} />
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
      {:else if activeTab === 'about'}<AboutSection />
      {/if}
    </div>
  </div>
{/if}
