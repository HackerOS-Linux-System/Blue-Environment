<script lang="ts">
  // Blue Security and Privacy — new app. A dashboard/hub, not a
  // replacement for the existing Settings sections it links out to
  // (SecuritySection.svelte's lock-screen pattern/fingerprint setup,
  // ParentalControlsSection.svelte, ScreenTimeSection.svelte,
  // ClonedAppsSection.svelte all keep their own implementations here —
  // this reads the same backend state and gives it one place to be seen
  // at a glance, the way Android's Privacy Dashboard aggregates rather
  // than reimplements each individual permission screen).
  import { onMount } from 'svelte';
  import { ShieldCheck, Lock, Fingerprint, Users, Hourglass, Copy, Keyboard, ChevronRight, ShieldAlert } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { configStore } from '../../../utils/configStore';
  import { t } from '../../../stores/language';
  import { openApp } from '../../../stores/windowManager';
  import { AppId } from '../../../types';
  import type { SettingsTab } from '../Settings-App/types';

  interface ParentalControlsConfig {
    enabled: boolean;
    blocked_apps: string[];
    daily_limits_minutes: Record<string, number>;
  }
  interface ScreenTimeRangeSummary {
    total_minutes: number;
    by_app: Record<string, number>;
  }

  let hasPattern = false;
  let hasFingerprint = false;
  let parental: ParentalControlsConfig | null = null;
  let todayMinutes = 0;
  let clonedCount = 0;
  let keyboardEnabled = false;
  let loading = true;

  onMount(async () => {
    const username = SystemBridge.getUsername();
    const [pattern, fingerprint, pc, screenTime] = await Promise.all([
      SystemBridge.invokeCommand<boolean>('pattern_is_configured', { username, home: await SystemBridge.getHomePath() }).catch(() => false),
      SystemBridge.invokeCommand<boolean>('has_fingerprint', { username }).catch(() => false),
      SystemBridge.invokeCommand<ParentalControlsConfig>('parental_controls_get').catch(() => null),
      SystemBridge.invokeCommand<ScreenTimeRangeSummary>('screen_time_get_summary', { range: 'today' }).catch(() => null),
    ]);
    hasPattern = pattern;
    hasFingerprint = fingerprint;
    parental = pc;
    todayMinutes = screenTime?.total_minutes ?? 0;
    const cfg = configStore.get();
    clonedCount = cfg.clonedApps?.length ?? 0;
    keyboardEnabled = cfg.onscreenKeyboardEnabled ?? false;
    loading = false;
  });

  function goToSettingsTab(tab: SettingsTab) {
    openApp(AppId.SETTINGS, false, undefined, { initialTab: tab });
  }

  function fmtDuration(minutes: number): string {
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    if (h === 0) return `${m} min`;
    if (m === 0) return `${h} h`;
    return `${h} h ${m} min`;
  }

  $: lockConfigured = hasPattern || hasFingerprint;
</script>

<div class="h-full overflow-y-auto bg-slate-950 text-white p-6 space-y-5">
  <div class="flex items-center gap-3">
    <div class="w-11 h-11 rounded-2xl bg-emerald-500/15 flex items-center justify-center">
      <ShieldCheck size={22} class="text-emerald-400" />
    </div>
    <div>
      <h1 class="text-xl font-bold">{$t('security_app.title')}</h1>
      <p class="text-xs text-slate-500">{$t('security_app.subtitle')}</p>
    </div>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-16 text-slate-500 text-sm">{$t('settings.screen_time.loading')}</div>
  {:else}
    <!-- Lock screen -->
    <button on:click={() => goToSettingsTab('security')} class="w-full flex items-center gap-4 p-4 rounded-2xl bg-slate-900 border border-white/5 hover:bg-slate-900/70 transition-colors text-left">
      <div class="w-10 h-10 rounded-xl {lockConfigured ? 'bg-emerald-500/15' : 'bg-amber-500/15'} flex items-center justify-center shrink-0">
        {#if hasFingerprint}<Fingerprint size={18} class={lockConfigured ? 'text-emerald-400' : 'text-amber-400'} />
        {:else}<Lock size={18} class={lockConfigured ? 'text-emerald-400' : 'text-amber-400'} />{/if}
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium">{$t('security_app.lock_screen')}</div>
        <div class="text-xs text-slate-500">
          {lockConfigured ? $t('security_app.lock_configured') : $t('security_app.lock_not_configured')}
        </div>
      </div>
      {#if !lockConfigured}<ShieldAlert size={16} class="text-amber-400 shrink-0" />{/if}
      <ChevronRight size={16} class="text-slate-600 shrink-0" />
    </button>

    <!-- Parental controls -->
    <button on:click={() => goToSettingsTab('parental_controls')} class="w-full flex items-center gap-4 p-4 rounded-2xl bg-slate-900 border border-white/5 hover:bg-slate-900/70 transition-colors text-left">
      <div class="w-10 h-10 rounded-xl bg-blue-500/15 flex items-center justify-center shrink-0">
        <Users size={18} class="text-blue-400" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium">{$t('settings.tab.parental_controls')}</div>
        <div class="text-xs text-slate-500">
          {parental?.enabled
            ? $t('security_app.parental_on').replace('{n}', String(parental.blocked_apps.length))
            : $t('security_app.parental_off')}
        </div>
      </div>
      <ChevronRight size={16} class="text-slate-600 shrink-0" />
    </button>

    <!-- Screen time -->
    <button on:click={() => goToSettingsTab('screen_time')} class="w-full flex items-center gap-4 p-4 rounded-2xl bg-slate-900 border border-white/5 hover:bg-slate-900/70 transition-colors text-left">
      <div class="w-10 h-10 rounded-xl bg-purple-500/15 flex items-center justify-center shrink-0">
        <Hourglass size={18} class="text-purple-400" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium">{$t('settings.tab.screen_time')}</div>
        <div class="text-xs text-slate-500">{$t('security_app.screen_time_today').replace('{t}', fmtDuration(todayMinutes))}</div>
      </div>
      <ChevronRight size={16} class="text-slate-600 shrink-0" />
    </button>

    <!-- Cloned apps -->
    <button on:click={() => goToSettingsTab('cloned_apps')} class="w-full flex items-center gap-4 p-4 rounded-2xl bg-slate-900 border border-white/5 hover:bg-slate-900/70 transition-colors text-left">
      <div class="w-10 h-10 rounded-xl bg-orange-500/15 flex items-center justify-center shrink-0">
        <Copy size={18} class="text-orange-400" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium">{$t('settings.tab.cloned_apps')}</div>
        <div class="text-xs text-slate-500">
          {clonedCount === 0 ? $t('security_app.cloned_none') : $t('security_app.cloned_count').replace('{n}', String(clonedCount))}
        </div>
      </div>
      <ChevronRight size={16} class="text-slate-600 shrink-0" />
    </button>

    <!-- On-screen keyboard -->
    <button on:click={() => goToSettingsTab('keyboard')} class="w-full flex items-center gap-4 p-4 rounded-2xl bg-slate-900 border border-white/5 hover:bg-slate-900/70 transition-colors text-left">
      <div class="w-10 h-10 rounded-xl bg-teal-500/15 flex items-center justify-center shrink-0">
        <Keyboard size={18} class="text-teal-400" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium">{$t('settings.tab.keyboard')}</div>
        <div class="text-xs text-slate-500">{keyboardEnabled ? $t('security_app.enabled') : $t('security_app.disabled')}</div>
      </div>
      <ChevronRight size={16} class="text-slate-600 shrink-0" />
    </button>

    <p class="text-[11px] text-slate-600 text-center pt-2">{$t('security_app.footer_note')}</p>
  {/if}
</div>
