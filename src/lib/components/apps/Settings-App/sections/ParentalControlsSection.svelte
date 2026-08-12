<script lang="ts">
  import { onMount } from 'svelte';
  import { ShieldCheck, Lock, Clock, Ban, KeyRound } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';

  // Same static app list used by AppsSection.svelte, for a consistent
  // set of "known apps" across both sections rather than inventing a
  // second app-enumeration scheme.
  const APP_LIST = [
    ['Blue AI', 'blueAI'], ['Blue Code', 'blueCode'], ['Blue Software', 'blueSoftware'],
    ['Mail', 'mail'], ['Calculator', 'calculator'], ['Notepad', 'notepad'], ['Blue Docs', 'blue_docs'],
    ['System Monitor', 'systemMonitor'], ['Explorer', 'explorer'], ['Terminal', 'terminal'],
    ['Blue Web', 'blueWeb'], ['Camera', 'camera'],
  ] as const;

  interface ParentalConfig {
    enabled: boolean;
    blocked_apps: string[];
    daily_limits_minutes: Record<string, number>;
    usage_minutes_today: Record<string, number>;
    allowed_hours_start: string | null;
    allowed_hours_end: string | null;
  }

  let pinSet = false;
  let unlocked = false;
  let pinInput = '';
  let pinError = false;

  let cfg: ParentalConfig = {
    enabled: false, blocked_apps: [], daily_limits_minutes: {}, usage_minutes_today: {},
    allowed_hours_start: null, allowed_hours_end: null,
  };

  // Only held in memory for the duration of an "unlocked" editing
  // session — never persisted client-side. Every mutating command still
  // re-verifies it server-side (see parental_controls.rs), so holding it
  // here is purely a UX convenience (avoid re-prompting for every single
  // toggle) rather than a trust boundary.
  let sessionPin = '';

  let newPin = '';
  let newPinConfirm = '';
  let changePinError = '';

  let restrictHours = false;
  let hoursStart = '08:00';
  let hoursEnd = '20:00';

  onMount(async () => {
    pinSet = await SystemBridge.invokeCommand<boolean>('parental_controls_is_pin_set').catch(() => false);
    await refresh();
  });

  async function refresh() {
    const c = await SystemBridge.invokeCommand<ParentalConfig>('parental_controls_get').catch(() => null);
    if (c) {
      cfg = c;
      restrictHours = !!(c.allowed_hours_start && c.allowed_hours_end);
      hoursStart = c.allowed_hours_start ?? '08:00';
      hoursEnd = c.allowed_hours_end ?? '20:00';
    }
  }

  async function unlock() {
    pinError = false;
    const ok = await SystemBridge.invokeCommand<boolean>('parental_controls_verify_pin', { pin: pinInput }).catch(() => false);
    if (ok) {
      unlocked = true;
      sessionPin = pinInput;
      pinInput = '';
    } else {
      pinError = true;
    }
  }

  async function setInitialPin() {
    changePinError = '';
    if (newPin.length < 4) { changePinError = $t('settings.parental.pin_too_short'); return; }
    if (newPin !== newPinConfirm) { changePinError = $t('settings.parental.pin_mismatch'); return; }
    const ok = await SystemBridge.invokeCommand<boolean>('parental_controls_set_pin', { pin: newPin, currentPin: pinSet ? sessionPin : null }).catch(() => false);
    if (ok) {
      pinSet = true;
      unlocked = true;
      sessionPin = newPin;
      newPin = ''; newPinConfirm = '';
    } else {
      changePinError = $t('settings.parental.pin_set_failed');
    }
  }

  async function toggleEnabled() {
    const next = !cfg.enabled;
    const ok = await SystemBridge.invokeCommand<boolean>('parental_controls_set_enabled', { enabled: next, pin: sessionPin }).catch(() => false);
    if (ok) cfg.enabled = next;
  }

  async function toggleBlocked(appKey: string) {
    const blocked = new Set(cfg.blocked_apps);
    if (blocked.has(appKey)) blocked.delete(appKey); else blocked.add(appKey);
    const list = Array.from(blocked);
    const ok = await SystemBridge.invokeCommand<boolean>('parental_controls_set_blocked_apps', { apps: list, pin: sessionPin }).catch(() => false);
    if (ok) cfg.blocked_apps = list;
  }

  async function setLimit(appKey: string, minutesStr: string) {
    const minutes = minutesStr.trim() === '' ? null : Math.max(0, parseInt(minutesStr, 10) || 0);
    await SystemBridge.invokeCommand('parental_controls_set_daily_limit', { appId: appKey, minutes, pin: sessionPin }).catch(() => {});
    await refresh();
  }

  async function saveHours() {
    const start = restrictHours ? hoursStart : null;
    const end = restrictHours ? hoursEnd : null;
    await SystemBridge.invokeCommand('parental_controls_set_allowed_hours', { start, end, pin: sessionPin }).catch(() => {});
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white flex items-center gap-2">
    <ShieldCheck class="w-6 h-6 text-blue-400" /> {$t('settings.parental.title')}
  </h2>

  {#if !pinSet}
    <!-- First-time setup: no PIN exists yet, so there's nothing to
         unlock — go straight to "create a PIN". -->
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
      <p class="text-white/70 text-sm">{$t('settings.parental.setup_intro')}</p>
      <div class="flex flex-col gap-3 max-w-xs">
        <input type="password" inputmode="numeric" placeholder={$t('settings.parental.new_pin')} bind:value={newPin}
          class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-white" />
        <input type="password" inputmode="numeric" placeholder={$t('settings.parental.confirm_pin')} bind:value={newPinConfirm}
          class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-white" />
        {#if changePinError}<p class="text-red-400 text-sm">{changePinError}</p>{/if}
        <button on:click={setInitialPin}
          class="flex items-center justify-center gap-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg px-4 py-2">
          <KeyRound class="w-4 h-4" /> {$t('settings.parental.set_pin')}
        </button>
      </div>
    </div>
  {:else if !unlocked}
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4 max-w-xs">
      <p class="text-white/70 text-sm flex items-center gap-2"><Lock class="w-4 h-4" /> {$t('settings.parental.enter_pin')}</p>
      <input type="password" inputmode="numeric" placeholder={$t('settings.parental.pin_placeholder')} bind:value={pinInput}
        on:keydown={(e) => e.key === 'Enter' && unlock()}
        class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-white w-full" />
      {#if pinError}<p class="text-red-400 text-sm">{$t('settings.parental.incorrect_pin')}</p>{/if}
      <button on:click={unlock} class="bg-blue-600 hover:bg-blue-500 text-white rounded-lg px-4 py-2 w-full">{$t('settings.parental.unlock')}</button>
    </div>
  {:else}
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 flex items-center justify-between">
      <div>
        <p class="text-white font-medium">{$t('settings.parental.title')}</p>
        <p class="text-white/50 text-sm">{$t('settings.parental.off_desc')}</p>
      </div>
      <button on:click={toggleEnabled}
        class="relative w-12 h-6 rounded-full transition-colors {cfg.enabled ? 'bg-blue-600' : 'bg-white/10'}">
        <span class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {cfg.enabled ? 'translate-x-6' : ''}" />
      </button>
    </div>

    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
      <h3 class="text-white font-medium flex items-center gap-2"><Ban class="w-4 h-4" /> {$t('settings.parental.blocked_apps')}</h3>
      <div class="grid grid-cols-2 gap-y-2">
        {#each APP_LIST as [name, key] (key)}
          <label class="flex items-center gap-2 text-white/80 text-sm">
            <input type="checkbox" checked={cfg.blocked_apps.includes(key)}
              on:change={() => toggleBlocked(key)} class="w-4 h-4 accent-red-500" />
            {name}
          </label>
        {/each}
      </div>
    </div>

    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
      <h3 class="text-white font-medium flex items-center gap-2"><Clock class="w-4 h-4" /> {$t('settings.parental.daily_limits')}</h3>
      <div class="space-y-2">
        {#each APP_LIST as [name, key] (key)}
          <div class="flex items-center justify-between text-sm">
            <span class="text-white/80">{name}</span>
            <div class="flex items-center gap-2">
              {#if cfg.usage_minutes_today[key]}
                <span class="text-white/40 text-xs">{$t('settings.parental.used_today').replace('{min}', String(cfg.usage_minutes_today[key]))}</span>
              {/if}
              <input type="number" min="0" placeholder={$t('settings.parental.no_limit')}
                value={cfg.daily_limits_minutes[key] ?? ''}
                on:change={(e) => setLimit(key, e.currentTarget.value)}
                class="w-24 bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white text-right" />
              <span class="text-white/40 text-xs">{$t('settings.parental.minutes_short')}</span>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
      <h3 class="text-white font-medium">{$t('settings.parental.allowed_hours')}</h3>
      <label class="flex items-center gap-2 text-white/80 text-sm">
        <input type="checkbox" bind:checked={restrictHours} on:change={saveHours} class="w-4 h-4 accent-blue-500" />
        {$t('settings.parental.restrict_hours')}
      </label>
      {#if restrictHours}
        <div class="flex items-center gap-3">
          <input type="time" bind:value={hoursStart} on:change={saveHours}
            class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" />
          <span class="text-white/40">{$t('settings.parental.to')}</span>
          <input type="time" bind:value={hoursEnd} on:change={saveHours}
            class="bg-slate-900 border border-white/10 rounded-lg px-2 py-1 text-white" />
        </div>
      {/if}
    </div>

    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-3">
      <h3 class="text-white font-medium">{$t('settings.parental.change_pin')}</h3>
      <div class="flex flex-wrap items-center gap-3">
        <input type="password" inputmode="numeric" placeholder={$t('settings.parental.new_pin')} bind:value={newPin}
          class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-white w-32" />
        <input type="password" inputmode="numeric" placeholder={$t('settings.parental.confirm_short')} bind:value={newPinConfirm}
          class="bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-white w-32" />
        <button on:click={setInitialPin} class="bg-blue-600 hover:bg-blue-500 text-white rounded-lg px-4 py-2">{$t('settings.common.update')}</button>
      </div>
      {#if changePinError}<p class="text-red-400 text-sm">{changePinError}</p>{/if}
    </div>
  {/if}
</div>
