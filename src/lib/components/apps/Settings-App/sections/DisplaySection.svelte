<script lang="ts">
  import { Image as ImageIcon, RefreshCw, Check } from 'lucide-svelte';
  import type { UserConfig } from '../../../../types';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { applyResolution, applyRefreshRate } from '../display_helpers';

  export let config: UserConfig;
  export let onSave: (patch: Partial<UserConfig>) => Promise<void>;
  export let wallpapers: string[];
  export let wallpaperPreviews: Map<string, string>;
  export let onReloadWallpapers: () => void;
  export let brightness: number;
  export let resolution: string;
  export let refreshRate: number;
  export let resolutionList: string[];
  export let rateList: number[];

  async function handleResolutionChange(e: Event) {
    resolution = (e.currentTarget as HTMLSelectElement).value;
    await applyResolution(resolution);
  }
  async function handleRateChange(e: Event) {
    refreshRate = parseInt((e.currentTarget as HTMLSelectElement).value);
    await applyRefreshRate(refreshRate);
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">Display</h2>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <label class="text-sm font-medium text-slate-400 mb-4 flex items-center gap-2"><ImageIcon size={16} class="text-blue-400" /> Wallpaper</label>
    <div class="grid grid-cols-3 gap-4 max-h-72 overflow-y-auto p-1">
      {#each wallpapers as wp, idx (idx)}
        <div class="relative aspect-video rounded-xl overflow-hidden cursor-pointer border-2 transition-all hover:scale-105 {config.wallpaper === wp ? 'border-blue-500 ring-2 ring-blue-500/20' : 'border-transparent'}"
             on:click={() => onSave({ wallpaper: wp })}>
          {#if wallpaperPreviews.has(wp)}
            <img src={wallpaperPreviews.get(wp)} class="w-full h-full object-cover" alt="" />
          {:else}
            <div class="w-full h-full bg-slate-700 flex items-center justify-center"><ImageIcon size={24} class="text-slate-500" /></div>
          {/if}
          {#if config.wallpaper === wp}
            <div class="absolute inset-0 flex items-center justify-center bg-blue-500/20"><Check size={24} class="text-white" /></div>
          {/if}
        </div>
      {/each}
    </div>
    <button on:click={onReloadWallpapers} class="mt-3 text-xs text-slate-500 hover:text-white flex items-center gap-1"><RefreshCw size={12} /> Refresh wallpapers</button>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <label class="block text-sm font-medium text-slate-400 mb-2">Brightness</label>
    <input type="range" min="0" max="100" bind:value={brightness} on:mouseup={() => SystemBridge.setBrightness(brightness)}
      class="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500" />
    <div class="flex justify-between text-xs text-slate-500 mt-1"><span>0%</span><span>{brightness}%</span><span>100%</span></div>
  </div>

  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
    <label class="block text-sm font-medium text-slate-400 mb-2">Display Scale</label>
    <select value={config.displayScale} on:change={(e) => onSave({ displayScale: parseFloat(e.currentTarget.value) })}
      class="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
      {#each ['1', '1.25', '1.5', '1.75', '2'] as s (s)}<option value={s}>{parseFloat(s) * 100}%</option>{/each}
    </select>
  </div>

  <div class="grid grid-cols-2 gap-4">
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
      <label class="block text-sm font-medium text-slate-400 mb-2">Resolution</label>
      <select value={resolution} on:change={handleResolutionChange} class="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
        {#each resolutionList as r (r)}<option>{r}</option>{/each}
      </select>
    </div>
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5">
      <label class="block text-sm font-medium text-slate-400 mb-2">Refresh Rate</label>
      <select value={refreshRate} on:change={handleRateChange} class="w-full bg-slate-900 border border-white/10 rounded-lg px-4 py-2 text-white focus:outline-none">
        {#each rateList as r (r)}<option value={r}>{r} Hz</option>{/each}
      </select>
    </div>
  </div>
</div>
