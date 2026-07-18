<script lang="ts">
  import { CheckCircle2, RotateCw } from 'lucide-svelte';
  import type { InstallState } from '../installState';
  export let state: InstallState;
</script>

<div class="flex-1 flex flex-col items-center justify-center px-12 gap-6 text-center">
  <div class="w-20 h-20 rounded-full bg-green-500/15 border-2 border-green-500/30 flex items-center justify-center">
    <CheckCircle2 size={40} class="text-green-400" />
  </div>
  <div>
    <h2 class="text-2xl font-bold text-white mb-2">Installation complete</h2>
    <p class="text-slate-400 max-w-sm">Blue Environment has been installed. Restart your computer to boot into your new system.</p>
  </div>
  <button on:click={state.finishAndReboot} class="flex items-center gap-2 px-6 py-3 bg-blue-600 hover:bg-blue-500 rounded-xl text-white font-medium transition-colors">
    <RotateCw size={16} /> Restart Now
  </button>
</div>
