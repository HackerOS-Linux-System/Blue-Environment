<script lang="ts">
  import { APPS } from '../constants';
  import { AppId } from '../types';
  import { SystemBridge } from '../utils/systemBridge';
  import {
    Search, Power, Grid, User, Box, RefreshCcw, HardDrive, Clock,
    ChevronRight, Loader2, Globe, Video, Image, FileText, Code,
    Gamepad2, Settings, LogOut, Moon, ExternalLink, LayoutGrid, X,
  } from 'lucide-svelte';
  import AppIconGlyph from './AppIconGlyph.svelte';
  import { createEventDispatcher } from 'svelte';

  interface SystemApp { id: string; name: string; comment: string; icon: string; exec: string; categories: string[]; desktop_file: string; is_external: boolean; }
  interface InternalApp { id: string; name: string; icon: any; categories: string[]; isInternal: true; }
  type AnyApp = SystemApp | InternalApp;

  export let isOpen = false;
  export let isFullScreen = false;
  export let appsEnabled: Record<string, boolean> = {};
  // See App.svelte's `startMenuZIndex` computation — always kept above
  // every currently-open window instead of a hardcoded value that a
  // long session's window zIndex counter could eventually climb past.
  export let zIndex = 9040;
  export let panelPosition: 'top' | 'bottom' = 'top';
  export let panelSize = 48;

  const dispatch = createEventDispatcher<{ openApp: { appId: string; isExternal?: boolean; exec?: string }; close: void; toggleFullScreen: void }>();

  const CATEGORY_ORDER = [
    { key: 'Recent', label: 'Recent', icon: Clock, keys: ['Recent'] },
    { key: 'Internet', label: 'Internet', icon: Globe, keys: ['Network', 'WebBrowser'] },
    { key: 'Multimedia', label: 'Multimedia', icon: Video, keys: ['AudioVideo', 'Audio', 'Video'] },
    { key: 'Graphics', label: 'Graphics', icon: Image, keys: ['Graphics'] },
    { key: 'Office', label: 'Office', icon: FileText, keys: ['Office'] },
    { key: 'Development', label: 'Development', icon: Code, keys: ['Development', 'IDE'] },
    { key: 'Games', label: 'Games', icon: Gamepad2, keys: ['Game'] },
    { key: 'System', label: 'System', icon: Settings, keys: ['System', 'Settings', 'Utility'] },
    { key: 'Other', label: 'Other', icon: Box, keys: [] },
  ];

  const INTERNAL_APP_CATEGORIES: Record<string, string[]> = {
    [AppId.TERMINAL]: ['System'], [AppId.BLUE_WEB]: ['Internet'],
    [AppId.EXPLORER]: ['System'], [AppId.CALCULATOR]: ['Utility'],
    [AppId.SYSTEM_MONITOR]: ['System'], [AppId.AI_ASSISTANT]: ['Utility'],
    [AppId.SETTINGS]: ['System'], [AppId.ABOUT]: ['System'],
    [AppId.NOTEPAD]: ['Office'], [AppId.BLUE_DOCS]: ['Office'], [AppId.BLUE_CODE]: ['Development'],
    [AppId.BLUE_IMAGES]: ['Graphics'], [AppId.BLUE_VIDEOS]: ['Multimedia'],
    [AppId.BLUE_MUSIC]: ['Multimedia'], [AppId.BLUE_SCREEN]: ['System'],
    [AppId.BLUE_ARCHIVE]: ['Utility'], [AppId.MAIL]: ['Internet'],
    [AppId.BLUE_SOFTWARE]: ['System'], [AppId.CAMERA]: ['Graphics'],
    [AppId.BLUE_PARTITION_MANAGER]: ['System'],
    [AppId.BLUE_PLAY]: ['Games'],
    [AppId.BLUE_TRANSLATE]: ['Utility'],
  };

  function getCategory(app: AnyApp): string {
    const cats = 'isInternal' in app ? INTERNAL_APP_CATEGORIES[app.id] || ['Other'] : app.categories;
    for (const cat of CATEGORY_ORDER) if (cat.keys.some((k) => cats.includes(k))) return cat.key;
    return 'Other';
  }

  const APP_ENABLED_KEYS: Record<string, string> = {
    ai_assistant: 'blueAI', blue_code: 'blueCode', blue_software: 'blueSoftware', mail: 'mail',
    calculator: 'calculator', notepad: 'notepad', blue_docs: 'blue_docs', system_monitor: 'systemMonitor',
    explorer: 'explorer', terminal: 'terminal', blue_web: 'blueWeb', camera: 'camera',
  };

  // `searchInput` updates immediately (so the text field itself feels
  // instant); the actual filtering below reads the debounced
  // `searchTerm` instead, so fast typing doesn't re-run
  // filter/sort/group over a potentially large `systemApps` list (every
  // installed .desktop app) on every single keystroke.
  let searchInput = '';
  let searchTerm = '';
  let searchDebounceHandle: ReturnType<typeof setTimeout> | undefined;
  $: {
    clearTimeout(searchDebounceHandle);
    const next = searchInput;
    searchDebounceHandle = setTimeout(() => (searchTerm = next), 90);
  }

  let systemApps: SystemApp[] = [];
  let recentApps: string[] = [];
  let loading = false;
  let activeCategory = 'Recent';
  let showPowerMenu = false;

  $: if (isOpen) loadApps();

  function loadApps() {
    loading = true;
    Promise.all([SystemBridge.getSystemApps(false), SystemBridge.getRecentApps()]).then(([apps, recent]) => {
      systemApps = apps as SystemApp[];
      recentApps = recent;
      loading = false;
    });
  }

  $: internalApps = Object.values(APPS)
    .filter((app) => {
      if (app.isExternal || !app.component) return false;
      // Every internal app is now individually toggleable in Settings →
      // Applications, keyed directly by its AppId (`app.id`) — see
      // AppsSection.svelte, which used to only expose a fixed 12-app
      // subset. `APP_ENABLED_KEYS` is kept as a fallback purely so
      // configs saved under the old key names (e.g. `blueAI` instead of
      // `ai_assistant`) still work.
      const id = app.id as string;
      const legacyKey = APP_ENABLED_KEYS[id];
      if (appsEnabled[id] === false) return false;
      if (legacyKey && appsEnabled[legacyKey] === false) return false;
      return true;
    })
    .map(
      (app): InternalApp => ({
        id: app.id as string,
        name: app.title,
        icon: app.icon,
        categories: INTERNAL_APP_CATEGORIES[app.id as string] || ['Other'],
        isInternal: true,
      })
    );

  $: allApps = (() => {
    const term = searchTerm.toLowerCase().trim();
    const combined: AnyApp[] = [...internalApps, ...systemApps];
    if (!term) return combined;
    return combined.filter(
      (app) => app.name.toLowerCase().includes(term) || (!('isInternal' in app) && app.comment.toLowerCase().includes(term))
    );
  })();

  $: groupedApps = (() => {
    const groups: Record<string, AnyApp[]> = {};
    // Only internal Blue apps get grouped into the fixed categories
    // (System/Internet/Multimedia/...). External .desktop apps already
    // have their own dedicated "Installed apps" view (see
    // `externalAppCount`/`currentTiles` below) — showing them here too
    // would just duplicate every installed app under "System" et al.
    for (const app of internalApps) {
      const cat = getCategory(app);
      if (!groups[cat]) groups[cat] = [];
      groups[cat].push(app);
    }
    for (const cat in groups) groups[cat].sort((a, b) => a.name.localeCompare(b.name));
    if (!searchTerm && recentApps.length > 0) {
      const recentList = recentApps.map((id) => allApps.find((a) => a.id === id)).filter(Boolean) as AnyApp[];
      if (recentList.length) groups['Recent'] = recentList.slice(0, 8);
    }
    return groups;
  })();

  $: visibleCategories = CATEGORY_ORDER.filter((cat) => groupedApps[cat.key]?.length > 0);

  $: externalAppCount = systemApps.length;

  // Powers the fullscreen grid: search always shows every match; otherwise
  // "All Apps" and "Installed apps" (external/system .desktop apps) are
  // synthetic views layered on top of the per-category groups above.
  $: currentTiles = (() => {
    if (searchTerm) return allApps;
    if (activeCategory === 'All') return [...allApps].sort((a, b) => a.name.localeCompare(b.name));
    if (activeCategory === 'External') return [...systemApps].sort((a, b) => a.name.localeCompare(b.name));
    return groupedApps[activeCategory] || [];
  })();

  function handleLaunch(app: AnyApp) {
    if ('isInternal' in app) dispatch('openApp', { appId: app.id, isExternal: false });
    else dispatch('openApp', { appId: app.id, isExternal: app.is_external, exec: app.exec });
    SystemBridge.recordAppLaunch(app.id);
    dispatch('close');
  }

  async function handleRefresh() {
    loading = true;
    systemApps = (await SystemBridge.getSystemApps(true)) as SystemApp[];
    loading = false;
  }

  const POWER_ITEMS = [
    { action: 'shutdown', icon: Power, label: 'Shut Down', cls: 'hover:bg-red-500/20 hover:text-red-400' },
    { action: 'reboot', icon: RefreshCcw, label: 'Restart', cls: 'hover:bg-white/10' },
    { action: 'suspend', icon: Moon, label: 'Suspend', cls: 'hover:bg-white/10' },
    { action: 'hibernate', icon: HardDrive, label: 'Hibernate', cls: 'hover:bg-white/10' },
  ];
</script>

{#if isOpen && isFullScreen}
  <div class="absolute inset-0 bg-slate-900/97 backdrop-blur-xl flex" style="z-index:{zIndex};" on:click={() => dispatch('close')}>
    <div class="w-60 border-r border-white/5 flex flex-col pt-16 px-3 gap-1 shrink-0" on:click|stopPropagation>
      <button on:click={() => { activeCategory = 'All'; searchInput = ''; searchTerm = ''; }}
        class="flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium text-left transition-all mb-1 {activeCategory === 'All' ? 'bg-blue-600 text-white shadow-lg' : 'text-slate-300 hover:bg-white/5 hover:text-white'}">
        <LayoutGrid size={18} />
        <span>All Apps</span>
        <span class="ml-auto text-xs opacity-50">{allApps.length}</span>
      </button>
      <div class="h-px bg-white/5 my-1.5" />
      {#each visibleCategories as cat (cat.key)}
        <button on:click={() => (activeCategory = cat.key)}
          class="flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium text-left transition-all {activeCategory === cat.key ? 'bg-blue-600 text-white shadow-lg' : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
          <svelte:component this={cat.icon} size={18} />
          <span class="truncate">{cat.label}</span>
          <span class="ml-auto text-xs opacity-50">{groupedApps[cat.key]?.length || 0}</span>
        </button>
      {/each}
      {#if externalAppCount > 0}
        <div class="h-px bg-white/5 my-1.5" />
        <button on:click={() => (activeCategory = 'External')}
          class="flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium text-left transition-all {activeCategory === 'External' ? 'bg-blue-600 text-white shadow-lg' : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
          <ExternalLink size={18} />
          <span class="truncate">Installed apps</span>
          <span class="ml-auto text-xs opacity-50">{externalAppCount}</span>
        </button>
      {/if}
    </div>
    <div class="flex-1 flex flex-col min-w-0" on:click|stopPropagation>
      <div class="px-8 pt-10 pb-6 flex items-center gap-4">
        <div class="relative flex-1 max-w-xl">
          <Search class="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" size={20} />
          <input type="text" autofocus placeholder="Search apps..."
            class="w-full bg-slate-800 border border-white/10 rounded-2xl py-3 pl-12 pr-4 text-white text-lg focus:outline-none focus:border-blue-500/60 transition-colors"
            bind:value={searchInput} />
        </div>
        <button on:click={() => dispatch('close')} title="Close"
          class="p-3 rounded-2xl bg-slate-800/60 border border-white/5 text-slate-400 hover:text-white hover:bg-white/10 transition-colors shrink-0">
          <X size={18} />
        </button>
      </div>
      <div class="flex-1 overflow-y-auto px-8 pb-8">
        {#if loading}
          <div class="flex items-center gap-2 text-slate-500 text-sm"><Loader2 size={16} class="animate-spin" /> Loading…</div>
        {:else}
          {#if !searchTerm}
            <div class="flex items-baseline gap-2 mb-5">
              <h3 class="text-xl font-semibold text-white">
                {activeCategory === 'All' ? 'All Apps' : activeCategory === 'External' ? 'Installed apps' : CATEGORY_ORDER.find((c) => c.key === activeCategory)?.label}
              </h3>
              <span class="text-sm text-slate-500">{currentTiles.length}</span>
            </div>
          {/if}
          {#if currentTiles.length === 0}
            <div class="flex flex-col items-center justify-center gap-3 text-slate-500 pt-20">
              <Search size={32} class="opacity-40" />
              <p class="text-sm">No apps found{#if searchTerm} for "{searchTerm}"{/if}</p>
            </div>
          {:else}
            <div class="grid grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3">
              {#each currentTiles as app (app.id)}
                {@const external = !('isInternal' in app)}
                <button on:click={() => handleLaunch(app)}
                  title={external ? app.comment || app.name : app.name}
                  class="relative flex flex-col items-center gap-2 p-3.5 rounded-2xl border border-transparent hover:border-white/10 hover:bg-white/[0.07] active:scale-[0.97] transition-all group text-center">
                  {#if external}
                    <span class="absolute top-2 right-2 w-4 h-4 rounded-full bg-slate-950/80 border border-white/10 flex items-center justify-center text-slate-500 group-hover:text-blue-400 transition-colors" title="Installed system app">
                      <ExternalLink size={9} />
                    </span>
                  {/if}
                  <div class="w-14 h-14 bg-slate-800/60 border border-white/5 rounded-2xl flex items-center justify-center group-hover:bg-blue-600/20 group-hover:border-blue-500/30 transition-colors overflow-hidden shadow-sm">
                    <AppIconGlyph icon={app.icon} name={app.name} size={36} />
                  </div>
                  <div class="w-full">
                    <span class="block text-xs text-slate-300 group-hover:text-white text-center leading-tight line-clamp-2">{app.name}</span>
                    {#if external && app.comment}
                      <span class="block text-[10px] text-slate-500 text-center leading-tight line-clamp-1 mt-0.5">{app.comment}</span>
                    {/if}
                  </div>
                </button>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{:else if isOpen}
  <div class="absolute left-3 w-80 bg-slate-900/97 backdrop-blur-md border border-white/10 rounded-2xl shadow-2xl overflow-visible flex flex-col" style="z-index:{zIndex}; {panelPosition === 'top' ? `top:${panelSize + 6}px;` : `bottom:${panelSize + 6}px;`}" on:click|stopPropagation>
    <div class="p-4 flex items-center justify-between border-b border-white/5">
      <div class="flex items-center gap-3">
        <div class="w-9 h-9 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white font-bold shadow-lg">
          <User size={18} />
        </div>
        <div>
          <div class="text-sm font-bold text-white leading-none">{SystemBridge.getUsername()}</div>
          <div class="text-[10px] text-blue-300 mt-0.5">Blue Environment</div>
        </div>
      </div>
      <div class="flex gap-1">
        <button on:click={handleRefresh} class="p-1.5 hover:bg-white/10 rounded-full text-slate-400 hover:text-white transition-colors">
          <RefreshCcw size={14} class={loading ? 'animate-spin' : ''} />
        </button>
        <button on:click={() => dispatch('toggleFullScreen')} class="p-1.5 hover:bg-white/10 rounded-full text-slate-400 hover:text-white transition-colors">
          <Grid size={16} />
        </button>
      </div>
    </div>

    <div class="p-2 grid grid-cols-5 gap-1 border-b border-white/5">
      {#each internalApps.slice(0, 5) as app (app.id)}
        <button on:click={() => handleLaunch(app)} class="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-white/10 transition-colors group" title={app.name}>
          <div class="w-9 h-9 bg-slate-800 rounded-xl flex items-center justify-center border border-white/5 group-hover:bg-blue-600/30 transition-colors">
            <AppIconGlyph icon={app.icon} name={app.name} size={18} />
          </div>
          <span class="text-[9px] text-slate-400 group-hover:text-white text-center leading-none truncate w-full">{app.name}</span>
        </button>
      {/each}
    </div>

    <div class="px-3 pt-3">
      <div class="relative">
        <Search size={13} class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
        <input type="text" placeholder="Search apps..."
          class="w-full bg-slate-800 border border-white/10 rounded-xl py-2 pl-8 pr-3 text-sm text-white focus:outline-none focus:border-blue-500/50 placeholder-slate-500"
          bind:value={searchInput} />
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-2 space-y-0.5 max-h-72 mt-2">
      {#if loading}
        <div class="flex items-center gap-2 px-3 py-2 text-slate-500 text-xs"><Loader2 size={12} class="animate-spin" /> Loading…</div>
      {:else}
        {#if !searchTerm && recentApps.length > 0}
          <div class="px-2 py-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider flex items-center gap-1"><Clock size={10} /> Recent</div>
          {#each recentApps.slice(0, 3) as id (id)}
            {@const app = allApps.find((a) => a.id === id)}
            {#if app}
              <button on:click={() => handleLaunch(app)} class="w-full flex items-center gap-3 px-2 py-1.5 rounded-xl hover:bg-white/5 transition-colors group">
                <div class="w-8 h-8 bg-slate-800 rounded-xl flex items-center justify-center border border-white/5 shrink-0 overflow-hidden">
                  <AppIconGlyph icon={app.icon} name={app.name} size={20} />
                </div>
                <div class="flex-1 min-w-0 text-left">
                  <div class="text-sm text-slate-200 group-hover:text-white font-medium truncate">{app.name}</div>
                  {#if !('isInternal' in app) && app.comment}
                    <div class="text-[10px] text-slate-500 truncate">{app.comment}</div>
                  {/if}
                </div>
              </button>
            {/if}
          {/each}
          <div class="h-px bg-white/5 my-1" />
        {/if}
        {#each (searchTerm ? allApps : allApps.filter((a) => !recentApps.includes(a.id))).slice(0, 12) as app (app.id)}
          <button on:click={() => handleLaunch(app)} class="w-full flex items-center gap-3 px-2 py-1.5 rounded-xl hover:bg-white/5 transition-colors group">
            <div class="w-8 h-8 bg-slate-800 rounded-xl flex items-center justify-center border border-white/5 shrink-0 overflow-hidden">
              <AppIconGlyph icon={app.icon} name={app.name} size={20} />
            </div>
            <div class="flex-1 min-w-0 text-left">
              <div class="text-sm text-slate-200 group-hover:text-white font-medium truncate">{app.name}</div>
              {#if !('isInternal' in app) && app.comment}
                <div class="text-[10px] text-slate-500 truncate">{app.comment}</div>
              {/if}
            </div>
          </button>
        {/each}
        {#if allApps.length === 0 && searchTerm}
          <div class="px-3 py-4 text-center text-slate-500 text-xs">No results</div>
        {/if}
      {/if}
    </div>

    <div class="mt-auto p-3 border-t border-white/5 bg-slate-950/50 rounded-b-2xl flex items-center justify-between relative">
      <button on:click={() => dispatch('toggleFullScreen')} class="text-xs text-slate-500 hover:text-white transition-colors flex items-center gap-1">
        All apps <ChevronRight size={12} />
      </button>
      <button on:click={() => (showPowerMenu = !showPowerMenu)} class="p-2 rounded-full bg-red-500/10 hover:bg-red-500 text-red-400 hover:text-white transition-all">
        <Power size={15} />
      </button>
      {#if showPowerMenu}
        <div class="absolute bottom-14 left-4 bg-slate-800 border border-white/10 rounded-xl shadow-2xl p-2 flex flex-col gap-1 w-48" style="z-index:{zIndex + 1};">
          {#each POWER_ITEMS as { action, icon, label, cls } (action)}
            <button on:click={() => SystemBridge.powerAction(action)} class="flex items-center gap-3 p-2 {cls} rounded-lg transition-colors text-left text-sm text-slate-200">
              <svelte:component this={icon} size={16} /> {label}
            </button>
          {/each}
          <div class="h-px bg-white/10 my-1" />
          <button on:click={() => SystemBridge.powerAction('logout')} class="flex items-center gap-3 p-2 hover:bg-white/10 rounded-lg transition-colors text-left text-sm text-slate-200">
            <LogOut size={16} /> Log Out
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}
