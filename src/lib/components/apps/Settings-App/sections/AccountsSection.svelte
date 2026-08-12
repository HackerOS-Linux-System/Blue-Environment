<script lang="ts">
  import type { UserConfig } from '../../../../types';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  let showLogin = false;

  async function handleGoogle() {
    const result = await SystemBridge.googleSignIn();
    if (result) { await onSave({ accounts: { ...config.accounts, google: result.user } }); showLogin = false; }
  }

  function googleEmail(): string {
    return (config.accounts?.google as { email?: string } | undefined)?.email ?? '';
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.accounts.title')}</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    {#if !config.accounts?.google}
      <button on:click={() => (showLogin = true)} class="w-full flex items-center justify-center gap-2 bg-white text-slate-900 p-3 rounded-lg hover:bg-slate-100 transition-colors">
        <img src="https://www.google.com/favicon.ico" class="w-5 h-5" alt="Google" />
        {$t('settings.accounts.sign_in_google')}
      </button>
    {/if}
    {#if config.accounts?.google}
      <div class="flex items-center justify-between py-2">
        <div class="flex items-center gap-2 text-sm text-slate-300">
          <img src="https://www.google.com/favicon.ico" class="w-4 h-4" alt="" />
          {googleEmail()}
        </div>
        <button on:click={() => onSave({ accounts: { ...config.accounts, google: undefined } })} class="text-xs text-red-400 hover:underline">{$t('settings.common.remove')}</button>
      </div>
    {/if}
  </div>

  {#if showLogin}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div class="bg-slate-800 rounded-2xl p-6 w-96 border border-white/10">
        <h3 class="text-lg font-bold text-white mb-4">{$t('settings.accounts.sign_in_google')}</h3>
        <p class="text-sm text-slate-300 mb-4">{$t('settings.accounts.redirect_notice')}</p>
        <div class="flex justify-end gap-2">
          <button on:click={() => (showLogin = false)} class="px-4 py-2 bg-slate-700 rounded-lg hover:bg-slate-600">{$t('settings.common.cancel')}</button>
          <button on:click={handleGoogle} class="px-4 py-2 bg-blue-600 rounded-lg hover:bg-blue-500">{$t('settings.accounts.continue')}</button>
        </div>
      </div>
    </div>
  {/if}
</div>
