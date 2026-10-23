<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Wifi, BatteryFull, BatteryCharging } from 'lucide-svelte';
  import { SystemBridge, type BatteryStatus } from '../lib/utils/systemBridge';
  import { formatClock, formatDate } from '../lib/altShell/shellUtils';

  interface Props {
    now: Date;
    use24h?: boolean;
  }

  let { now, use24h = true }: Props = $props();

  let battery = $state<BatteryStatus | null>(null);
  let timer: ReturnType<typeof setInterval>;
  async function refresh() { try { battery = await SystemBridge.getBatteryStatus(); } catch { /* keep last */ } }
  onMount(() => { refresh(); timer = setInterval(refresh, 30000); });
  onDestroy(() => clearInterval(timer));
</script>

<div class="flex items-center justify-between px-12 pt-8 text-white">
  <div>
    <div class="text-5xl font-light tabular-nums">{formatClock(now, use24h)}</div>
    <div class="text-lg text-white/70 capitalize">{formatDate(now)}</div>
  </div>
  <div class="flex items-center gap-5 text-white/80">
    <Wifi size={26} />
    {#if battery?.present}
      <span class="flex items-center gap-2 text-xl tabular-nums">
        {#if battery.charging}<BatteryCharging size={26} />{:else}<BatteryFull size={26} />{/if}{Math.round(battery.percentage)}%
      </span>
    {/if}
  </div>
</div>
