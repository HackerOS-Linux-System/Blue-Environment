<script lang="ts">
  /**
   * Window control buttons (minimize/PiP/maximize/close), extracted out
   * of Window.svelte specifically so `windowControlsStyle`/
   * `windowControlsPosition` (see builtinThemes.ts's `ShellThemeLayout`)
   * have somewhere real to plug into — previously those two fields
   * existed on every theme's data but nothing read them at all; the
   * buttons were hardcoded to one fixed look/position regardless of the
   * active theme.
   *
   * ── The four styles ───────────────────────────────────────────────
   * - `windows` (the pre-existing default look, unchanged): square
   *   hover-highlight buttons, right-aligned by default.
   * - `macos`: circular traffic-light dots (red/yellow/green), icons
   *   only appear on hover of the *group* (real macOS behavior — the
   *   dots sit there quietly until you mouse over any of them), left-
   *   aligned by default.
   * - `gnome`: a single, more prominent close button with the other two
   *   noticeably smaller/quieter — matches GNOME's own real visual
   *   hierarchy (close is the button people reach for constantly;
   *   minimize/maximize are secondary).
   * - `minimal`: bare icons, no button background at all even on
   *   hover beyond a subtle color shift — the sparsest option.
   *
   * `position` (`left`/`right`) reorders the whole group but doesn't
   * change which style is used — a `macos`-styled group can still be
   * told to render on the right, etc.; the two are independent axes,
   * matching how `ShellThemeLayout` models them as two separate fields
   * rather than one combined enum.
   *
   * `accentGlow` (optional) — a theme-specific hover-glow color (Hydra
   * passes its pink accent) layered on top of whichever style is
   * active, rather than being its own fifth style. Keeps the glow
   * treatment reusable by any future theme without needing a new
   * `windowControlsStyle` value just to get a colored glow on an
   * otherwise-ordinary button style.
   */
  import { X, Minus, Maximize2, Square, PictureInPicture2 } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';
  import type { WindowControlsStyle, WindowControlsPosition } from '../data/builtinThemes';

  export let isMaximized: boolean;
  export let isPiP: boolean | undefined = false;
  export let style: WindowControlsStyle = 'windows';
  export let position: WindowControlsPosition = 'right';
  export let accentGlow: string | undefined = undefined;

  const dispatch = createEventDispatcher<{ close: void; minimize: void; maximize: void; pip: void }>();

  $: orderClass = position === 'left' ? 'flex-row' : 'flex-row-reverse';
  $: glowStyle = accentGlow ? `--ctl-glow:${accentGlow};` : '';

  // Same "no inline `as` cast in a template expression" issue already
  // fixed once in PluginsSection.svelte (real svelte-check error:
  // "Unexpected token" at exactly an inline `as HTMLElement` cast) —
  // named handlers here instead of inlining the cast in every
  // mouseenter/mouseleave attribute below.
  function glowOn(e: Event) {
    if (accentGlow) (e.currentTarget as HTMLElement).style.boxShadow = `0 0 10px ${accentGlow}`;
  }
  function glowOff(e: Event) {
    if (accentGlow) (e.currentTarget as HTMLElement).style.boxShadow = 'none';
  }
</script>

{#if style === 'macos'}
  <!-- Traffic-light dots. Icons hidden until the group is hovered,
       matching real macOS behavior. -->
  <div class="group flex items-center gap-2 {orderClass}" style={glowStyle}>
    <button on:click={() => dispatch('close')} title="Close (Alt+F4)"
      class="w-3 h-3 rounded-full bg-[#ff5f57] flex items-center justify-center transition-shadow"
      style={accentGlow ? 'box-shadow: 0 0 0 rgba(0,0,0,0);' : ''}
      on:mouseenter={glowOn}
      on:mouseleave={glowOff}>
      <X size={7} class="opacity-0 group-hover:opacity-70 text-black" />
    </button>
    <button on:click={() => dispatch('minimize')} title="Minimize (Super+↓)"
      class="w-3 h-3 rounded-full bg-[#febc2e] flex items-center justify-center">
      <Minus size={7} class="opacity-0 group-hover:opacity-70 text-black" />
    </button>
    <button on:click={() => dispatch('maximize')} title="Maximize (Super+↑)"
      class="w-3 h-3 rounded-full bg-[#28c840] flex items-center justify-center">
      {#if isMaximized}<Square size={6} class="opacity-0 group-hover:opacity-70 text-black" />
      {:else}<Maximize2 size={6} class="opacity-0 group-hover:opacity-70 text-black" />{/if}
    </button>
    <button on:click={() => dispatch('pip')} title="Picture-in-Picture"
      class="w-3 h-3 rounded-full flex items-center justify-center transition-colors {isPiP ? 'bg-blue-400' : 'bg-slate-500'}">
      <PictureInPicture2 size={6} class="opacity-0 group-hover:opacity-70 text-black" />
    </button>
  </div>
{:else if style === 'gnome'}
  <div class="flex items-center gap-1 {orderClass}" style={glowStyle}>
    <button on:click={() => dispatch('minimize')} title="Minimize (Super+↓)"
      class="w-6 h-6 flex items-center justify-center hover:bg-white/10 rounded-full theme-text-secondary hover:text-white transition-colors opacity-70 hover:opacity-100">
      <Minus size={11} />
    </button>
    <button on:click={() => dispatch('maximize')} title="Maximize (Super+↑)"
      class="w-6 h-6 flex items-center justify-center hover:bg-white/10 rounded-full theme-text-secondary hover:text-white transition-colors opacity-70 hover:opacity-100">
      {#if isMaximized}<Square size={10} />{:else}<Maximize2 size={10} />{/if}
    </button>
    <button on:click={() => dispatch('pip')} title="Picture-in-Picture"
      class="w-6 h-6 flex items-center justify-center hover:bg-white/10 rounded-full theme-text-secondary transition-colors opacity-70 hover:opacity-100 {isPiP ? 'text-blue-400' : ''}">
      <PictureInPicture2 size={10} />
    </button>
    <button on:click={() => dispatch('close')} title="Close (Alt+F4)"
      class="w-7 h-7 flex items-center justify-center hover:bg-red-500 rounded-full text-white/90 hover:text-white transition-colors ml-1"
      style={accentGlow ? 'background: rgba(255,255,255,0.08);' : 'background: rgba(255,255,255,0.08);'}
      on:mouseenter={glowOn}
      on:mouseleave={glowOff}>
      <X size={13} />
    </button>
  </div>
{:else if style === 'minimal'}
  <div class="flex items-center gap-2.5 {orderClass}" style={glowStyle}>
    <button on:click={() => dispatch('minimize')} title="Minimize (Super+↓)" class="theme-text-secondary hover:text-yellow-400 transition-colors">
      <Minus size={13} />
    </button>
    <button on:click={() => dispatch('pip')} title="Picture-in-Picture" class="theme-text-secondary transition-colors {isPiP ? 'text-blue-400' : 'hover:text-blue-400'}">
      <PictureInPicture2 size={12} />
    </button>
    <button on:click={() => dispatch('maximize')} title="Maximize (Super+↑)" class="theme-text-secondary hover:text-green-400 transition-colors">
      {#if isMaximized}<Square size={11} />{:else}<Maximize2 size={11} />{/if}
    </button>
    <button on:click={() => dispatch('close')} title="Close (Alt+F4)" class="theme-text-secondary hover:text-red-400 transition-colors">
      <X size={13} />
    </button>
  </div>
{:else}
  <!-- 'windows' — the original, pre-existing default look. -->
  <div class="flex items-center gap-0.5 {orderClass}" style={glowStyle}>
    <button on:click={() => dispatch('minimize')}
      class="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary hover:text-yellow-400 transition-colors"
      title="Minimize (Super+↓)">
      <Minus size={13} />
    </button>
    <button on:click={() => dispatch('pip')}
      class="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary transition-colors {isPiP ? 'text-blue-400' : 'hover:text-blue-400'}"
      title="Picture-in-Picture">
      <PictureInPicture2 size={12} />
    </button>
    <button on:click={() => dispatch('maximize')}
      class="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary hover:text-green-400 transition-colors"
      title="Maximize (Super+↑)">
      {#if isMaximized}<Square size={11} />{:else}<Maximize2 size={11} />{/if}
    </button>
    <button on:click={() => dispatch('close')}
      class="w-7 h-7 flex items-center justify-center hover:bg-red-500/80 rounded-md theme-text-secondary hover:text-white transition-colors"
      title="Close (Alt+F4)"
      on:mouseenter={glowOn}
      on:mouseleave={glowOff}>
      <X size={13} />
    </button>
  </div>
{/if}
