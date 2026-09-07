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
   * ── Button order (`order` prop) ─────────────────────────────────────
   * Each button is rendered in DOM order from `order`, then the whole
   * group is optionally mirrored by CSS (`flex-row-reverse`) when
   * `position === 'right'`. That mirroring is what makes "close nearest
   * the window's outer edge" work for *either* side without needing two
   * separate hand-written orderings: write the sequence once as
   * "edge-outward" (closest to the window's outer corner first) and the
   * same array works whichever side the group is docked to.
   *
   * `DEFAULT_WINDOW_CONTROLS_ORDER` (edge-outward: close, maximize,
   * minimize, pip) was previously hardcoded per-style with `pip` and
   * `minimize` swapped for the `windows`/`gnome`/`minimal` styles — that
   * was a real, reproducible bug (screenshotted: right-aligned controls
   * rendered as close, maximize, [pip], minimize instead of close,
   * maximize, minimize, [pip]). `macos`'s own DOM order was already
   * correct by accident (it doesn't reverse, being left-aligned by
   * default) and is kept as its own distinct default to match real
   * macOS's close-minimize-maximize convention specifically.
   *
   * A person can override this default entirely from Settings →
   * Themes → "Window control button order" (see ThemesSection.svelte),
   * persisted as `config.windowControlsOrder` and passed in here as
   * `order` — that's the "also customizable" half of the fix.
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
  import type { WindowControlsStyle, WindowControlsPosition, WindowControlId, WindowControlsOrder } from '../data/builtinThemes';
  import { DEFAULT_WINDOW_CONTROLS_ORDER, MACOS_DEFAULT_WINDOW_CONTROLS_ORDER } from '../data/builtinThemes';

  export let isMaximized: boolean;
  export let isPiP: boolean | undefined = false;
  export let style: WindowControlsStyle = 'windows';
  export let position: WindowControlsPosition = 'right';
  export let accentGlow: string | undefined = undefined;
  /** Custom button order override (see this file's doc comment above).
   * Falls back to the style's own sensible default when not provided. */
  export let order: WindowControlsOrder | undefined = undefined;

  const dispatch = createEventDispatcher<{ close: void; minimize: void; maximize: void; pip: void }>();

  let effectiveOrder: WindowControlsOrder;
  $: effectiveOrder = (order && order.length === 4)
    ? order
    : (style === 'macos' ? MACOS_DEFAULT_WINDOW_CONTROLS_ORDER : DEFAULT_WINDOW_CONTROLS_ORDER);

  $: orderClass = position === 'left' ? 'flex-row' : 'flex-row-reverse';
  $: glowStyle = accentGlow ? `--ctl-glow:${accentGlow};` : '';

  function fire(id: WindowControlId) {
    dispatch(id);
  }

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
    {#each effectiveOrder as id (id)}
      {#if id === 'close'}
        <button on:click={() => fire('close')} title="Close (Alt+F4)"
          class="w-3 h-3 rounded-full bg-[#ff5f57] flex items-center justify-center transition-shadow"
          style={accentGlow ? 'box-shadow: 0 0 0 rgba(0,0,0,0);' : ''}
          on:mouseenter={glowOn}
          on:mouseleave={glowOff}>
          <X size={7} class="opacity-0 group-hover:opacity-70 text-black" />
        </button>
      {:else if id === 'minimize'}
        <button on:click={() => fire('minimize')} title="Minimize (Super+↓)"
          class="w-3 h-3 rounded-full bg-[#febc2e] flex items-center justify-center">
          <Minus size={7} class="opacity-0 group-hover:opacity-70 text-black" />
        </button>
      {:else if id === 'maximize'}
        <button on:click={() => fire('maximize')} title="Maximize (Super+↑)"
          class="w-3 h-3 rounded-full bg-[#28c840] flex items-center justify-center">
          {#if isMaximized}<Square size={6} class="opacity-0 group-hover:opacity-70 text-black" />
          {:else}<Maximize2 size={6} class="opacity-0 group-hover:opacity-70 text-black" />{/if}
        </button>
      {:else}
        <button on:click={() => fire('pip')} title="Picture-in-Picture"
          class="w-3 h-3 rounded-full flex items-center justify-center transition-colors {isPiP ? 'bg-blue-400' : 'bg-slate-500'}">
          <PictureInPicture2 size={6} class="opacity-0 group-hover:opacity-70 text-black" />
        </button>
      {/if}
    {/each}
  </div>
{:else if style === 'gnome'}
  <div class="flex items-center gap-1 {orderClass}" style={glowStyle}>
    {#each effectiveOrder as id (id)}
      {#if id === 'minimize'}
        <button on:click={() => fire('minimize')} title="Minimize (Super+↓)"
          class="w-6 h-6 flex items-center justify-center hover:bg-white/10 rounded-full theme-text-secondary hover:text-white transition-colors opacity-70 hover:opacity-100">
          <Minus size={11} />
        </button>
      {:else if id === 'maximize'}
        <button on:click={() => fire('maximize')} title="Maximize (Super+↑)"
          class="w-6 h-6 flex items-center justify-center hover:bg-white/10 rounded-full theme-text-secondary hover:text-white transition-colors opacity-70 hover:opacity-100">
          {#if isMaximized}<Square size={10} />{:else}<Maximize2 size={10} />{/if}
        </button>
      {:else if id === 'pip'}
        <button on:click={() => fire('pip')} title="Picture-in-Picture"
          class="w-6 h-6 flex items-center justify-center hover:bg-white/10 rounded-full theme-text-secondary transition-colors opacity-70 hover:opacity-100 {isPiP ? 'text-blue-400' : ''}">
          <PictureInPicture2 size={10} />
        </button>
      {:else}
        <button on:click={() => fire('close')} title="Close (Alt+F4)"
          class="w-7 h-7 flex items-center justify-center hover:bg-red-500 rounded-full text-white/90 hover:text-white transition-colors ml-1"
          style={accentGlow ? 'background: rgba(255,255,255,0.08);' : 'background: rgba(255,255,255,0.08);'}
          on:mouseenter={glowOn}
          on:mouseleave={glowOff}>
          <X size={13} />
        </button>
      {/if}
    {/each}
  </div>
{:else if style === 'minimal'}
  <div class="flex items-center gap-2.5 {orderClass}" style={glowStyle}>
    {#each effectiveOrder as id (id)}
      {#if id === 'minimize'}
        <button on:click={() => fire('minimize')} title="Minimize (Super+↓)" class="theme-text-secondary hover:text-yellow-400 transition-colors">
          <Minus size={13} />
        </button>
      {:else if id === 'pip'}
        <button on:click={() => fire('pip')} title="Picture-in-Picture" class="theme-text-secondary transition-colors {isPiP ? 'text-blue-400' : 'hover:text-blue-400'}">
          <PictureInPicture2 size={12} />
        </button>
      {:else if id === 'maximize'}
        <button on:click={() => fire('maximize')} title="Maximize (Super+↑)" class="theme-text-secondary hover:text-green-400 transition-colors">
          {#if isMaximized}<Square size={11} />{:else}<Maximize2 size={11} />{/if}
        </button>
      {:else}
        <button on:click={() => fire('close')} title="Close (Alt+F4)" class="theme-text-secondary hover:text-red-400 transition-colors">
          <X size={13} />
        </button>
      {/if}
    {/each}
  </div>
{:else}
  <!-- 'windows' — the original, pre-existing default look. -->
  <div class="flex items-center gap-0.5 {orderClass}" style={glowStyle}>
    {#each effectiveOrder as id (id)}
      {#if id === 'minimize'}
        <button on:click={() => fire('minimize')}
          class="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary hover:text-yellow-400 transition-colors"
          title="Minimize (Super+↓)">
          <Minus size={13} />
        </button>
      {:else if id === 'pip'}
        <button on:click={() => fire('pip')}
          class="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary transition-colors {isPiP ? 'text-blue-400' : 'hover:text-blue-400'}"
          title="Picture-in-Picture">
          <PictureInPicture2 size={12} />
        </button>
      {:else if id === 'maximize'}
        <button on:click={() => fire('maximize')}
          class="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary hover:text-green-400 transition-colors"
          title="Maximize (Super+↑)">
          {#if isMaximized}<Square size={11} />{:else}<Maximize2 size={11} />{/if}
        </button>
      {:else}
        <button on:click={() => fire('close')}
          class="w-7 h-7 flex items-center justify-center hover:bg-red-500/80 rounded-md theme-text-secondary hover:text-white transition-colors"
          title="Close (Alt+F4)"
          on:mouseenter={glowOn}
          on:mouseleave={glowOff}>
          <X size={13} />
        </button>
      {/if}
    {/each}
  </div>
{/if}
