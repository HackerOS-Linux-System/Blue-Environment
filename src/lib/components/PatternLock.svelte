<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';

  interface Props {
    disabled?: boolean;
    error?: boolean;
  }

  let { disabled = false, error = false }: Props = $props();

  const dispatch = createEventDispatcher<{ complete: number[] }>();

  let svgEl = $state() as SVGSVGElement;
  let dots = $state<{ x: number; y: number }[]>([]);
  let path = $state<number[]>([]);
  let dragging = $state(false);
  let cursor = $state({ x: 0, y: 0 });

  const SIZE = 260;
  const PAD = 40;
  const STEP = (SIZE - PAD * 2) / 2;

  onMount(() => {
    dots = Array.from({ length: 9 }, (_, i) => ({ x: PAD + (i % 3) * STEP, y: PAD + Math.floor(i / 3) * STEP }));
  });

  function nearestDot(x: number, y: number): number | null {
    for (let i = 0; i < dots.length; i++) {
      const d = dots[i];
      if (Math.hypot(d.x - x, d.y - y) < 24) return i;
    }
    return null;
  }

  function getLocalPoint(e: PointerEvent): { x: number; y: number } {
    const rect = svgEl.getBoundingClientRect();
    return { x: ((e.clientX - rect.left) / rect.width) * SIZE, y: ((e.clientY - rect.top) / rect.height) * SIZE };
  }

  function handlePointerDown(e: PointerEvent) {
    if (disabled) return;
    const p = getLocalPoint(e);
    const idx = nearestDot(p.x, p.y);
    path = idx !== null ? [idx] : [];
    dragging = true;
    cursor = p;
    svgEl.setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging || disabled) return;
    const p = getLocalPoint(e);
    cursor = p;
    const idx = nearestDot(p.x, p.y);
    if (idx !== null && !path.includes(idx)) path = [...path, idx];
  }

  function handlePointerUp() {
    if (!dragging) return;
    dragging = false;
    if (path.length >= 4) dispatch('complete', path);
    else path = [];
  }

  let lineColor = $derived(error ? '#ef4444' : '#3b82f6');
</script>

<svg bind:this={svgEl} viewBox="0 0 {SIZE} {SIZE}" width={SIZE} height={SIZE}
  class="touch-none select-none {disabled ? 'opacity-50' : ''}"
  onpointerdown={handlePointerDown} onpointermove={handlePointerMove} onpointerup={handlePointerUp} onpointercancel={handlePointerUp}>

  {#each path.slice(0, -1) as idx, i (i)}
    <line x1={dots[idx]?.x} y1={dots[idx]?.y} x2={dots[path[i + 1]]?.x} y2={dots[path[i + 1]]?.y}
      stroke={lineColor} stroke-width="4" stroke-linecap="round" opacity="0.85" />
  {/each}
  {#if dragging && path.length > 0}
    <line x1={dots[path[path.length - 1]]?.x} y1={dots[path[path.length - 1]]?.y} x2={cursor.x} y2={cursor.y}
      stroke={lineColor} stroke-width="4" stroke-linecap="round" opacity="0.5" />
  {/if}

  {#each dots as dot, i (i)}
    {@const active = path.includes(i)}
    <circle cx={dot.x} cy={dot.y} r={active ? 10 : 8}
      fill={active ? lineColor : 'rgba(255,255,255,0.08)'}
      stroke={active ? lineColor : 'rgba(255,255,255,0.25)'} stroke-width="2"
      style="transition:r 0.1s ease, fill 0.1s ease;" />
    {#if active}<circle cx={dot.x} cy={dot.y} r="16" fill="none" stroke={lineColor} stroke-width="1.5" opacity="0.35" />{/if}
  {/each}
</svg>
