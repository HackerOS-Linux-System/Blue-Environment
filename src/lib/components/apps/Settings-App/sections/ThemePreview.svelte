<script lang="ts">
  /**
   * Live miniature of a shell theme for the theme editor: a desktop with a
   * panel and one window, drawn from the *draft* values (not the saved
   * theme), so every slider shows its effect immediately. It re-implements
   * the handful of rules `app.css` applies globally, scoped to this box.
   */
  import type { ShellTheme } from '../../../../data/builtinThemes';
  import { withAlpha, rgbTriplet } from '../../../../utils/colorUtil';

  interface Props {
    theme: ShellTheme;
  }

  let { theme }: Props = $props();

  let e = $derived(theme.extras ?? {});
  let c = $derived(theme.colors);
  let radius = $derived(Math.min(32, Math.max(0, e.cornerRadiusPx ?? (theme.layout.cornerStyle === 'sharp' ? 0 : 12))) * 0.6);
  let shadow = $derived(({
    none: 'none',
    soft: '0 2px 8px rgba(0,0,0,.3)',
    medium: '0 6px 16px rgba(0,0,0,.45)',
    strong: '0 12px 28px rgba(0,0,0,.7)',
    glow: `0 0 14px ${withAlpha(c.accent, 0.55) ?? c.accent}, 0 4px 10px rgba(0,0,0,.4)`,
  } as Record<string, string>)[e.windowShadow ?? 'medium']);
  let borderW = $derived(e.windowBorderWidth ?? 1);
  let borderC = $derived(e.windowBorderColor || c.border);
  let tbH = $derived(Math.round(((e.titlebarHeight ?? 36) / 36) * 14));
  let scale = $derived((e.uiScalePercent ?? 100) / 100);
  let panelRgb = $derived(rgbTriplet(e.panelColor) ?? '15, 23, 42');
  let panelOp = $derived((e.panelOpacity ?? 100) / 100);
  let wall = $derived(`filter: blur(${(e.wallpaperBlur ?? 0) / 8}px) brightness(${(e.wallpaperBrightness ?? 100) / 100}) saturate(${(e.wallpaperSaturation ?? 100) / 100});`);
  let pos = $derived(theme.layout.panelPosition);
  let horizontal = $derived(pos === 'top' || pos === 'bottom');

  // ── mini app menu ──
  let menuRgb = $derived(rgbTriplet(e.menuBackgroundColor) ?? '15, 23, 42');
  let menuOp = $derived((e.menuOpacity ?? 70) / 100);
  let menuBlur = $derived((e.menuBlur ?? 40) / 8);
  let sideBg = $derived(e.menuSidebar === 'transparent' ? 'transparent' : e.menuSidebar === 'solid' ? `rgba(${rgbTriplet(e.menuBackgroundColor) ?? '2, 6, 23'}, 0.9)` : 'rgba(2, 6, 23, 0.3)');
  let tileCount = $derived(e.menuTileSize === 'compact' ? 10 : e.menuTileSize === 'large' ? 4 : 6);
  let tileEdge = $derived(e.menuTileSize === 'compact' ? 9 : e.menuTileSize === 'large' ? 18 : 13);
  let tileRadius = $derived(e.menuTileShape === 'circle' ? '9999px' : e.menuTileShape === 'square' ? '2px' : '5px');
  let searchRadius = $derived(e.menuSearchShape === 'pill' ? '9999px' : e.menuSearchShape === 'square' ? '2px' : '5px');
</script>

<div class="relative w-full h-36 rounded-xl overflow-hidden border border-white/10 select-none" aria-hidden="true"
     style="font-family:{e.fontFamily || 'inherit'};">
  <div class="absolute inset-0" style="background:linear-gradient(135deg,{c.accent},{e.accentSecondary || c.background} 60%,{c.background}); {wall}"></div>

  <!-- panel -->
  <div class="absolute flex items-center gap-1 px-2"
       style="background:rgba({panelRgb},{panelOp}); border:0 solid {c.border};
              {pos === 'top' ? 'top:0;left:0;right:0;height:14px;border-bottom-width:1px;'
              : pos === 'bottom' ? 'bottom:0;left:0;right:0;height:14px;border-top-width:1px;'
              : pos === 'left' ? 'top:0;bottom:0;left:0;width:14px;border-right-width:1px;'
              : 'top:0;bottom:0;right:0;width:14px;border-left-width:1px;'}
              flex-direction:{horizontal ? 'row' : 'column'}; padding:{horizontal ? '0 6px' : '6px 0'};">
    <span class="rounded-full" style="width:6px;height:6px;background:{c.accent}"></span>
    <span class="rounded-full opacity-50" style="width:6px;height:6px;background:{c.text}"></span>
    <span class="rounded-full opacity-50" style="width:6px;height:6px;background:{c.text}"></span>
  </div>

  <!-- window -->
  <div class="absolute overflow-hidden flex flex-col"
       style="left:{horizontal ? 18 : 28}px; top:{pos === 'top' ? 24 : 12}px; width:62%; height:{pos === 'bottom' ? '62%' : '68%'};
              background:{c.background}; border:{borderW}px solid {borderC}; border-radius:{radius}px;
              box-shadow:{shadow}; opacity:{(e.windowOpacity ?? 100) / 100};
              transform:scale({Math.min(1.15, Math.max(0.85, scale))}); transform-origin:top left;">
    <div class="flex items-center justify-between px-2 shrink-0" style="height:{tbH}px; background:{c.surface}; border-bottom:1px solid {c.border}">
      <span class="rounded-full" style="width:5px;height:5px;background:{c.accent}"></span>
      <span class="flex gap-0.5"><i class="block rounded-full" style="width:5px;height:5px;background:{c.textMuted}"></i><i class="block rounded-full" style="width:5px;height:5px;background:{c.textMuted}"></i></span>
    </div>
    <div class="flex-1 p-1.5 space-y-1" style="background:{c.background}">
      <div class="rounded" style="height:5px;width:70%;background:{c.text};opacity:.85"></div>
      <div class="rounded" style="height:4px;width:50%;background:{c.textMuted}"></div>
      <div class="rounded" style="height:9px;width:36%;background:{c.accent}"></div>
    </div>
  </div>
</div>

<!-- The full-screen application menu, in miniature -->
<div class="relative w-full h-24 mt-2 rounded-xl overflow-hidden border border-white/10 select-none" aria-hidden="true">
  <div class="absolute inset-0" style="background:linear-gradient(135deg,{c.accent},{e.accentSecondary || c.background} 60%,{c.background}); {wall}"></div>
  <div class="absolute inset-0 flex" style="background-color:rgba({menuRgb},{menuOp}); backdrop-filter:blur({menuBlur}px); -webkit-backdrop-filter:blur({menuBlur}px);">
    <div class="w-1/4 h-full p-1.5 space-y-1" style="background:{sideBg}">
      <div class="h-2 rounded" style="background:{c.accent}"></div>
      <div class="h-2 rounded bg-white/15"></div>
      <div class="h-2 rounded bg-white/15"></div>
    </div>
    <div class="flex-1 p-2 space-y-1.5">
      <div class="h-3 bg-white/10 border border-white/10" style="border-radius:{searchRadius}"></div>
      <div class="flex flex-wrap gap-1">
        {#each Array(tileCount) as _, i (i)}
          <div class="bg-white/15" style="width:{tileEdge}px;height:{tileEdge}px;border-radius:{tileRadius}"></div>
        {/each}
      </div>
    </div>
  </div>
</div>
