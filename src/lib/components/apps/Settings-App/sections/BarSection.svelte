<script lang="ts">
  import type { UserConfig } from '../../../../types';
  import { t } from '../../../../stores/language';
  import { normalizeTopBar, DEFAULT_TOP_BAR, type TopBarConfig, type BarRightItem, type BarLeftItem } from '../../../../utils/topBarConfig';
  import { BAR_PRESETS, applyBarPreset } from '../../../../utils/barPresets';
  import ReorderList from './ReorderList.svelte';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  $: bar = normalizeTopBar(config.topBar);
  const save = (patch: Partial<TopBarConfig>) => onSave({ topBar: { ...bar, ...patch } } as Partial<UserConfig>);
  const onBool = (k: keyof TopBarConfig) => (e: Event) => save({ [k]: (e.currentTarget as HTMLInputElement).checked } as Partial<TopBarConfig>);
  const onNum = (k: keyof TopBarConfig) => (e: Event) => save({ [k]: Number((e.currentTarget as HTMLInputElement).value) } as Partial<TopBarConfig>);
  const onStr = (k: keyof TopBarConfig) => (e: Event) => save({ [k]: (e.currentTarget as HTMLInputElement | HTMLSelectElement).value } as Partial<TopBarConfig>);
  const isOn = (b: TopBarConfig, k: keyof TopBarConfig) => b[k] === true;
  const reset = () => onSave({ topBar: { ...DEFAULT_TOP_BAR } } as Partial<UserConfig>);

  // Widgets that can be reordered AND switched on/off from the lists below.
  const RIGHT: Record<BarRightItem, { label: string; flag: keyof TopBarConfig }> = {
    workspaces:    { label: 'sbar.workspaces',    flag: 'showWorkspaces' },
    weather:       { label: 'sbar.weather',       flag: 'showWeather' },
    battery:       { label: 'sbar.battery',       flag: 'showBattery' },
    clipboard:     { label: 'sbar.clipboard',     flag: 'showClipboard' },
    network:       { label: 'sbar.network',       flag: 'showNetworkIcon' },
    notifications: { label: 'sbar.notifications', flag: 'showNotifications' },
    showDesktop:   { label: 'sbar.show_desktop',  flag: 'showDesktopButton' },
    power:         { label: 'startmenu.power',    flag: 'showPowerButton' },
  };
  const LEFT: Record<BarLeftItem, { label: string; flag: keyof TopBarConfig }> = {
    start:   { label: 'sbar.start',        flag: 'showStartButton' },
    running: { label: 'sbar.running_apps', flag: 'showRunningApps' },
  };
  // ReorderList speaks plain strings; these narrow them back to the item unions.
  const rightOf = (id: string) => RIGHT[id as BarRightItem];
  const leftOf = (id: string) => LEFT[id as BarLeftItem];
  const reorderLeft = (e: CustomEvent<string[]>) => save({ leftOrder: e.detail as BarLeftItem[] });
  const reorderRight = (e: CustomEvent<string[]>) => save({ rightOrder: e.detail as BarRightItem[] });
  const toggleFlag = (flag: keyof TopBarConfig) => save({ [flag]: !bar[flag] } as Partial<TopBarConfig>);

  // Presets: a bar look in one click. Some also move the bar to a screen edge.
  function applyPreset(id: string) {
    const preset = BAR_PRESETS.find((p) => p.id === id);
    if (!preset) return;
    const patch: Partial<UserConfig> = { topBar: applyBarPreset(bar, id) } as Partial<UserConfig>;
    if (preset.panelPosition) (patch as Record<string, unknown>).panelPosition = preset.panelPosition;
    return onSave(patch);
  }

  const TOGGLES: { key: keyof TopBarConfig; labelKey: string }[] = [
    { key: 'showPinned', labelKey: 'sbar.pinned' },
    { key: 'showClock', labelKey: 'sbar.clock' },
    { key: 'showBatteryPercent', labelKey: 'sbar.battery_percent' },
    { key: 'accentIndicators', labelKey: 'sbar.indicators' },
  ];
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('sbar.title')}</h2>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-3">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.presets_title')}</div>
    <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
      {#each BAR_PRESETS as preset (preset.id)}
        <button class="text-left px-3 py-2.5 rounded-xl border border-white/5 bg-slate-900/40 hover:bg-white/5 transition-colors" on:click={() => applyPreset(preset.id)}>
          <div class="text-sm text-slate-200">{$t(preset.labelKey)}</div>
          <div class="text-[11px] text-slate-500 mt-0.5">{$t(preset.descKey)}</div>
        </button>
      {/each}
    </div>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-3">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.elements')}</div>
    {#each TOGGLES as item (item.key)}
      <label class="flex items-center justify-between text-sm text-slate-200">
        <span>{$t(item.labelKey)}</span>
        <input type="checkbox" checked={isOn(bar, item.key)} on:change={onBool(item.key)} />
      </label>
    {/each}
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.clock_title')}</div>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.clock_format')}
      <select class="bg-slate-700 rounded-lg px-2 py-1" value={bar.clockStyle} on:change={onStr('clockStyle')}>
        <option value="time">{$t('sbar.clock_time')}</option>
        <option value="time-date">{$t('sbar.clock_time_date')}</option>
        <option value="date-time">{$t('sbar.clock_date_time')}</option>
      </select>
    </label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.clock_24h')}
      <input type="checkbox" checked={bar.clock24h} on:change={onBool('clock24h')} /></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.clock_seconds')}
      <input type="checkbox" checked={bar.showSeconds} on:change={onBool('showSeconds')} /></label>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.look')}</div>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.start_label')}
      <input class="bg-slate-700 rounded-lg px-2 py-1 w-32" maxlength="24" placeholder="Blue" value={bar.startLabel} on:change={onStr('startLabel')} /></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.icon_size', { n: bar.pinnedIconSize })}
      <input type="range" min="14" max="32" value={bar.pinnedIconSize} on:change={onNum('pinnedIconSize')} /></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.running_max', { n: bar.runningAppsMax })}
      <input type="range" min="1" max="12" step="1" value={bar.runningAppsMax} on:change={onNum('runningAppsMax')} /></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.running_labels')}
      <input type="checkbox" checked={bar.runningAppsLabels} on:change={onBool('runningAppsLabels')} /></label>
    <p class="text-xs text-slate-500 -mt-2">{$t('sbar.running_hint')}</p>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.blur')}
      <select class="bg-slate-700 rounded-lg px-2 py-1" value={bar.blur} on:change={onStr('blur')}>
        <option value="none">{$t('sbar.blur_none')}</option><option value="sm">{$t('sbar.blur_sm')}</option><option value="md">{$t('sbar.blur_md')}</option><option value="xl">{$t('sbar.blur_xl')}</option>
      </select></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.border')}
      <input type="checkbox" checked={bar.showBorder} on:change={onBool('showBorder')} /></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.floating')}
      <input type="checkbox" checked={bar.floating} on:change={onBool('floating')} /></label>
    {#if bar.floating}
      <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.margin', { n: bar.floatingMargin })}
        <input type="range" min="0" max="24" value={bar.floatingMargin} on:change={onNum('floatingMargin')} /></label>
      <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.radius', { n: bar.cornerRadius })}
        <input type="range" min="0" max="28" value={bar.cornerRadius} on:change={onNum('cornerRadius')} /></label>
    {/if}
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.autohide')}
      <input type="checkbox" checked={bar.autoHide} on:change={onBool('autoHide')} /></label>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.bg_title')}</div>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.bg_mode')}
      <select class="bg-slate-700 rounded-lg px-2 py-1" value={bar.backgroundMode} on:change={onStr('backgroundMode')}>
        <option value="theme">{$t('sbar.bg_theme')}</option><option value="custom">{$t('sbar.bg_custom')}</option>
      </select></label>
    {#if bar.backgroundMode === 'custom'}
      <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.bg_color')}
        <input type="color" class="w-9 h-8 rounded-lg bg-slate-700 border border-white/10 cursor-pointer" value={bar.backgroundColor} on:change={onStr('backgroundColor')} /></label>
      <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.bg_opacity', { n: bar.backgroundOpacity })}
        <input type="range" min="10" max="100" value={bar.backgroundOpacity} on:change={onNum('backgroundOpacity')} /></label>
    {/if}
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.shadow')}
      <input type="checkbox" checked={bar.shadow} on:change={onBool('shadow')} /></label>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.layout_title')}</div>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.font_size', { n: bar.fontSize })}
      <input type="range" min="10" max="18" value={bar.fontSize} on:change={onNum('fontSize')} /></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.spacing', { n: bar.itemSpacing })}
      <input type="range" min="0" max="16" value={bar.itemSpacing} on:change={onNum('itemSpacing')} /></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.start_style')}
      <select class="bg-slate-700 rounded-lg px-2 py-1" value={bar.startButtonStyle} on:change={onStr('startButtonStyle')}>
        <option value="rounded">{$t('sbar.start_rounded')}</option><option value="pill">{$t('sbar.start_pill')}</option><option value="square">{$t('sbar.start_square')}</option>
      </select></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.pinned_style')}
      <select class="bg-slate-700 rounded-lg px-2 py-1" value={bar.pinnedStyle} on:change={onStr('pinnedStyle')}>
        <option value="dock">{$t('sbar.pinned_dock')}</option><option value="flat">{$t('sbar.pinned_flat')}</option>
      </select></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.indicator_style')}
      <select class="bg-slate-700 rounded-lg px-2 py-1" value={bar.indicatorStyle} on:change={onStr('indicatorStyle')}>
        <option value="line">{$t('sbar.ind_line')}</option><option value="dot">{$t('sbar.ind_dot')}</option><option value="glow">{$t('sbar.ind_glow')}</option>
      </select></label>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.workspace_style')}
      <select class="bg-slate-700 rounded-lg px-2 py-1" value={bar.workspaceStyle} on:change={onStr('workspaceStyle')}>
        <option value="dots">{$t('sbar.ws_dots')}</option><option value="numbers">{$t('sbar.ws_numbers')}</option>
      </select></label>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-2">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.order_left_title')}</div>
    <p class="text-xs text-slate-500">{$t('sbar.order_hint')}</p>
    <ReorderList items={bar.leftOrder} labelFor={(id) => $t(leftOf(id).label)} isOn={(id) => bar[leftOf(id).flag] === true}
      on:reorder={reorderLeft} on:toggle={(e) => toggleFlag(leftOf(e.detail).flag)} />
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-2">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.order_title')}</div>
    <p class="text-xs text-slate-500">{$t('sbar.order_hint')}</p>
    <ReorderList items={bar.rightOrder} labelFor={(id) => $t(rightOf(id).label)} isOn={(id) => bar[rightOf(id).flag] === true}
      on:reorder={reorderRight} on:toggle={(e) => toggleFlag(rightOf(e.detail).flag)} />
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-3">
    <div class="text-sm font-medium text-slate-400">{$t('sbar.osd_title')}</div>
    <label class="flex items-center justify-between text-sm text-slate-200">{$t('sbar.osd_enabled')}
      <input type="checkbox" checked={config.osdEnabled !== false} on:change={(e) => onSave({ osdEnabled: e.currentTarget.checked })} /></label>
    <p class="text-xs text-slate-500">{$t('sbar.osd_hint')}</p>
  </div>

  <div class="text-xs text-slate-500">{$t('sbar.hint')}</div>
  <button class="px-3 py-1.5 text-sm rounded-lg bg-white/10 hover:bg-white/20 text-white" on:click={reset}>{$t('common.reset')}</button>
</div>
