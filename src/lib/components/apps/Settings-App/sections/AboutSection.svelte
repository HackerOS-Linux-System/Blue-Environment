<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Info, ExternalLink, Cpu } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { CompositorBridge } from '../../../../utils/compositorBridge';
  import { t } from '../../../../stores/language';

  interface DistroInfo { Name?: string; Version?: string; Copyright?: string; [key: string]: any; }
  interface GpuEntry { node: string; primary: boolean; output_count: number; }

  let distro: DistroInfo = {};
  let username = '';
  // Real GPU inventory from the compositor's GpuManager — see
  // compositorBridge.ts's onGpuList doc comment. Empty until the
  // compositor actually sends it (udev backend only; stays empty when
  // running nested/dev under winit, which has no real DRM nodes).
  let gpus: GpuEntry[] = [];
  let unsubGpuPromise: Promise<() => void> | undefined;

  onMount(() => {
    SystemBridge.getDistroInfo().then((d: DistroInfo) => (distro = d)).catch(() => {});
    try { username = SystemBridge.getUsername(); } catch {}
    unsubGpuPromise = CompositorBridge.onGpuList((list) => { gpus = list; });
  });

  onDestroy(() => { unsubGpuPromise?.then((fn) => fn()); });
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">{$t('settings.about.title')}</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="flex items-center gap-4">
      <div class="w-14 h-14 rounded-2xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-lg shadow-blue-500/20">
        <Info size={24} class="text-white" />
      </div>
      <div>
        <div class="text-lg font-semibold text-white">{distro.Name || 'Blue Environment'}</div>
        <div class="text-sm text-slate-400">{$t('settings.about.version').replace('{v}', distro.Version || '0.6')}</div>
      </div>
    </div>
    <div class="grid grid-cols-2 gap-3 text-sm pt-2">
      <div class="bg-slate-900/50 rounded-lg p-3">
        <div class="text-slate-500 text-xs mb-1">{$t('settings.about.signed_in_as')}</div>
        <div class="font-medium text-white">{username || $t('settings.about.unknown')}</div>
      </div>
      <div class="bg-slate-900/50 rounded-lg p-3">
        <div class="text-slate-500 text-xs mb-1">{$t('settings.about.shell')}</div>
        <div class="font-medium text-white">{$t('settings.about.shell_value')}</div>
      </div>
    </div>
    {#if distro.Copyright}<p class="text-xs text-slate-500 pt-2">{distro.Copyright}</p>{/if}
    <a href="https://github.com/LegendaryOS-Linux-System/Blue-Environment" target="_blank" rel="noreferrer"
       class="inline-flex items-center gap-1.5 text-xs text-blue-400 hover:text-blue-300 transition-colors">
      {$t('settings.about.project_page')} <ExternalLink size={11} />
    </a>
  </div>

  {#if gpus.length > 0}
    <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-3">
      <div class="flex items-center gap-2 text-white font-semibold">
        <Cpu size={16} /> {$t('settings.about.gpu_title')}
      </div>
      <div class="space-y-2">
        {#each gpus as gpu (gpu.node)}
          <div class="bg-slate-900/50 rounded-lg p-3 flex items-center justify-between">
            <div>
              <div class="text-sm font-medium text-white font-mono">{gpu.node}</div>
              <div class="text-xs text-slate-500">{$t('settings.about.gpu_node')} · {gpu.output_count}</div>
            </div>
            <span class="text-xs px-2 py-1 rounded-full {gpu.primary ? 'bg-blue-600 text-white' : 'bg-slate-700 text-slate-300'}">
              {gpu.primary ? $t('settings.about.gpu_primary') : $t('settings.about.gpu_secondary')}
            </span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
