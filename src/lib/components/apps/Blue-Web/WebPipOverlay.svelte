<script lang="ts">
  /**
   * Picture-in-Picture frame for Blue Web — mounted ONCE in App.svelte, outside
   * every app window, so it survives minimising the browser, switching workspaces
   * and tab changes (like Android's PiP).
   *
   * The video itself is the live page of a Blue Web tab: a native webview that this
   * component positions in the "hole" of the frame (see pip.ts `videoRect`) while
   * the page shows nothing but its <video> (pip.rs). Everything the person touches
   * — title bar, buttons, resize grip — is ordinary shell DOM around the hole,
   * because the native view always paints above DOM and cannot carry shell UI.
   *
   *  • drag the title bar to move, release to snap to the nearest screen corner
   *  • drag the grip (the corner facing the screen centre) to resize; the corner
   *    the frame is snapped to stays put
   *  • double-click the title bar, or ⤢, to go back to the tab; × closes the video
   */
  import { untrack } from 'svelte';
  import { get } from 'svelte/store';
  import { Play, Pause, RotateCcw, RotateCw, Volume2, VolumeX, Maximize2, X, Globe } from 'lucide-svelte';
  import { t } from '../../../stores/language';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { blockingOverlayOpen, windowInteracting } from '../../../stores/overlayState';
  import {
    windows, currentWorkspace, focusWindow, toggleWindowFromTaskbar, switchWorkspace, openApp,
  } from '../../../stores/windowManager';
  import { AppId } from '../../../types';
  import { sameRect } from './occlusion';
  import { forgetLiveWebview } from './tabs';
  import {
    webPip, videoInfo, pipReserved, exitPip, closePip, controlPip, getPipHost, setPendingAdopt,
    frameRect, videoRect, videoHeight, frameHeight, clampPosition, clampWidth, cornerPosition,
    nearestCorner, savePipPrefs, PIP_TITLE_H, PIP_CTRL_H, type Rect, type Corner,
  } from './pip';

  interface Props { barHeight?: number; panelPosition?: 'top' | 'bottom' }
  let { barHeight = 0, panelPosition = 'top' }: Props = $props();

  // ── what the frame must stay clear of ─────────────────────────────────
  $effect(() => {
    pipReserved.set({
      top: panelPosition === 'top' ? barHeight : 0,
      bottom: panelPosition === 'bottom' ? barHeight : 0,
      left: 0, right: 0,
    });
  });

  let vw = $state(typeof window !== 'undefined' ? window.innerWidth : 1280);
  let vh = $state(typeof window !== 'undefined' ? window.innerHeight : 720);
  const onResize = () => { vw = window.innerWidth; vh = window.innerHeight; };

  // ── video state from the page ─────────────────────────────────────────
  let pipTab = $derived($webPip?.tabId ?? null);
  let info = $derived(pipTab ? $videoInfo[pipTab] : undefined);

  // A report that says "no video" counts only after the page had time to answer.
  let ready = $state(false);
  $effect(() => {
    const id = pipTab;
    ready = false;
    if (!id) return;
    const timer = setTimeout(() => { ready = true; }, 1500);
    return () => clearTimeout(timer);
  });
  let noVideo = $derived(ready && info?.hasVideo === false);

  // Keep the frame in its corner when the video's aspect ratio, the screen or the panel changes.
  $effect(() => {
    const a = info?.hasVideo ? info.aspect : null;
    const r = $pipReserved;
    const w = vw, h = vh;
    untrack(() => {
      const s = get(webPip);
      if (!s || busy) return;
      const aspect = a ?? s.aspect;
      const width = clampWidth(s.width, aspect, w, h, r);
      const pos = cornerPosition(s.corner, width, aspect, w, h, r);
      if (width !== s.width || aspect !== s.aspect || pos.x !== s.x || pos.y !== s.y) {
        webPip.set({ ...s, width, aspect, ...pos });
      }
    });
  });

  // ── the native page follows the hole ──────────────────────────────────
  type Want = { tabId: string; rect: Rect; visible: boolean };
  let applied: Want | null = null;
  let want: Want | null = null;
  let pushing = false;
  let failures = 0;

  async function flush() {
    if (pushing) return;
    pushing = true;
    try {
      while (want) {
        const w = want;
        want = null;
        try {
          const sameTab = applied?.tabId === w.tabId;
          if (w.visible) {
            if (!sameTab || !applied || !sameRect(applied.rect, w.rect)) {
              await SystemBridge.invokeCommand('web_view_set_bounds', { tabId: w.tabId, ...w.rect });
            }
            if (!sameTab || !applied?.visible) {
              await SystemBridge.invokeCommand('web_view_set_visible', { tabId: w.tabId, visible: true });
            }
          } else if (!sameTab || applied?.visible) {
            await SystemBridge.invokeCommand('web_view_set_visible', { tabId: w.tabId, visible: false });
          }
          applied = w;
          failures = 0;
        } catch {
          applied = null;
          if (++failures <= 5) setTimeout(() => { if (!want && get(webPip)?.tabId === w.tabId) { want = w; flush(); } }, 250);
        }
      }
    } finally {
      pushing = false;
    }
  }

  $effect(() => {
    const s = $webPip;
    const hidden = $blockingOverlayOpen; // start menu & co. must be able to cover the video
    if (!s) { applied = null; want = null; return; }
    if (!SystemBridge.isTauri()) return;
    want = { tabId: s.tabId, rect: videoRect(s), visible: !hidden };
    flush();
  });

  // While something is being dragged (here or an app window) the native page lets mouse events through.
  let dragging = $state(false);
  let busy = $derived(dragging || $windowInteracting);
  $effect(() => {
    const id = pipTab;
    const b = busy;
    if (id && SystemBridge.isTauri()) SystemBridge.invokeCommand('web_view_set_interactive', { tabId: id, interactive: !b }).catch(() => {});
  });

  // ── moving ────────────────────────────────────────────────────────────
  let drag: { px: number; py: number; sx: number; sy: number } | null = null;

  function onTitleDown(e: PointerEvent) {
    if ((e.target as HTMLElement).closest('button')) return;
    const s = get(webPip);
    if (!s) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { px: e.clientX, py: e.clientY, sx: s.x, sy: s.y };
    dragging = true;
  }
  function onTitleMove(e: PointerEvent) {
    if (!drag) return;
    const d = drag;
    webPip.update((s) => s ? { ...s, ...clampPosition({ x: d.sx + e.clientX - d.px, y: d.sy + e.clientY - d.py, width: s.width, aspect: s.aspect }, vw, vh, get(pipReserved)) } : s);
  }
  function settle() {
    const s = get(webPip);
    if (s) {
      const r = get(pipReserved);
      const corner = nearestCorner(s, vw, vh, r);
      webPip.set({ ...s, corner, ...cornerPosition(corner, s.width, s.aspect, vw, vh, r) });
      savePipPrefs({ width: s.width, corner });
    }
    dragging = false;
  }
  function onTitleUp() {
    if (!drag) return;
    drag = null;
    settle();
  }

  // ── resizing (from the corner that faces the screen centre) ───────────
  let resize: { px: number; py: number; width: number; right: number; bottom: number; corner: Corner } | null = null;

  function onGripDown(e: PointerEvent) {
    const s = get(webPip);
    if (!s) return;
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    resize = { px: e.clientX, py: e.clientY, width: s.width, right: s.x + s.width, bottom: s.y + frameHeight(s.width, s.aspect), corner: s.corner };
    dragging = true;
  }
  function onGripMove(e: PointerEvent) {
    if (!resize) return;
    const g = resize;
    const anchorRight = g.corner[1] === 'r';
    const dx = (e.clientX - g.px) * (anchorRight ? -1 : 1);
    webPip.update((s) => {
      if (!s) return s;
      const width = clampWidth(g.width + dx, s.aspect, vw, vh, get(pipReserved));
      const x = anchorRight ? g.right - width : s.x;
      const y = g.corner[0] === 'b' ? g.bottom - frameHeight(width, s.aspect) : s.y;
      return { ...s, width, ...clampPosition({ x, y, width, aspect: s.aspect }, vw, vh, get(pipReserved)) };
    });
  }
  function onGripUp() {
    if (!resize) return;
    resize = null;
    settle();
  }

  // ── buttons ───────────────────────────────────────────────────────────
  /** Back to the tab it came from — reopening the browser window if it was hidden or closed. */
  async function giveBack() {
    const s = get(webPip);
    if (!s) return;
    const host = getPipHost(s.windowId);
    if (host) {
      host.activateTab(s.tabId);
      await exitPip();
      focusWindow(s.windowId);
      return;
    }
    // The browser window is minimised / on another workspace / gone: its replacement claims the live tab.
    setPendingAdopt({ tabId: s.tabId, url: s.url, title: s.title, favicon: s.favicon });
    const win = get(windows).find((w) => w.id === s.windowId);
    if (win) {
      if (win.workspace !== get(currentWorkspace)) switchWorkspace(win.workspace);
      if (win.isMinimized) toggleWindowFromTaskbar(win.id); else focusWindow(win.id);
    } else {
      openApp(AppId.BLUE_WEB);
    }
  }

  async function close() {
    const orphan = await closePip();
    if (orphan) forgetLiveWebview(orphan);
  }

  let frame = $derived($webPip ? frameRect($webPip) : null);
  let holeHeight = $derived($webPip ? videoHeight($webPip.width, $webPip.aspect) : 0);
  // The grip sits at the corner facing the screen centre: the anchored corner must stay put.
  let gripAtTop = $derived($webPip?.corner[0] === 'b');
  let gripAtLeft = $derived($webPip?.corner[1] === 'r');
  let gripCursor = $derived(gripAtTop === gripAtLeft ? 'nwse-resize' : 'nesw-resize');
</script>

<svelte:window onresize={onResize} />

{#if $webPip && frame}
  {@const s = $webPip}
  <div data-shell-occluder="web-pip" role="group" aria-label={$t('blueweb.pip')}
    class="fixed z-[9990] flex flex-col rounded-lg overflow-hidden bg-black border border-white/15 shadow-2xl shadow-black/60 text-white select-none"
    style="left:{frame.x}px; top:{frame.y}px; width:{frame.width}px; height:{frame.height}px;">

    <!-- title bar: drag to move, double-click to go back -->
    <div class="flex items-center gap-1.5 px-2 bg-slate-900/95 shrink-0 cursor-grab active:cursor-grabbing"
      style="height:{PIP_TITLE_H}px;"
      onpointerdown={onTitleDown} onpointermove={onTitleMove} onpointerup={onTitleUp} onpointercancel={onTitleUp}
      ondblclick={giveBack} role="presentation">
      {#if s.favicon}<img src={s.favicon} alt="" class="w-3.5 h-3.5 rounded-sm shrink-0 pointer-events-none" />
      {:else}<Globe size={12} class="shrink-0 text-slate-400 pointer-events-none" />{/if}
      <span class="text-[11px] text-slate-300 truncate flex-1 pointer-events-none">{noVideo ? $t('blueweb.pip_no_video') : s.title}</span>
      <button onclick={giveBack} title={$t('blueweb.pip_return')} aria-label={$t('blueweb.pip_return')} class="p-1 rounded hover:bg-white/10 text-slate-300 hover:text-white"><Maximize2 size={12} /></button>
      <button onclick={close} title={$t('blueweb.pip_close')} aria-label={$t('blueweb.pip_close')} class="p-1 rounded hover:bg-red-500/80 text-slate-300 hover:text-white"><X size={12} /></button>
    </div>

    <!-- the hole: the native page (nothing but the video) is placed exactly here -->
    <div class="bg-black shrink-0 pointer-events-none" style="height:{holeHeight}px;"></div>

    <!-- controls -->
    <div class="flex items-center justify-center gap-1 bg-slate-900/95 shrink-0" style="height:{PIP_CTRL_H}px;">
      <button onclick={() => controlPip('seek', -10)} title={$t('blueweb.pip_back10')} aria-label={$t('blueweb.pip_back10')} class="p-1.5 rounded-full hover:bg-white/10 text-slate-200"><RotateCcw size={15} /></button>
      <button onclick={() => controlPip('toggle')} title={info?.playing ? $t('blueweb.pip_pause') : $t('blueweb.pip_play')} aria-label={info?.playing ? $t('blueweb.pip_pause') : $t('blueweb.pip_play')} class="p-2 rounded-full bg-white/10 hover:bg-white/20 text-white">
        {#if info?.playing}<Pause size={16} />{:else}<Play size={16} />{/if}
      </button>
      <button onclick={() => controlPip('seek', 10)} title={$t('blueweb.pip_fwd10')} aria-label={$t('blueweb.pip_fwd10')} class="p-1.5 rounded-full hover:bg-white/10 text-slate-200"><RotateCw size={15} /></button>
      <button onclick={() => controlPip('toggle_mute')} title={info?.muted ? $t('blueweb.pip_unmute') : $t('blueweb.pip_mute')} aria-label={info?.muted ? $t('blueweb.pip_unmute') : $t('blueweb.pip_mute')} class="p-1.5 rounded-full hover:bg-white/10 text-slate-200 ml-1">
        {#if info?.muted}<VolumeX size={15} />{:else}<Volume2 size={15} />{/if}
      </button>
    </div>

    <!-- resize grip, in a bar corner (the native video would hide it anywhere else) -->
    <div role="presentation" title={$t('blueweb.pip_resize')}
      class="absolute w-5 h-5 z-10 flex items-center justify-center text-white/40 hover:text-white/80"
      style="cursor:{gripCursor}; {gripAtTop ? 'top:0;' : 'bottom:0;'} {gripAtLeft ? 'left:0;' : 'right:0;'} touch-action:none;"
      onpointerdown={onGripDown} onpointermove={onGripMove} onpointerup={onGripUp} onpointercancel={onGripUp}>
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"
        style="transform: scale({gripAtLeft ? -1 : 1}, {gripAtTop ? -1 : 1});">
        <path d="M9 3 3 9M9 6.5 6.5 9" />
      </svg>
    </div>
  </div>
{/if}
