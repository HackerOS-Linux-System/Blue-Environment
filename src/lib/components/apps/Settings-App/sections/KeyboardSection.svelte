<script lang="ts">
  import { Keyboard, Info } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import type { UserConfig } from '../../../../types';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  $: enabled = config.onscreenKeyboardEnabled ?? false;
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.tab.keyboard')}</h2>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 flex items-center gap-4">
    <div class="w-11 h-11 rounded-2xl bg-blue-500/15 flex items-center justify-center shrink-0">
      <Keyboard size={20} class="text-blue-400" />
    </div>
    <div class="flex-1">
      <div class="text-white font-medium">{$t('settings.keyboard.toggle_label')}</div>
      <p class="text-xs text-slate-500 mt-0.5">{$t('settings.keyboard.toggle_desc')}</p>
    </div>
    <button
      role="switch"
      aria-checked={enabled}
      on:click={() => onSave({ onscreenKeyboardEnabled: !enabled })}
      class="w-12 h-7 rounded-full relative transition-colors shrink-0 {enabled ? 'bg-blue-600' : 'bg-white/10'}">
      <span class="absolute top-1 w-5 h-5 rounded-full bg-white transition-all {enabled ? 'left-6' : 'left-1'}" />
    </button>
  </div>

  <div class="bg-slate-800/50 p-4 rounded-xl border border-white/5 flex items-start gap-3">
    <Info size={16} class="text-blue-400 shrink-0 mt-0.5" />
    <p class="text-xs text-slate-400 leading-relaxed">{$t('settings.keyboard.limitation_note')}</p>
  </div>
</div>
