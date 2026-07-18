<script lang="ts">
  import { onMount } from 'svelte';
  import { Info, ExternalLink } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';

  interface DistroInfo { Name?: string; Version?: string; Copyright?: string; [key: string]: any; }

  let distro: DistroInfo = {};
  let username = '';

  onMount(() => {
    SystemBridge.getDistroInfo().then((d: DistroInfo) => (distro = d)).catch(() => {});
    try { username = SystemBridge.getUsername(); } catch {}
  });
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-white">About</h2>
  <div class="bg-slate-800 p-6 rounded-2xl border border-white/5 space-y-4">
    <div class="flex items-center gap-4">
      <div class="w-14 h-14 rounded-2xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-lg shadow-blue-500/20">
        <Info size={24} class="text-white" />
      </div>
      <div>
        <div class="text-lg font-semibold text-white">{distro.Name || 'Blue Environment'}</div>
        <div class="text-sm text-slate-400">Version {distro.Version || '0.6'}</div>
      </div>
    </div>
    <div class="grid grid-cols-2 gap-3 text-sm pt-2">
      <div class="bg-slate-900/50 rounded-lg p-3">
        <div class="text-slate-500 text-xs mb-1">Signed in as</div>
        <div class="font-medium text-white">{username || 'unknown'}</div>
      </div>
      <div class="bg-slate-900/50 rounded-lg p-3">
        <div class="text-slate-500 text-xs mb-1">Shell</div>
        <div class="font-medium text-white">Blue Shell (Svelte)</div>
      </div>
    </div>
    {#if distro.Copyright}<p class="text-xs text-slate-500 pt-2">{distro.Copyright}</p>{/if}
    <a href="https://github.com/LegendaryOS-Linux-System/Blue-Environment" target="_blank" rel="noreferrer"
       class="inline-flex items-center gap-1.5 text-xs text-blue-400 hover:text-blue-300 transition-colors">
      Project page <ExternalLink size={11} />
    </a>
  </div>
</div>
