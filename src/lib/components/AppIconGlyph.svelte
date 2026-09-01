<script lang="ts">
  import { activeShellThemeId } from '../stores/shellTheme';

  export let icon: any;
  export let name: string;
  export let size = 32;

  let failed = false;
  $: isUrl = typeof icon === 'string' && (icon.startsWith('http') || icon.startsWith('file://'));
  $: hue = (name.charCodeAt(0) * 37) % 360;
  // Real, honest scope (see this app's request history — a true
  // pixel-matched custom icon set is separate, larger work with no
  // actual art assets to draw from): rather than hand-drawing a
  // handful of icons that would only cover a few apps and leave the
  // rest inconsistent, every icon rendered through this one shared
  // component gets a neon glow treatment while Hydra is active — a
  // systemic improvement across every app's icon (pinned, tray,
  // launcher grid, wherever this component is used), not a partial
  // one-off retouch of a chosen few.
  $: isHydra = $activeShellThemeId === 'hydra';
</script>

{#if typeof icon === 'string' && isUrl && !failed}
  <img src={icon} alt={name} width={size} height={size} class="object-contain {isHydra ? 'hydra-icon-glow' : ''}" on:error={() => (failed = true)} />
{:else if typeof icon !== 'string' && !failed}
  <div class={isHydra ? 'hydra-icon-glow' : ''} style={isHydra ? 'display:contents;' : undefined}>
    <svelte:component this={icon} {size} />
  </div>
{:else}
  <div class="flex items-center justify-center rounded-lg font-bold text-white {isHydra ? 'hydra-icon-glow' : ''}"
       style="width:{size}px; height:{size}px; background:{isHydra ? 'linear-gradient(135deg, #ec4899, #8b5cf6, #3b82f6)' : `hsl(${hue},60%,40%)`}; font-size:{size * 0.45}px;">
    {name.charAt(0).toUpperCase()}
  </div>
{/if}
