<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from 'svelte';
  import { Wifi, BatteryFull, BatteryCharging, BatteryLow, Bell } from 'lucide-svelte';
  import { SystemBridge, type BatteryStatus } from '../lib/utils/systemBridge';
  import { classifySwipe } from '../lib/altShell/gestures';
  import { formatClock } from '../lib/altShell/shellUtils';

  export let now: Date;
  export let use24h = true;
  export let unread = 0;

  const dispatch = createEventDispatcher<{ pull: void }>();

  let battery: BatteryStatus | null = null;
  let timer: ReturnType<typeof setInterval>;
  async function refresh() { try { battery = await SystemBridge.getBatteryStatus(); } catch { /* keep last */ } }
  onMount(() => { refresh(); timer = setInterval(refresh, 30000); });
  onDestroy(() => clearInterval(timer));

  // A tap or a downward drag on the bar pulls the shade down.
  let startX = 0; let startY = 0;
  function down(e: PointerEvent) { startX = e.clientX; startY = e.clientY; }
  function up(e: PointerEvent) {
    const dx = e.clientX - startX; const dy = e.clientY - startY;
    const swipe = classifySwipe(dx, dy, { minDistance: 24 });
    if (swipe === 'down' || (Math.abs(dx) < 10 && Math.abs(dy) < 10)) dispatch('pull');
  }

  $: lowBattery = battery?.present && battery.percentage <= 15 && !battery.charging;
</script>

<div role="button" tabindex="0" aria-label="Status bar"
  class="phone-safe-top relative z-30 shrink-0 bg-black/30 backdrop-blur-md text-white"
  on:pointerdown={down} on:pointerup={up}
  on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && dispatch('pull')}>
  <div class="h-8 px-4 flex items-center justify-between text-xs font-medium tabular-nums">
    <span>{formatClock(now, use24h)}</span>
    <span class="flex items-center gap-2">
      {#if unread > 0}<span class="flex items-center gap-0.5"><Bell size={12} />{unread}</span>{/if}
      <Wifi size={13} />
      {#if battery?.present}
        <span class="flex items-center gap-1 {lowBattery ? 'text-red-400' : ''}">
          {#if battery.charging}<BatteryCharging size={14} />{:else if lowBattery}<BatteryLow size={14} />{:else}<BatteryFull size={14} />{/if}
          {Math.round(battery.percentage)}%
        </span>
      {/if}
    </span>
  </div>
</div>
