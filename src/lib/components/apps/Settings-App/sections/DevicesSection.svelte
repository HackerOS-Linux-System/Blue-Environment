<script lang="ts">
  import type { UserConfig } from '../../../../types';
  import { t } from '../../../../stores/language';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  // Oba przełączniki są domyślnie włączone (brak wartości = włączone).
  $: notify = config.deviceNotifications !== false;
  $: progress = config.showTransferProgress !== false;
  const onNotify = (e: Event) => onSave({ deviceNotifications: (e.currentTarget as HTMLInputElement).checked });
  const onProgress = (e: Event) => onSave({ showTransferProgress: (e.currentTarget as HTMLInputElement).checked });
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('sdev.title')}</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <label class="flex items-center justify-between text-sm text-slate-200">
      <span>{$t('sdev.notify')}<span class="block text-xs text-slate-500">{$t('sdev.notify_hint')}</span></span>
      <input type="checkbox" checked={notify} on:change={onNotify} />
    </label>
    <label class="flex items-center justify-between text-sm text-slate-200">
      <span>{$t('sdev.progress')}</span>
      <input type="checkbox" checked={progress} on:change={onProgress} />
    </label>
  </div>
</div>
