<script lang="ts">
  import type { UserConfig } from '../../../../types';
  import { AppId } from '../../../../types';
  import { APPS } from '../../../../constants';
  import { t } from '../../../../stores/language';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  // Previously this was a fixed, hand-maintained list of 12 apps — every
  // internal app added since (Blue Screenshot, Blue Archive, Blue
  // Translate, Blue Partition Manager, Blue Images/Video...) had no way
  // to be disabled here at all. This now enumerates every real internal
  // app straight from the same `APPS` registry the shell itself launches
  // from, so the two can never drift out of sync again. Settings itself
  // is excluded — disabling it would lock the user out of this very
  // screen with no way back in short of editing the config file by hand.
  $: appList = Object.values(APPS)
    .filter((app) => !app.isExternal && app.component && app.id !== AppId.SETTINGS)
    .sort((a, b) => a.title.localeCompare(b.title));

  // Keyed directly by AppId (e.g. `ai_assistant`) — matches
  // StartMenu.svelte's filter exactly, which is also what makes a
  // disabled app disappear from the app menu entirely instead of still
  // being launchable and showing a "blocked" notification.
  function isAppEnabled(id: string): boolean {
    const appsEnabled = config.appsEnabled as Record<string, boolean> | undefined;
    return appsEnabled?.[id] ?? true;
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.apps.title')}</h2>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <span class="block text-sm font-medium text-white mb-1">{$t('settings.apps.default_text_editor')}</span>
    <p class="text-xs text-slate-500 mb-3">{$t('settings.apps.default_text_editor_desc')}</p>
    <div class="flex gap-2">
      <button on:click={() => onSave({ defaultTextEditor: 'notepad' })}
        class="flex-1 py-2 rounded-xl text-sm font-medium border transition-colors {(config.defaultTextEditor ?? 'notepad') === 'notepad' ? 'bg-blue-600 border-blue-500 text-white' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
        {$t('settings.apps.notepad')}
      </button>
      <button on:click={() => onSave({ defaultTextEditor: 'blue_code' })}
        class="flex-1 py-2 rounded-xl text-sm font-medium border transition-colors {config.defaultTextEditor === 'blue_code' ? 'bg-blue-600 border-blue-500 text-white' : 'border-white/10 text-slate-400 hover:bg-white/5'}">
        {$t('settings.apps.blue_code')}
      </button>
    </div>
  </div>

  <p class="text-sm text-slate-400 -mt-2">{$t('settings.apps.toggle_hint')}</p>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-1">
    {#each appList as app (app.id)}
      {@const id = String(app.id)}
      <div class="flex items-center gap-3 py-2.5 border-b border-white/5 last:border-0">
        <svelte:component this={app.icon} size={17} class="text-slate-400 shrink-0" />
        <span class="text-white flex-1">{app.title}</span>
        <input type="checkbox" checked={isAppEnabled(id)}
          on:change={(e) => onSave({ appsEnabled: { ...config.appsEnabled, [id]: e.currentTarget.checked } })}
          class="w-4 h-4 accent-blue-500" />
      </div>
    {/each}
  </div>
</div>
