<script lang="ts">
  /**
   * Live miniature of a shell theme for the theme editor: a desktop with a
   * panel and one window, drawn from the *draft* values (not the saved
   * theme), so every slider shows its effect immediately. It re-implements
   * the handful of rules `app.css` applies globally, scoped to this box.
   */
  import type { ShellTheme } from '../../../../data/builtinThemes';
  import { withAlpha, rgbTriplet } from '../../../../utils/colorUtil';

  export let theme: ShellTheme;

  $: e = theme.extras ?? {};
  $: c = theme.colors;
  $: radius = Math.min(32, Math.max(0, e.cornerRadiusPx ?? (theme.layout.cornerStyle === 'sharp' ? 0 : 12))) * 0.6;
  $: shadow = ({
    none: 'none',
    soft: '0 2px 8px rgba(0,0,0,.3)',
    medium: '0 6px 16px rgba(0,0,0,.45)',
    strong: '0 12px 28px rgba(0,0,0,.7)',
    glow: `0 0 14px ${withAlpha(c.accent, 0.55) ?? c.accent}, 0 4px 10px rgba(0,0,0,.4)`,
  } as Record<string, string>)[e.windowShadow ?? 'medium'];
  $: borderW = e.windowBorderWidth ?? 1;
  $: borderC = e.windowBorderColor || c.border;
  $: tbH = Math.round(((e.titlebarHeight ?? 36) / 36) * 14);
  $: scale = (e.uiScalePercent ?? 100) / 100;
  $: panelRgb = rgbTriplet(e.panelColor) ?? '15, 23, 42';
  $: panelOp = (e.panelOpacity ?? 100) / 100;
  $: wall = `filter: blur(${(e.wallpaperBlur ?? 0) / 8}px) brightness(${(e.wallpaperBrightness ?? 100) / 100}) saturate(${(e.wallpaperSaturation ?? 100) / 100});`;
  $: pos = theme.layout.panelPosition;
  $: horizontal = pos === 'top' || pos === 'bottom';
</script>

<div class="relative w-full h-36 rounded-xl overflow-hidden border border-white/10 select-none" aria-hidden="true"
     style="font-family:{e.fontFamily || 'inherit'};">
  <div class="absolute inset-0" style="background:linear-gradient(135deg,{c.accent},{e.accentSecondary || c.background} 60%,{c.background}); {wall}" />

  <!-- panel -->
  <div class="absolute flex items-center gap-1 px-2"
       style="background:rgba({panelRgb},{panelOp}); border:0 solid {c.border};
              {pos === 'top' ? 'top:0;left:0;right:0;height:14px;border-bottom-width:1px;'
              : pos === 'bottom' ? 'bottom:0;left:0;right:0;height:14px;border-top-width:1px;'
              : pos === 'left' ? 'top:0;bottom:0;left:0;width:14px;border-right-width:1px;'
              : 'top:0;bottom:0;right:0;width:14px;border-left-width:1px;'}
              flex-direction:{horizontal ? 'row' : 'column'}; padding:{horizontal ? '0 6px' : '6px 0'};">
    <span class="rounded-full" style="width:6px;height:6px;background:{c.accent}" />
    <span class="rounded-full opacity-50" style="width:6px;height:6px;background:{c.text}" />
    <span class="rounded-full opacity-50" style="width:6px;height:6px;background:{c.text}" />
  </div>

  <!-- window -->
  <div class="absolute overflow-hidden flex flex-col"
       style="left:{horizontal ? 18 : 28}px; top:{pos === 'top' ? 24 : 12}px; width:62%; height:{pos === 'bottom' ? '62%' : '68%'};
              background:{c.background}; border:{borderW}px solid {borderC}; border-radius:{radius}px;
              box-shadow:{shadow}; opacity:{(e.windowOpacity ?? 100) / 100};
              transform:scale({Math.min(1.15, Math.max(0.85, scale))}); transform-origin:top left;">
    <div class="flex items-center justify-between px-2 shrink-0" style="height:{tbH}px; background:{c.surface}; border-bottom:1px solid {c.border}">
      <span class="rounded-full" style="width:5px;height:5px;background:{c.accent}" />
      <span class="flex gap-0.5"><i class="block rounded-full" style="width:5px;height:5px;background:{c.textMuted}" /><i class="block rounded-full" style="width:5px;height:5px;background:{c.textMuted}" /></span>
    </div>
    <div class="flex-1 p-1.5 space-y-1" style="background:{c.background}">
      <div class="rounded" style="height:5px;width:70%;background:{c.text};opacity:.85" />
      <div class="rounded" style="height:4px;width:50%;background:{c.textMuted}" />
      <div class="rounded" style="height:9px;width:36%;background:{c.accent}" />
    </div>
  </div>
</div>
