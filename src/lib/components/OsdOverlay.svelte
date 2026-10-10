<script lang="ts">
  /**
   * On-screen display for volume and brightness — modelled on KDE Plasma's OSD:
   * a small rounded glass panel at the bottom centre of the screen with a
   * level glyph, a slim progress bar and the percentage. It appears when the
   * level changes (see stores/osd.ts for where the changes come from) and fades
   * out by itself; it never takes focus or intercepts the pointer.
   */
  import { fly, fade } from 'svelte/transition';
  import { Volume, Volume1, Volume2, VolumeX, Sun, SunDim, SunMedium } from 'lucide-svelte';
  import { osd, volumeIconKind, brightnessIconKind, barFill } from '../stores/osd';
  import { t } from '../stores/language';

  /** Above every window / overlay, see App.svelte's `startMenuZIndex`. */
  export let zIndex = 9400;
  /** Space the top bar takes at the bottom edge (0 when it sits at the top). */
  export let bottomInset = 0;

  const reduceMotion = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  const VOLUME_ICONS = { muted: VolumeX, low: Volume, medium: Volume1, high: Volume2 } as const;
  const BRIGHTNESS_ICONS = { dim: SunDim, medium: SunMedium, full: Sun } as const;

  $: isVolume = $osd.kind === 'volume';
  $: icon = isVolume ? VOLUME_ICONS[volumeIconKind($osd.value, $osd.muted)] : BRIGHTNESS_ICONS[brightnessIconKind($osd.value)];
  $: fill = barFill($osd.value, isVolume && $osd.muted);
  $: amplified = isVolume && !$osd.muted && $osd.value > 100;
  $: label = isVolume ? $t('osd.volume') : $t('osd.brightness');
  $: valueText = isVolume && $osd.muted ? $t('osd.muted') : `${$osd.value}%`;
</script>

{#if $osd.visible}
  <div
    class="osd"
    role="status"
    aria-live="polite"
    aria-label="{label}: {valueText}"
    data-shell-occluder="osd"
    style="z-index:{zIndex}; bottom:calc({bottomInset}px + 72px);"
    in:fly={{ y: reduceMotion ? 0 : 14, duration: reduceMotion ? 0 : 140 }}
    out:fade={{ duration: reduceMotion ? 0 : 200 }}
  >
    <span class="glyph" class:dim={isVolume && $osd.muted} aria-hidden="true">
      <svelte:component this={icon} size={22} strokeWidth={1.9} />
    </span>
    <span class="track" aria-hidden="true">
      <span class="fill" class:amplified style="width:{fill}%;"></span>
    </span>
    <span class="value" class:dim={isVolume && $osd.muted} class:amplified>{valueText}</span>
  </div>
{/if}

<style>
  .osd {
    position: absolute;
    /* Centred with auto margins, not translateX(-50%): the fly-in transition
       writes its own `transform` and would otherwise knock it off-centre. */
    left: 0;
    right: 0;
    margin: 0 auto;
    display: flex;
    align-items: center;
    gap: 14px;
    box-sizing: border-box;
    width: min(340px, calc(100vw - 32px));
    height: 56px;
    padding: 0 20px 0 18px;
    border-radius: 14px;
    color: #e2e8f0;
    background: rgba(15, 23, 42, 0.9);
    -webkit-backdrop-filter: blur(20px) saturate(1.2);
    backdrop-filter: blur(20px) saturate(1.2);
    border: 1px solid rgba(255, 255, 255, 0.09);
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5), 0 2px 8px rgba(0, 0, 0, 0.35);
    /* Purely informative: clicks go straight through to whatever is below. */
    pointer-events: none;
    user-select: none;
  }
  .glyph {
    display: inline-flex;
    flex: none;
    width: 24px;
    justify-content: center;
    color: #f1f5f9;
    transition: color 120ms ease;
  }
  .glyph.dim { color: #94a3b8; }
  .track {
    position: relative;
    flex: 1;
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, #3b82f6, #60a5fa);
    transition: width 90ms linear;
  }
  .fill.amplified { background: linear-gradient(90deg, #f59e0b, #fbbf24); }
  .value {
    flex: none;
    min-width: 46px;
    text-align: right;
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: #f1f5f9;
  }
  .value.dim { color: #94a3b8; font-weight: 500; }
  .value.amplified { color: #fbbf24; }
  @media (prefers-reduced-motion: reduce) {
    .fill, .glyph { transition: none; }
  }
</style>
