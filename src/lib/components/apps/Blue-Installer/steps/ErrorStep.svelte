<script lang="ts">
  import { XCircle, RotateCcw } from 'lucide-svelte';
  import type { InstallState } from '../installState';
  export let state: InstallState;
  const { installError } = state;
</script>

<div class="flex-1 flex flex-col items-center justify-center px-12 gap-6 text-center">
  <div class="w-20 h-20 rounded-full bg-red-500/15 border-2 border-red-500/30 flex items-center justify-center">
    <XCircle size={40} class="text-red-400" />
  </div>
  <div>
    <h2 class="text-2xl font-bold text-white mb-2">Installation failed</h2>
    <p class="text-slate-400 max-w-md">{$installError ?? 'An unknown error occurred.'}</p>
  </div>
  <button on:click={() => state.step.set('summary')} class="flex items-center gap-2 px-6 py-3 bg-slate-700 hover:bg-slate-600 rounded-xl text-white font-medium transition-colors">
    <RotateCcw size={16} /> Back to Summary
  </button>
</div>
