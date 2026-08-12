<script lang="ts">
  import { onMount } from 'svelte';
  import { Shield, Fingerprint, Grid3x3, Trash2, Check } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import PatternLock from '../../../PatternLock.svelte';
  import { dialogConfirm } from '../../../../stores/dialog';
  import { t } from '../../../../stores/language';

  let hasPattern = false;
  let hasFingerprint = false;
  let settingUp = false;
  let firstPattern: number[] | null = null;
  let error = false;
  let saved = false;
  let username = '';

  onMount(async () => {
    username = SystemBridge.getUsername();
    hasPattern = await SystemBridge.invokeCommand<boolean>('pattern_is_configured', {
      username, home: await SystemBridge.getHomePath(),
    }).catch(() => false);
    hasFingerprint = await SystemBridge.invokeCommand<boolean>('has_fingerprint', { username }).catch(() => false);
  });

  function startSetup() { settingUp = true; firstPattern = null; error = false; saved = false; }

  async function handlePattern(pattern: number[]) {
    if (!firstPattern) {
      firstPattern = pattern;
      return;
    }
    // Confirm — must match the first draw, same UX as Android's pattern setup.
    const matches = pattern.length === firstPattern.length && pattern.every((v, i) => v === firstPattern![i]);
    if (!matches) { error = true; firstPattern = null; setTimeout(() => (error = false), 600); return; }

    try {
      await SystemBridge.invokeCommand('save_pattern_lock', { username, pattern });
      hasPattern = true;
      saved = true;
      settingUp = false;
    } catch {
      error = true;
    }
  }

  function onPatternComplete(e: CustomEvent<number[]>) {
    handlePattern(e.detail);
  }

  async function removePattern() {
    const ok = await dialogConfirm({ title: $t('settings.security.remove_pattern_title'), message: $t('settings.security.remove_pattern_message'), confirmLabel: $t('settings.common.remove'), danger: true });
    if (!ok) return;
    await SystemBridge.invokeCommand('delete_pattern_lock', { username }).catch(() => {});
    hasPattern = false;
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white flex items-center gap-2"><Shield size={22} class="text-blue-400" /> {$t('settings.security.title')}</h2>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <div class="flex items-center gap-3 mb-4">
      <div class="w-10 h-10 rounded-xl bg-blue-600/20 flex items-center justify-center"><Grid3x3 size={18} class="text-blue-400" /></div>
      <div class="flex-1">
        <div class="text-white font-medium">{$t('settings.security.pattern_lock')}</div>
        <div class="text-xs text-slate-500">{$t('settings.security.pattern_lock_desc')}</div>
      </div>
      {#if hasPattern && !settingUp}
        <span class="text-xs bg-green-600/20 text-green-300 px-2 py-1 rounded-full border border-green-500/20">{$t('settings.security.configured')}</span>
      {/if}
    </div>

    {#if settingUp}
      <div class="flex flex-col items-center gap-3 py-4">
        <PatternLock error={error} on:complete={onPatternComplete} />
        <p class="text-sm text-slate-400">
          {#if error}{$t('settings.security.pattern_mismatch')}{:else if firstPattern}{$t('settings.security.pattern_confirm')}{:else}{$t('settings.security.pattern_draw')}{/if}
        </p>
        <button on:click={() => (settingUp = false)} class="text-xs text-slate-500 hover:text-white">{$t('settings.common.cancel')}</button>
      </div>
    {:else}
      <div class="flex gap-2">
        <button on:click={startSetup} class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm text-white">
          {hasPattern ? $t('settings.security.change_pattern') : $t('settings.security.setup_pattern')}
        </button>
        {#if hasPattern}
          <button on:click={removePattern} class="px-3 py-2 bg-red-600/20 hover:bg-red-500/30 text-red-400 rounded-xl text-sm flex items-center gap-1.5"><Trash2 size={13} /> {$t('settings.common.remove')}</button>
        {/if}
      </div>
      {#if saved}<p class="text-xs text-green-400 mt-2 flex items-center gap-1"><Check size={12} /> {$t('settings.security.pattern_saved')}</p>{/if}
    {/if}
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <div class="flex items-center gap-3">
      <div class="w-10 h-10 rounded-xl bg-blue-600/20 flex items-center justify-center"><Fingerprint size={18} class="text-blue-400" /></div>
      <div class="flex-1">
        <div class="text-white font-medium">{$t('settings.security.fingerprint')}</div>
        <div class="text-xs text-slate-500">
          {#if hasFingerprint}{$t('settings.security.fingerprint_enrolled_desc')}
          {:else}{$t('settings.security.fingerprint_none_desc')}{/if}
        </div>
      </div>
      {#if hasFingerprint}
        <span class="text-xs bg-green-600/20 text-green-300 px-2 py-1 rounded-full border border-green-500/20">{$t('settings.security.enrolled')}</span>
      {/if}
    </div>
    {#if !hasFingerprint}
      <p class="text-xs text-slate-600 mt-3">
        {$t('settings.security.enroll_hint').split('{cmd}')[0]}<code class="bg-slate-900 px-1.5 py-0.5 rounded">fprintd-enroll {username}</code>{$t('settings.security.enroll_hint').split('{cmd}')[1].split('{pkg}')[0]}<code class="bg-slate-900 px-1.5 py-0.5 rounded">fprintd</code>{$t('settings.security.enroll_hint').split('{pkg}')[1]}
      </p>
    {/if}
  </div>
</div>
