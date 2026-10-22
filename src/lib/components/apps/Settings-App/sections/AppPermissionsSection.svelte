<script lang="ts">
  import { onMount } from 'svelte';
  import { ShieldCheck, RotateCcw, Loader2, AlertTriangle } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';
  import {
    parseFlatpakList, parseContext, mergeContext, parsePermissionShow, emptyContext, hasOverrides,
    isToggleOn, overrideArgs, portalSetArgs, resetArgs, portalState,
    PORTAL_PERMISSIONS, SYSTEM_TOGGLES,
    type FlatpakApp, type Context, type PortalState, type PortalPermission, type SystemToggle,
  } from '../flatpakPermissions';

  let apps = $state<FlatpakApp[]>([]);
  let loadingApps = $state(true);
  let unavailable = $state(false);
  let selectedId = $state('');
  let loading = $state(false);
  let busy = $state(false);
  let error = $state('');
  let query = $state('');

  let effective = $state<Context>(emptyContext());
  let overridden = $state(false);
  let portal = $state<Record<string, PortalState>>({});

  let selected = $derived(apps.find((a) => a.id === selectedId));
  let visibleApps = $derived(apps.filter((a) => !query.trim() || a.name.toLowerCase().includes(query.trim().toLowerCase()) || a.id.toLowerCase().includes(query.trim().toLowerCase())));

  onMount(async () => {
    try {
      await SystemBridge.flatpak(['--version']);
      apps = parseFlatpakList(await SystemBridge.flatpak(['list', '--app', '--columns=application,name,version']));
      if (apps.length) await select(apps[0].id);
    } catch {
      unavailable = true;
    } finally {
      loadingApps = false;
    }
  });

  async function select(id: string) {
    selectedId = id;
    error = '';
    loading = true;
    try {
      const [meta, ov, show] = await Promise.all([
        SystemBridge.flatpak(['info', '--show-permissions', id]).catch(() => ''),
        SystemBridge.flatpak(['override', '--user', '--show', id]).catch(() => ''),
        SystemBridge.flatpak(['permission-show', id]).catch(() => ''),
      ]);
      const overrides = parseContext(ov);
      effective = mergeContext(parseContext(meta), overrides);
      overridden = hasOverrides(overrides);
      portal = parsePermissionShow(show);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function run(fn: () => Promise<void>) {
    if (!selectedId || busy) return;
    busy = true; error = '';
    try { await fn(); await select(selectedId); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }

  const toggleSystem = (tg: SystemToggle) =>
    run(async () => { await SystemBridge.flatpak(overrideArgs(selectedId, tg, !isToggleOn(effective, tg))); });

  const togglePortal = (p: PortalPermission) =>
    run(async () => { await SystemBridge.flatpak(portalSetArgs(selectedId, p, portalState(portal, p) !== 'granted')); });

  const resetAll = () =>
    run(async () => { for (const a of resetArgs(selectedId)) await SystemBridge.flatpak(a).catch(() => {}); });

  const stateLabel: Record<PortalState, string> = {
    granted: 'permissions.allowed', denied: 'permissions.denied', unset: 'permissions.ask',
  };
</script>

<div class="space-y-6">
  <div>
    <h2 class="text-2xl font-bold text-white">{$t('settings.tab.app_permissions')}</h2>
    <p class="text-sm text-slate-400 mt-1">{$t('permissions.subtitle')}</p>
  </div>

  {#if loadingApps}
    <div class="flex items-center gap-2 text-slate-400 text-sm"><Loader2 size={16} class="animate-spin" /> {$t('permissions.loading')}</div>
  {:else if unavailable}
    <div class="bg-slate-800/50 p-4 rounded-xl border border-white/5 flex items-start gap-3">
      <AlertTriangle size={16} class="text-amber-400 shrink-0 mt-0.5" />
      <p class="text-sm text-slate-300">{$t('permissions.no_flatpak')}</p>
    </div>
  {:else if apps.length === 0}
    <p class="text-sm text-slate-500 py-8 text-center">{$t('permissions.no_apps')}</p>
  {:else}
    <div class="grid md:grid-cols-[14rem_1fr] gap-4">
      <!-- app list -->
      <div class="bg-slate-800 rounded-2xl border border-white/5 p-2 max-h-[28rem] overflow-y-auto">
        <input bind:value={query} placeholder={$t('permissions.search')} class="w-full mb-2 bg-slate-900 border border-white/10 rounded-lg px-2.5 py-1.5 text-xs text-white placeholder:text-slate-500" />
        {#each visibleApps as a (a.id)}
          <button onclick={() => select(a.id)}
            class="w-full text-left px-3 py-2 rounded-lg text-sm truncate transition-colors {selectedId === a.id ? 'bg-blue-600 text-white' : 'text-slate-300 hover:bg-white/5'}"
            title={a.id}>{a.name}</button>
        {/each}
      </div>

      <!-- details -->
      <div class="space-y-4">
        {#if selected}
          <div class="flex items-center gap-3">
            <div class="w-10 h-10 rounded-xl bg-blue-500/15 flex items-center justify-center"><ShieldCheck size={18} class="text-blue-400" /></div>
            <div class="flex-1 min-w-0">
              <div class="text-white font-medium truncate">{selected.name}</div>
              <div class="text-xs text-slate-500 truncate">{selected.id}{selected.version ? ` · ${selected.version}` : ''}</div>
            </div>
            {#if overridden}<span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300">{$t('permissions.overridden')}</span>{/if}
            <button onclick={resetAll} disabled={busy}
              class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-slate-300 hover:bg-white/10 disabled:opacity-40"
              title={$t('permissions.reset')}><RotateCcw size={13} /> {$t('permissions.reset')}</button>
          </div>

          {#if error}<p class="text-xs text-red-400">{error}</p>{/if}

          {#if loading}
            <div class="flex items-center gap-2 text-slate-400 text-sm"><Loader2 size={16} class="animate-spin" /> {$t('permissions.loading')}</div>
          {:else}
            <div class="bg-slate-800 rounded-2xl border border-white/5 divide-y divide-white/5">
              <div class="px-4 py-2 text-xs font-medium text-slate-400 uppercase tracking-wide">{$t('permissions.portal_group')}</div>
              {#each PORTAL_PERMISSIONS as p (p.id)}
                {@const st = portalState(portal, p)}
                <div class="flex items-center justify-between px-4 py-2.5">
                  <div>
                    <div class="text-sm text-white">{$t(p.labelKey)}</div>
                    <div class="text-[11px] text-slate-500">{$t(stateLabel[st])}</div>
                  </div>
                  <button role="switch" aria-checked={st === 'granted'} aria-label={$t(p.labelKey)} disabled={busy}
                    onclick={() => togglePortal(p)}
                    class="w-10 h-6 rounded-full transition-colors relative disabled:opacity-50 {st === 'granted' ? 'bg-blue-600' : 'bg-slate-600'}">
                    <span class="absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform {st === 'granted' ? 'translate-x-4' : ''}"></span>
                  </button>
                </div>
              {/each}
            </div>

            <div class="bg-slate-800 rounded-2xl border border-white/5 divide-y divide-white/5">
              <div class="px-4 py-2 text-xs font-medium text-slate-400 uppercase tracking-wide">{$t('permissions.system_group')}</div>
              {#each SYSTEM_TOGGLES as tg (tg.id)}
                {@const on = isToggleOn(effective, tg)}
                <div class="flex items-center justify-between px-4 py-2.5">
                  <div class="text-sm text-white">{$t(tg.labelKey)}</div>
                  <button role="switch" aria-checked={on} aria-label={$t(tg.labelKey)} disabled={busy}
                    onclick={() => toggleSystem(tg)}
                    class="w-10 h-6 rounded-full transition-colors relative disabled:opacity-50 {on ? 'bg-blue-600' : 'bg-slate-600'}">
                    <span class="absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform {on ? 'translate-x-4' : ''}"></span>
                  </button>
                </div>
              {/each}
            </div>
            <p class="text-xs text-slate-500">{$t('permissions.restart_note')}</p>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>
