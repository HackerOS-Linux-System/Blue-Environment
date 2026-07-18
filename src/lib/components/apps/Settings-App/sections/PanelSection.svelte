<script lang="ts">
  import { PanelTop } from 'lucide-svelte';
  import type { UserConfig } from '../../../../types';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">Panel</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-6">
    <div class="flex items-center gap-3 text-slate-400">
      <PanelTop size={16} class="text-blue-400" />
      <span class="text-sm">Appearance of the top bar — applies live.</span>
    </div>
    <div>
      <label class="block text-sm font-medium text-slate-400 mb-1">Height — {config.panelSize ?? 48}px</label>
      <input type="range" min="36" max="64" step="1" value={config.panelSize ?? 48}
        on:change={(e) => onSave({ panelSize: parseInt(e.currentTarget.value, 10) })}
        class="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
    </div>
    <div>
      <label class="block text-sm font-medium text-slate-400 mb-1">Opacity — {Math.round((config.panelOpacity ?? 0.95) * 100)}%</label>
      <input type="range" min="0.5" max="1" step="0.05" value={config.panelOpacity ?? 0.95}
        on:change={(e) => onSave({ panelOpacity: parseFloat(e.currentTarget.value) })}
        class="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
    </div>
    <p class="text-xs text-slate-500">Window snapping is calibrated for the default 48px height — larger or smaller values may shift the snap zones slightly.</p>
  </div>
</div>
