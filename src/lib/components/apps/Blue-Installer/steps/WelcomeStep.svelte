<script lang="ts">
  import { Sparkles } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';
  const dispatch = createEventDispatcher<{ next: void }>();
</script>

<div class="flex-1 flex flex-col items-center justify-center text-center px-12 gap-6">
  <div class="w-20 h-20 rounded-3xl bg-gradient-to-br from-blue-500 to-indigo-700 flex items-center justify-center shadow-2xl shadow-blue-500/30">
    <Sparkles size={36} class="text-white" />
  </div>
  <div>
    <h1 class="text-3xl font-bold text-white mb-2">Welcome to Blue Installer</h1>
    <p class="text-slate-400 max-w-md">This wizard will install Blue Environment onto this computer's disk. It will take about 10–20 minutes.</p>
  </div>
  <button on:click={() => dispatch('next')} class="px-8 py-3 bg-blue-600 hover:bg-blue-500 rounded-2xl text-white font-medium shadow-lg shadow-blue-500/20 transition-colors">
    Get Started
  </button>
  <p class="text-xs text-slate-600 max-w-sm">You're currently running from a live session. Nothing on your disk is changed until you confirm on the summary screen.</p>
</div>
