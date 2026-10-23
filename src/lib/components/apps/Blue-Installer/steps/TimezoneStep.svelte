<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Check, Clock } from 'lucide-svelte';
  import { TIMEZONES } from '../types';
  import type { InstallState } from '../installState';

  interface Props {
    state: InstallState;
  }

  let { state }: Props = $props();
  const { config } = state;

  let now = new Date();
  let timer: ReturnType<typeof setInterval>;
  onMount(() => { timer = setInterval(() => (now = new Date()), 1000); });
  onDestroy(() => clearInterval(timer));

  function select(zone: string) {
    state.markTimezoneTouched();
    config.update((c) => ({ ...c, timezone: zone }));
  }

  function previewTime(zone: string): string {
    try {
      return now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', timeZone: zone });
    } catch { return '--:--'; }
  }
</script>

<div class="flex-1 flex flex-col px-10 py-8 overflow-y-auto">
  <div class="flex items-center gap-2 mb-2"><Clock size={20} class="text-blue-400" /><h2 class="text-xl font-semibold text-white">Choose your timezone</h2></div>
  <p class="text-sm text-slate-500 mb-6">
    Currently selected: <span class="text-white font-medium">{$config.timezone}</span>
    — {previewTime($config.timezone)}
  </p>

  <div class="space-y-5 max-w-2xl">
    {#each TIMEZONES as group (group.region)}
      <div>
        <div class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-2">{group.region}</div>
        <div class="grid grid-cols-2 gap-2">
          {#each group.zones as zone (zone)}
            <button onclick={() => select(zone)}
              class="flex items-center justify-between px-3.5 py-2 rounded-xl border text-sm transition-colors {$config.timezone === zone ? 'bg-blue-600/15 border-blue-500/40 text-white' : 'bg-slate-800/50 border-white/5 text-slate-300 hover:bg-white/5'}">
              <span class="truncate">{zone.replace(/_/g, ' ')}</span>
              <span class="flex items-center gap-1.5 shrink-0 text-xs text-slate-500">
                {previewTime(zone)}
                {#if $config.timezone === zone}<Check size={13} class="text-blue-400" />{/if}
              </span>
            </button>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</div>
