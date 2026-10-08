<script lang="ts">
  import type { UserConfig } from '../../../../types';
  import { APPS } from '../../../../constants';
  import { t } from '../../../../stores/language';
  import {
    normalizeHotCorners, HOT_CORNER_IDS, HOT_CORNER_ACTIONS, DEFAULT_HOT_CORNERS,
    type HotCornersConfig, type HotCornerId, type HotCornerAction,
  } from '../../../../utils/hotCorners';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  $: hc = normalizeHotCorners(config.hotCorners);
  const save = (patch: Partial<HotCornersConfig>) => onSave({ hotCorners: { ...hc, ...patch } } as Partial<UserConfig>);
  const onEnable = (e: Event) => save({ enabled: (e.currentTarget as HTMLInputElement).checked });
  const resetAll = () => onSave({ hotCorners: { ...DEFAULT_HOT_CORNERS } } as Partial<UserConfig>);
  const setCorner = (id: HotCornerId, slot: { action?: HotCornerAction; appId?: string }) =>
    save({ corners: { ...hc.corners, [id]: { ...hc.corners[id], ...slot } } });

  const NAME_KEYS: Record<HotCornerId, string> = {
    topLeft: 'scorner.top_left', topRight: 'scorner.top_right', bottomLeft: 'scorner.bottom_left', bottomRight: 'scorner.bottom_right',
  };
  const onAction = (id: HotCornerId) => (e: Event) => setCorner(id, { action: (e.currentTarget as HTMLSelectElement).value as HotCornerAction });
  const onApp = (id: HotCornerId) => (e: Event) => setCorner(id, { appId: (e.currentTarget as HTMLSelectElement).value });
  const onNum = (k: 'triggerSize' | 'dwellMs' | 'cooldownMs') => (e: Event) => save({ [k]: Number((e.currentTarget as HTMLInputElement).value) } as Partial<HotCornersConfig>);
  $: apps = Object.values(APPS).filter((a) => !a.isExternal && a.component).sort((a, b) => a.title.localeCompare(b.title));
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('scorner.title')}</h2>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <label class="flex items-center justify-between text-sm text-slate-200">
      <span>{$t('scorner.enable')}<span class="block text-xs text-slate-500">{$t('scorner.enable_hint')}</span></span>
      <input type="checkbox" checked={hc.enabled} on:change={onEnable} />
    </label>
    {#if hc.enabled}
      <label class="flex items-center justify-between text-sm text-slate-200">{$t('scorner.size', { n: hc.triggerSize })}
        <input type="range" min="2" max="40" value={hc.triggerSize} on:change={onNum('triggerSize')} /></label>
      <label class="flex items-center justify-between text-sm text-slate-200">{$t('scorner.delay', { n: hc.dwellMs })}
        <input type="range" min="0" max="1500" step="50" value={hc.dwellMs} on:change={onNum('dwellMs')} /></label>
      <label class="flex items-center justify-between text-sm text-slate-200">{$t('scorner.cooldown', { n: hc.cooldownMs })}
        <input type="range" min="200" max="5000" step="100" value={hc.cooldownMs} on:change={onNum('cooldownMs')} /></label>
    {/if}
  </div>

  {#if hc.enabled}
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
      {#each HOT_CORNER_IDS as id (id)}
        <div class="flex items-center justify-between gap-3 text-sm text-slate-200">
          <span class="w-32 shrink-0">{$t(NAME_KEYS[id])}</span>
          <select class="bg-slate-700 rounded-lg px-2 py-1 flex-1" value={hc.corners[id].action}
            on:change={onAction(id)}>
            {#each HOT_CORNER_ACTIONS as a (a.id)}<option value={a.id}>{$t(a.labelKey)}</option>{/each}
          </select>
          {#if hc.corners[id].action === 'open-app'}
            <select class="bg-slate-700 rounded-lg px-2 py-1" value={hc.corners[id].appId ?? ''}
              on:change={onApp(id)}>
              <option value="">{$t('scorner.pick_app')}</option>
              {#each apps as app (app.id)}<option value={app.id}>{app.title}</option>{/each}
            </select>
          {/if}
        </div>
      {/each}
    </div>
    <div class="text-xs text-slate-500">{$t('scorner.note')}</div>
  {/if}
  <button class="px-3 py-1.5 text-sm rounded-lg bg-white/10 hover:bg-white/20 text-white"
    on:click={resetAll}>{$t('common.reset')}</button>
</div>
