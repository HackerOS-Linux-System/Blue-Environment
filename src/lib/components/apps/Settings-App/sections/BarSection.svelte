<script lang="ts">
  import type { UserConfig } from '../../../../types';
  import { t } from '../../../../stores/language';
  import { normalizeTopBar, DEFAULT_TOP_BAR, moveRightItem, type TopBarConfig, type BarRightItem } from '../../../../utils/topBarConfig';
  import { ChevronUp, ChevronDown } from 'lucide-svelte';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  $: bar = normalizeTopBar(config.topBar);
  const save = (patch: Partial<TopBarConfig>) => onSave({ topBar: { ...bar, ...patch } } as Partial<UserConfig>);
  const onBool = (k: keyof TopBarConfig) => (e: Event) => save({ [k]: (e.currentTarget as HTMLInputElement).checked } as Partial<TopBarConfig>);
  const onNum = (k: keyof TopBarConfig) => (e: Event) => save({ [k]: Number((e.currentTarget as HTMLInputElement).value) } as Partial<TopBarConfig>);
  const onStr = (k: keyof TopBarConfig) => (e: Event) => save({ [k]: (e.currentTarget as HTMLInputElement | HTMLSelectElement).value } as Partial<TopBarConfig>);
  const isOn = (b: TopBarConfig, k: keyof TopBarConfig) => b[k] === true;
  const reset = () => onSave({ topBar: { ...DEFAULT_TOP_BAR } } as Partial<UserConfig>);

  const ORDER_LABELS: Record<BarRightItem, string> = {
    workspaces: 'sbar.workspaces', weather: 'sbar.weather', battery: 'sbar.battery',
    clipboard: 'sbar.clipboard', network: 'sbar.network', notifications: 'sbar.notifications',
  };
  const move = (index: number, delta: number) => save({ rightOrder: moveRightItem(bar.rightOrder, index, delta) });

  const TOGGLES: { key: keyof TopBarConfig; labelKey: string }[] = [
    { key: 'showStartButton', labelKey: 'sbar.start' },
    { key: 'showRunningApps', labelKey: 'sbar.running_apps' },
    { key: 'showPinned', labelKey: 'sbar.pinned' },
    { key: 'showWorkspaces', labelKey: 'sbar.workspaces' },
    { key: 'showWeather', labelKey: 'sbar.weather' },
    { key: 'showBattery', labelKey: 'sbar.battery' },
    { key: 'showClipboard', labelKey: 'sbar.clipboard' },
    { key: 'showNotifications', labelKey: 'sbar.notifications' },
    { key: 'showNetworkIcon', labelKey: 'sbar.network' },
    { key: 'showClock', labelKey: 'sbar.clock' },
    { key: 'showBatteryPercent', labelKey: 'sbar.battery_percent' },
    { key: 'accentIndicators', labelKey: 'sbar.indicators' },
  ];
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('sbar.title')}</h2>

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
    <div class="text-sm font-medium text-slate-400">{$t('sbar.order_title')}</div>
    <p class="text-xs text-slate-500">{$t('sbar.order_hint')}</p>
    {#each bar.rightOrder as item, i (item)}
      <div class="flex items-center justify-between text-sm text-slate-200 bg-slate-700/40 rounded-lg px-3 py-1.5">
        <span>{$t(ORDER_LABELS[item])}</span>
        <span class="flex gap-1">
          <button class="p-1 rounded hover:bg-white/10 disabled:opacity-30" disabled={i === 0} title={$t('sbar.order_up')} aria-label={$t('sbar.order_up')} on:click={() => move(i, -1)}><ChevronUp size={14} /></button>
          <button class="p-1 rounded hover:bg-white/10 disabled:opacity-30" disabled={i === bar.rightOrder.length - 1} title={$t('sbar.order_down')} aria-label={$t('sbar.order_down')} on:click={() => move(i, 1)}><ChevronDown size={14} /></button>
        </span>
      </div>
    {/each}
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
