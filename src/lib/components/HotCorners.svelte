<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';
  import { cornerAt, type HotCornersConfig, type HotCornerId, type HotCornerSlot } from '../utils/hotCorners';

  export let config: HotCornersConfig;

  const dispatch = createEventDispatcher<{ trigger: HotCornerSlot & { corner: HotCornerId } }>();

  let armed: HotCornerId | null = null;     // róg, w którym stoi kursor (odlicza czas)
  let fired: HotCornerId | null = null;     // róg już wyzwolony — wymaga wyjścia z niego
  let dwellTimer: ReturnType<typeof setTimeout> | undefined;
  let lastFire = 0;

  function clear() { clearTimeout(dwellTimer); dwellTimer = undefined; armed = null; }

  function onMove(e: MouseEvent) {
    if (!config.enabled) { if (armed) clear(); return; }
    const corner = cornerAt(e.clientX, e.clientY, window.innerWidth, window.innerHeight, config.triggerSize);
    if (corner !== armed) {
      clear();
      if (corner !== fired) fired = null;      // opuszczenie rogu zwalnia blokadę
      if (!corner) { fired = null; return; }
      if (corner === fired) return;
      const slot = config.corners[corner];
      if (!slot || slot.action === 'none') return;
      armed = corner;
      dwellTimer = setTimeout(() => {
        const now = Date.now();
        if (armed === corner && now - lastFire >= config.cooldownMs) {
          lastFire = now;
          fired = corner;
          dispatch('trigger', { ...slot, corner });
        }
        armed = null;
      }, config.dwellMs);
    }
  }

  function onLeaveWindow() { clear(); fired = null; }

  onMount(() => {
    window.addEventListener('mousemove', onMove, { passive: true, capture: true });
    document.documentElement.addEventListener('mouseleave', onLeaveWindow);
  });
  onDestroy(() => {
    clear();
    window.removeEventListener('mousemove', onMove, { capture: true } as any);
    document.documentElement.removeEventListener('mouseleave', onLeaveWindow);
  });

  const pos: Record<HotCornerId, string> = {
    topLeft: 'top-0 left-0 rounded-br-full',
    topRight: 'top-0 right-0 rounded-bl-full',
    bottomLeft: 'bottom-0 left-0 rounded-tr-full',
    bottomRight: 'bottom-0 right-0 rounded-tl-full',
  };
</script>

<!-- Delikatna poświata pokazuje, że róg jest "naładowany" (kursor dociska). -->
{#if config.enabled && armed}
  <div class="pointer-events-none fixed z-[9999] w-16 h-16 bg-blue-500/30 blur-md {pos[armed]} animate-pulse" />
{/if}
