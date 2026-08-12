<script lang="ts">
  import { PanelTop, PanelBottom, Plus, X, GripVertical } from 'lucide-svelte';
  import type { UserConfig } from '../../../../types';
  import { AppId } from '../../../../types';
  import { APPS } from '../../../../constants';
  import { t } from '../../../../stores/language';
  import { Layers } from 'lucide-svelte';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  const DEFAULT_PINNED = [AppId.TERMINAL, AppId.EXPLORER, AppId.SYSTEM_MONITOR, AppId.SETTINGS];

  let pinned: string[] = [];
  $: pinned = (config.pinnedApps && config.pinnedApps.length > 0) ? config.pinnedApps : DEFAULT_PINNED;

  // Every real internal app, so the "add to panel" picker can never go
  // stale the way the old hardcoded lists elsewhere in this app did.
  $: pinnable = Object.values(APPS)
    .filter((app) => !app.isExternal && app.component)
    .sort((a, b) => a.title.localeCompare(b.title));

  $: availableToAdd = pinnable.filter((app) => !pinned.includes(app.id as string));

  function addPinned(id: string) {
    onSave({ pinnedApps: [...pinned, id] });
  }
  function removePinned(id: string) {
    onSave({ pinnedApps: pinned.filter((p) => p !== id) });
  }
  function movePinned(index: number, dir: -1 | 1) {
    const next = [...pinned];
    const target = index + dir;
    if (target < 0 || target >= next.length) return;
    [next[index], next[target]] = [next[target], next[index]];
    onSave({ pinnedApps: next });
  }

  function getApp(id: string) {
    return APPS[id as AppId];
  }
  let addPickerOpen = false;
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.panel.title')}</h2>

  <!-- Visibility + position -->
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-5">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3 text-slate-300">
        <PanelTop size={16} class="text-blue-400" />
        <div>
          <div class="text-sm font-medium text-white">{$t('settings.panel.show_panel')}</div>
          <div class="text-xs text-slate-500">{$t('settings.panel.show_panel_desc')}</div>
        </div>
      </div>
      <button role="switch" aria-checked={config.panelEnabled ?? true}
        on:click={() => onSave({ panelEnabled: !(config.panelEnabled ?? true) })}
        class="relative w-11 h-6 rounded-full transition-colors shrink-0 {config.panelEnabled ?? true ? 'bg-blue-600' : 'bg-slate-700'}">
        <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {config.panelEnabled ?? true ? 'translate-x-5' : ''}" />
      </button>
    </div>

    {#if config.panelEnabled ?? true}
      <div>
        <div class="text-sm font-medium text-slate-400 mb-2">{$t('settings.panel.position')}</div>
        <div class="flex gap-2">
          <button on:click={() => onSave({ panelPosition: 'top' })}
            class="flex-1 flex items-center justify-center gap-2 py-2.5 rounded-xl text-sm font-medium border transition-colors {(config.panelPosition ?? 'top') === 'top' ? 'bg-blue-600 border-blue-500 text-white' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
            <PanelTop size={15} /> {$t('settings.panel.top')}
          </button>
          <button on:click={() => onSave({ panelPosition: 'bottom' })}
            class="flex-1 flex items-center justify-center gap-2 py-2.5 rounded-xl text-sm font-medium border transition-colors {config.panelPosition === 'bottom' ? 'bg-blue-600 border-blue-500 text-white' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
            <PanelBottom size={15} /> {$t('settings.panel.bottom')}
          </button>
        </div>
      </div>

      <div>
        <label class="block text-sm font-medium text-slate-400 mb-1">{$t('settings.panel.height').replace('{px}', String(config.panelSize ?? 48))}</label>
        <input type="range" min="36" max="64" step="1" value={config.panelSize ?? 48}
          on:change={(e) => onSave({ panelSize: parseInt(e.currentTarget.value, 10) })}
          class="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
      </div>
      <div>
        <label class="block text-sm font-medium text-slate-400 mb-1">{$t('settings.panel.opacity').replace('{pct}', String(Math.round((config.panelOpacity ?? 0.95) * 100)))}</label>
        <input type="range" min="0.5" max="1" step="0.05" value={config.panelOpacity ?? 0.95}
          on:change={(e) => onSave({ panelOpacity: parseFloat(e.currentTarget.value) })}
          class="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
      </div>
      <p class="text-xs text-slate-500">{$t('settings.panel.snap_hint')}</p>
    {/if}
  </div>

  {#if config.panelEnabled ?? true}
  <!-- Pinned apps -->
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div>
      <h3 class="text-sm font-semibold text-white">{$t('settings.panel.pinned_apps')}</h3>
      <p class="text-xs text-slate-500 mt-0.5">{$t('settings.panel.pinned_apps_desc')}</p>
    </div>
    <div class="space-y-1.5">
      {#each pinned as id, i (id)}
        {@const app = getApp(id)}
        {#if app}
          <div class="flex items-center gap-2 bg-slate-900/50 border border-white/5 rounded-xl px-3 py-2">
            <GripVertical size={14} class="text-slate-600 shrink-0" />
            <svelte:component this={app.icon} size={16} class="text-slate-400 shrink-0" />
            <span class="text-sm text-slate-200 flex-1 truncate">{app.title}</span>
            <button on:click={() => movePinned(i, -1)} disabled={i === 0} class="text-slate-500 hover:text-white disabled:opacity-20 disabled:hover:text-slate-500 px-1">▲</button>
            <button on:click={() => movePinned(i, 1)} disabled={i === pinned.length - 1} class="text-slate-500 hover:text-white disabled:opacity-20 disabled:hover:text-slate-500 px-1">▼</button>
            <button on:click={() => removePinned(id)} class="text-slate-500 hover:text-red-400 p-1"><X size={14} /></button>
          </div>
        {/if}
      {/each}
      {#if pinned.length === 0}
        <p class="text-xs text-slate-500 text-center py-3">{$t('settings.panel.no_pinned')}</p>
      {/if}
    </div>

    <div class="relative">
      <button on:click={() => (addPickerOpen = !addPickerOpen)} disabled={availableToAdd.length === 0}
        class="w-full flex items-center justify-center gap-2 py-2 rounded-xl border border-dashed border-white/10 text-sm text-slate-400 hover:text-white hover:border-white/20 disabled:opacity-40 transition-colors">
        <Plus size={14} /> {$t('settings.panel.pin_an_app')}
      </button>
      {#if addPickerOpen}
        <div class="fixed inset-0 z-10" on:click={() => (addPickerOpen = false)} />
        <div class="absolute left-0 right-0 top-full mt-1 max-h-56 overflow-y-auto bg-slate-900 border border-white/10 rounded-xl shadow-2xl z-20 p-1">
          {#each availableToAdd as app (app.id)}
            {@const appId = String(app.id)}
            <button on:click={() => { addPinned(appId); addPickerOpen = false; }}
              class="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-slate-300 hover:bg-white/5 hover:text-white text-left">
              <svelte:component this={app.icon} size={15} class="text-slate-400" />
              {app.title}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Hover behavior -->
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <h3 class="text-sm font-semibold text-white">{$t('settings.panel.hover_behavior')}</h3>
    <div class="flex items-center justify-between">
      <div>
        <div class="text-sm text-slate-200">{$t('settings.panel.clipboard_hover')}</div>
        <div class="text-xs text-slate-500">{$t('settings.panel.clipboard_hover_desc')}</div>
      </div>
      <button role="switch" aria-checked={config.clipboardHoverPreviewEnabled ?? true}
        on:click={() => onSave({ clipboardHoverPreviewEnabled: !(config.clipboardHoverPreviewEnabled ?? true) })}
        class="relative w-11 h-6 rounded-full transition-colors shrink-0 {config.clipboardHoverPreviewEnabled ?? true ? 'bg-blue-600' : 'bg-slate-700'}">
        <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {config.clipboardHoverPreviewEnabled ?? true ? 'translate-x-5' : ''}" />
      </button>
    </div>
    <div class="flex items-center justify-between">
      <div>
        <div class="text-sm text-slate-200">{$t('settings.panel.network_hover')}</div>
        <div class="text-xs text-slate-500">{$t('settings.panel.network_hover_desc')}</div>
      </div>
      <button role="switch" aria-checked={config.networkHoverInfoEnabled ?? true}
        on:click={() => onSave({ networkHoverInfoEnabled: !(config.networkHoverInfoEnabled ?? true) })}
        class="relative w-11 h-6 rounded-full transition-colors shrink-0 {config.networkHoverInfoEnabled ?? true ? 'bg-blue-600' : 'bg-slate-700'}">
        <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {config.networkHoverInfoEnabled ?? true ? 'translate-x-5' : ''}" />
      </button>
    </div>
  </div>

  <!-- Layer surfaces — informational, see comment below -->
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-3">
    <h3 class="text-sm font-semibold text-white flex items-center gap-2"><Layers size={15} class="text-blue-400" /> {$t('settings.panel.layers_group')}</h3>
    <p class="text-xs text-slate-500">{$t('settings.panel.layers_group_desc')}</p>
    <div class="bg-slate-900/50 rounded-lg px-3 py-2 text-xs text-slate-400 font-mono">
      {$t('settings.panel.layers_status_xdg')}
    </div>
  </div>
  {/if}
</div>
