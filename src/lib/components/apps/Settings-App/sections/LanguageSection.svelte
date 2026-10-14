<script lang="ts">
  import { Check, Languages } from 'lucide-svelte';
  import { language, t, setLanguage, SUPPORTED_LANGUAGES, type Language } from '../../../../stores/language';
  import UserFoldersCard from './UserFoldersCard.svelte';
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.language')}</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-1">
    <div class="flex items-center gap-3 text-slate-400 mb-4">
      <Languages size={16} class="text-blue-400" />
      <span class="text-sm">{$t('settings.select_language')}</span>
    </div>
    <div class="grid grid-cols-2 gap-2">
      {#each SUPPORTED_LANGUAGES as lang (lang.code)}
        <button on:click={() => setLanguage(lang.code)}
          class="flex items-center justify-between px-4 py-3 rounded-xl border transition-all {$language === lang.code ? 'bg-blue-600/15 border-blue-500/40 text-white' : 'bg-slate-900/40 border-white/5 text-slate-300 hover:bg-white/5'}">
          <span class="flex items-center gap-3">
            <span class="text-xs font-mono bg-slate-700 px-1.5 py-0.5 rounded text-slate-300 min-w-[28px] text-center">{lang.flag}</span>
            <span class="text-left">
              <div class="text-sm">{lang.nativeName}</div>
              <div class="text-xs text-slate-500">{lang.name}</div>
            </span>
          </span>
          {#if $language === lang.code}<Check size={14} class="text-blue-400 shrink-0" />{/if}
        </button>
      {/each}
    </div>
    <p class="text-xs text-slate-600 mt-4 pt-3 border-t border-white/5">{$t('settings.language_restart_hint')}</p>
  </div>

  <UserFoldersCard />
</div>
