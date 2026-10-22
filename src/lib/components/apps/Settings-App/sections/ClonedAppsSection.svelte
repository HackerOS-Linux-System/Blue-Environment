<script lang="ts">
  import { Copy, Trash2, Play, Plus, Info } from 'lucide-svelte';
  import type { UserConfig, ClonedAppEntry } from '../../../../types';
  import { AppId } from '../../../../types';
  import { APPS } from '../../../../constants';
  import { t } from '../../../../stores/language';
  import { openApp } from '../../../../stores/windowManager';
  import { SystemBridge, type DesktopEntry } from '../../../../utils/systemBridge';
  import { onMount } from 'svelte';

  interface Props {
    config: UserConfig;
    onSave: (p: Partial<UserConfig>) => Promise<void>;
  }

  let { config, onSave }: Props = $props();

  // Same registry + filter AppsSection.svelte uses for "which apps
  // exist and can meaningfully be launched as a normal window" — an
  // external (non-webview) app or Settings itself doesn't make sense to
  // clone the same way a regular app does.
  let cloneableApps = $derived(Object.values(APPS)
    .filter((app) => !app.isExternal && app.component && app.id !== AppId.SETTINGS)
    .sort((a, b) => a.title.localeCompare(b.title)));

  let clones = $derived(config.clonedApps ?? []);

  // Template `{@const}`/`{#each}` expressions use Svelte's own limited
  // expression parser, which doesn't accept TS `as` casts (confirmed by
  // a real `svelte-check` run: "Unexpected token") — this indirection
  // exists purely so the template can look up an app by a plain
  // `string` id without needing `APPS[id as AppId]` inline.
  function getApp(id: string): (typeof APPS)[AppId] | undefined {
    return APPS[id as AppId];
  }

  // Literal keys (not string concatenation) so the i18n usage test can verify them.
  const ISOLATION_KEY = {
    env: 'settings.cloned_apps.isolation_env',
    bwrap: 'settings.cloned_apps.isolation_bwrap',
    none: 'settings.cloned_apps.isolation_none',
  } as const;

  let creating = $state(false);
  let selectedBaseId = $state<string>('');
  let label = $state('');

  // ── installed (external) apps cloned with an isolated profile ──
  let cloneKind = $state<'builtin' | 'external'>('builtin');
  let installedApps = $state<DesktopEntry[]>([]);
  let selectedExternalId = $state('');
  let isolation = $state<'env' | 'bwrap' | 'none'>('env');
  let bwrapAvailable = $state(false);
  let profileRoot = $state('~/.local/share/blue-clones');

  onMount(async () => {
    try {
      const [apps, st] = await Promise.all([SystemBridge.getSystemApps(false), SystemBridge.cloneIsolationStatus()]);
      installedApps = apps.filter((a) => !a.noDisplay).sort((a, b) => a.name.localeCompare(b.name));
      bwrapAvailable = st.bwrapAvailable;
      profileRoot = st.root;
    } catch { /* browser preview — keep defaults */ }
  });

  let selectedExternal = $derived(installedApps.find((a) => a.id === selectedExternalId));

  let baseApp = $derived(selectedBaseId ? APPS[selectedBaseId as AppId] : undefined);
  $effect(() => {
    const name = cloneKind === 'external' ? selectedExternal?.name : baseApp?.title;
    if (name && !label) label = `${name} (2)`;
  });

  function startCreating() {
    creating = true;
    selectedBaseId = cloneableApps[0]?.id ?? '';
    selectedExternalId = installedApps[0]?.id ?? '';
    cloneKind = 'builtin';
    isolation = 'env';
    label = '';
  }

  async function confirmCreate() {
    const external = cloneKind === 'external';
    if (!label.trim() || (external ? !selectedExternal : !selectedBaseId)) return;
    const entry: ClonedAppEntry = external
      ? { id: crypto.randomUUID(), baseAppId: selectedExternal!.id, label: label.trim(), kind: 'external', exec: selectedExternal!.exec, isolation }
      : { id: crypto.randomUUID(), baseAppId: selectedBaseId, label: label.trim() };
    await onSave({ clonedApps: [...clones, entry] });
    creating = false;
  }

  async function removeClone(id: string, wipeData = false) {
    if (wipeData) await SystemBridge.deleteCloneProfile(id).catch(() => {});
    await onSave({ clonedApps: clones.filter((c) => c.id !== id) });
  }

  function openClone(entry: ClonedAppEntry) {
    openApp(`clone:${entry.id}`);
  }
</script>

<div class="space-y-6">
  <div>
    <h2 class="text-2xl font-bold text-white">{$t('settings.tab.cloned_apps')}</h2>
    <p class="text-sm text-slate-400 mt-1">{$t('settings.cloned_apps.subtitle')}</p>
  </div>

  <div class="bg-slate-800/50 p-4 rounded-xl border border-white/5 flex items-start gap-3">
    <Info size={16} class="text-blue-400 shrink-0 mt-0.5" />
    <p class="text-xs text-slate-400 leading-relaxed">{$t('settings.cloned_apps.scope_note')}</p>
  </div>

  <!-- Existing clones -->
  <div class="bg-slate-800 rounded-2xl border border-white/5 divide-y divide-white/5">
    {#if clones.length === 0 && !creating}
      <p class="text-sm text-slate-500 py-8 text-center">{$t('settings.cloned_apps.empty')}</p>
    {/if}
    {#each clones as entry (entry.id)}
      {@const base = getApp(entry.baseAppId)}
      <div class="flex items-center gap-3 p-4">
        <div class="w-10 h-10 rounded-xl bg-blue-500/15 flex items-center justify-center shrink-0 relative">
          {#if base}
            <base.icon size={18} class="text-blue-400" />
          {:else}
            <Copy size={16} class="text-blue-400" />
          {/if}
          <span class="absolute -bottom-1 -right-1 w-4 h-4 rounded-full bg-slate-700 border border-slate-800 flex items-center justify-center">
            <Copy size={9} class="text-slate-300" />
          </span>
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-white font-medium truncate">{entry.label}</div>
          <div class="text-xs text-slate-500 truncate">
            {$t('settings.cloned_apps.based_on')} {base?.title ?? entry.baseAppId}{#if entry.kind === 'external'} · {$t(ISOLATION_KEY[entry.isolation ?? 'env'])}{/if}
          </div>
        </div>
        <button
          onclick={() => openClone(entry)}
          class="p-2 rounded-lg hover:bg-white/10 text-slate-300"
          title={$t('settings.cloned_apps.open')} aria-label={$t('settings.cloned_apps.open')}>
          <Play size={16} />
        </button>
        <button
          onclick={() => removeClone(entry.id, entry.kind === 'external' && confirm($t('settings.cloned_apps.wipe_confirm')))}
          class="p-2 rounded-lg hover:bg-red-500/10 text-slate-400 hover:text-red-400"
          title={$t('settings.cloned_apps.remove')} aria-label={$t('settings.cloned_apps.remove')}>
          <Trash2 size={16} />
        </button>
      </div>
    {/each}
  </div>

  <!-- Create flow -->
  {#if creating}
    <div class="bg-slate-800 p-5 rounded-2xl border border-white/10 space-y-4">
      <div class="flex gap-1 bg-slate-900 p-1 rounded-xl">
        <button onclick={() => (cloneKind = 'builtin')} class="flex-1 py-1.5 rounded-lg text-sm {cloneKind === 'builtin' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white'}">{$t('settings.cloned_apps.kind_builtin')}</button>
        <button onclick={() => (cloneKind = 'external')} class="flex-1 py-1.5 rounded-lg text-sm {cloneKind === 'external' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white'}">{$t('settings.cloned_apps.kind_external')}</button>
      </div>
      {#if cloneKind === 'external'}
        <div>
          <label for="clone-ext-app" class="block text-xs font-medium text-slate-400 mb-1.5">{$t('settings.cloned_apps.pick_app')}</label>
          <select id="clone-ext-app" bind:value={selectedExternalId} class="w-full bg-slate-900 border border-white/10 rounded-xl px-3 py-2 text-white text-sm">
            {#each installedApps as app (app.id)}
              <option value={app.id}>{app.name}</option>
            {/each}
          </select>
        </div>
        <div>
          <label for="clone-isolation" class="block text-xs font-medium text-slate-400 mb-1.5">{$t('settings.cloned_apps.isolation')}</label>
          <select id="clone-isolation" bind:value={isolation} class="w-full bg-slate-900 border border-white/10 rounded-xl px-3 py-2 text-white text-sm">
            <option value="env">{$t('settings.cloned_apps.isolation_env')}</option>
            <option value="bwrap" disabled={!bwrapAvailable}>{$t('settings.cloned_apps.isolation_bwrap')}{bwrapAvailable ? '' : ' — ' + $t('settings.cloned_apps.bwrap_missing')}</option>
            <option value="none">{$t('settings.cloned_apps.isolation_none')}</option>
          </select>
          <p class="text-xs text-slate-500 mt-1.5">{$t('settings.cloned_apps.isolation_hint')} <code class="text-slate-400">{profileRoot}/&lt;id&gt;</code></p>
        </div>
      {:else}
      <div>
        <label for="clone-base-app" class="block text-xs font-medium text-slate-400 mb-1.5">{$t('settings.cloned_apps.pick_app')}</label>
        <select id="clone-base-app" bind:value={selectedBaseId} class="w-full bg-slate-900 border border-white/10 rounded-xl px-3 py-2 text-white text-sm">
          {#each cloneableApps as app (app.id)}
            <option value={app.id}>{app.title}</option>
          {/each}
        </select>
      </div>
      {/if}
      <div>
        <label for="clone-label" class="block text-xs font-medium text-slate-400 mb-1.5">{$t('settings.cloned_apps.name_label')}</label>
        <input id="clone-label" type="text" bind:value={label} class="w-full bg-slate-900 border border-white/10 rounded-xl px-3 py-2 text-white text-sm" />
      </div>
      <div class="flex gap-2 justify-end">
        <button onclick={() => (creating = false)} class="px-4 py-2 rounded-xl text-sm text-slate-400 hover:bg-white/5">{$t('settings.cloned_apps.cancel')}</button>
        <button onclick={confirmCreate} disabled={!label.trim() || (cloneKind === 'external' ? !selectedExternalId : !selectedBaseId)} class="px-4 py-2 rounded-xl text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:pointer-events-none text-white font-medium">
          {$t('settings.cloned_apps.create')}
        </button>
      </div>
    </div>
  {:else}
    <button
      onclick={startCreating}
      class="w-full flex items-center justify-center gap-2 py-2.5 rounded-xl text-sm font-medium border border-dashed border-white/15 text-slate-400 hover:bg-white/5 hover:text-white transition-colors">
      <Plus size={15} />
      {$t('settings.cloned_apps.create')}
    </button>
  {/if}
</div>
