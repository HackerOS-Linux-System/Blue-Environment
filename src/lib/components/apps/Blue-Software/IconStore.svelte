<script lang="ts">
  /**
   * Blue Software → Icons: the KDE Store (store.kde.org) in an embedded native webview.
   *
   * Icon themes there are plain freedesktop icon themes — exactly what Blue Environment's
   * icon resolver reads. When a download in the webview finishes, the archive is unpacked
   * into ~/.local/share/icons (`icon_store_install`) and can be applied straight away.
   *
   * IMPORTANT layout rule: the page is a NATIVE surface that always paints above the shell's
   * DOM. Nothing of ours may overlap it, so the status line and the installed-themes list are
   * placed BESIDE/BELOW the page region, never on top of it. The webview is sized to the
   * `pageEl` placeholder (same technique as Blue Web).
   */
  import { onMount, onDestroy } from 'svelte';
  import { ArrowLeft, ArrowRight, RotateCw, Home, Palette, Check, Trash2, Loader2, ExternalLink, PackageOpen } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { t } from '../../../stores/language';
  import { windows, topmostVisibleWindowId } from '../../../stores/windowManager';
  import { blockingOverlayOpen, windowInteracting } from '../../../stores/overlayState';
  import { iconThemes, isIconArchive, ICON_THEMES_CHANGED, ICON_THEME_APPLIED, type IconThemeInfo } from '../../../utils/iconThemes';

  export let windowId: string = '';
  /** False while another Blue Software section is showing — the native page must then be hidden. */
  export let visible: boolean = true;

  const STORE_HOME = 'https://store.kde.org/browse?cat=132&ord=latest'; // "Full Icon Themes"
  const tabId = `icon-store-${windowId || Math.random().toString(36).slice(2, 8)}`;

  let pageEl: HTMLDivElement;
  let created = false;
  let creating = false;
  let createError = '';
  let host = 'store.kde.org';
  let showInstalled = true;
  let installed: IconThemeInfo[] = [];
  let status: { kind: 'info' | 'ok' | 'error'; text: string; apply?: string[] } | null = null;
  let installing = false;
  let lastRect: { x: number; y: number; width: number; height: number } | null = null;
  let pushing = false;
  let raf = 0;
  const unlisten: (() => void)[] = [];
  const inTauri = SystemBridge.isTauri();

  $: shouldShow = visible && created && $topmostVisibleWindowId === windowId && !$blockingOverlayOpen;
  let prevShow = false;
  $: if (shouldShow !== prevShow) {
    prevShow = shouldShow;
    if (created) {
      if (shouldShow) lastRect = null; // re-push bounds right away, then show
      SystemBridge.invokeCommand('web_view_set_visible', { tabId, visible: shouldShow }).catch(() => {});
    }
  }
  // Click-through while the window is dragged/resized (same guard as Blue Web).
  $: if (created) SystemBridge.invokeCommand('web_view_set_interactive', { tabId, interactive: !$windowInteracting }).catch(() => {});

  function measure() {
    if (!pageEl) return null;
    const r = pageEl.getBoundingClientRect();
    let { left: x, top: y, width, height } = r;
    const win = $windows.find((w) => w.id === windowId);
    if (win && !win.isMaximized) {
      const l = Math.max(x, win.x), tp = Math.max(y, win.y);
      const rr = Math.min(x + width, win.x + win.width), b = Math.min(y + height, win.y + win.height);
      x = l; y = tp; width = Math.max(0, rr - l); height = Math.max(0, b - tp);
    }
    if (x < 0) { width += x; x = 0; }
    if (y < 0) { height += y; y = 0; }
    width = Math.max(0, Math.min(width, window.innerWidth - x));
    height = Math.max(0, Math.min(height, window.innerHeight - y));
    return width < 2 || height < 2 ? null : { x, y, width, height };
  }
  const same = (a: any, b: any) => !!a && !!b && a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;

  function loop() {
    if (created && shouldShow && !pushing) {
      const r = measure();
      if (r && !same(r, lastRect)) {
        pushing = true;
        SystemBridge.invokeCommand('web_view_set_bounds', { tabId, ...r })
          .then(() => { lastRect = r; })
          .catch(() => {})
          .finally(() => { pushing = false; });
      }
    }
    raf = requestAnimationFrame(loop);
  }

  async function createView() {
    if (!inTauri || created || creating) return;
    creating = true; createError = '';
    for (let i = 0; i < 12 && !measure(); i++) await new Promise((r) => requestAnimationFrame(() => r(null)));
    const r = measure();
    if (!r) { creating = false; createError = 'layout'; return; }
    try {
      await SystemBridge.invokeCommand('web_view_create', { windowLabel: 'main', tabId, url: STORE_HOME, ...r });
      created = true; lastRect = r;
      if (!shouldShow) SystemBridge.invokeCommand('web_view_set_visible', { tabId, visible: false }).catch(() => {});
      attachEvents();
    } catch (e: any) {
      createError = String(e?.message ?? e);
    } finally { creating = false; }
  }

  async function attachEvents() {
    try {
      const ev = await import('@tauri-apps/api/event');
      // keep the address indicator current
      unlisten.push(await ev.listen<string>(`web-nav-${tabId}`, (e) => { try { host = new URL(e.payload).host; } catch { /* keep */ } }));
      // `target=_blank` links are denied by the backend and reported here — open them in the same view.
      unlisten.push(await ev.listen<string>(`web-popup-${tabId}`, (e) => {
        SystemBridge.invokeCommand('web_view_navigate', { tabId, url: e.payload }).catch(() => {});
      }));
      // A finished download in OUR webview → try to install it.
      unlisten.push(await ev.listen<{ tab_id: string; filename: string; path: string; state: string }>('web-download-finished', (e) => {
        const d = e.payload;
        if (d.tab_id !== tabId) return;
        onDownloaded(d.filename, d.path, d.state);
      }));
    } catch { /* not in Tauri */ }
  }

  async function onDownloaded(filename: string, path: string, state: string) {
    if (state !== 'done') { status = { kind: 'error', text: $t('iconstore.install_failed', { error: filename }) }; return; }
    if (!isIconArchive(filename)) { status = { kind: 'info', text: $t('iconstore.not_theme', { name: filename }) }; return; }
    installing = true;
    status = { kind: 'info', text: $t('iconstore.installing', { name: filename }) };
    try {
      const names = await iconThemes.install(path);
      status = { kind: 'ok', text: $t('iconstore.installed_ok', { themes: names.join(', ') }), apply: names };
    } catch (e: any) {
      status = { kind: 'error', text: $t('iconstore.install_failed', { error: String(e?.message ?? e) }) };
    } finally { installing = false; }
  }

  async function refreshInstalled() { installed = await iconThemes.list(); }
  async function apply(name: string | null) {
    try {
      await iconThemes.apply(name);
      status = { kind: 'ok', text: $t('iconstore.applied', { name: name ?? $t('iconstore.system_theme') }) };
    } catch (e: any) { status = { kind: 'error', text: String(e?.message ?? e) }; }
    refreshInstalled();
  }
  async function remove(th: IconThemeInfo) {
    try {
      await iconThemes.remove(th.name);
      status = { kind: 'info', text: $t('iconstore.removed', { name: th.name }) };
    } catch (e: any) { status = { kind: 'error', text: String(e?.message ?? e) }; }
    refreshInstalled();
  }

  const nav = (cmd: string, extra: Record<string, unknown> = {}) => SystemBridge.invokeCommand(cmd, { tabId, ...extra }).catch(() => {});
  const goHome = () => nav('web_view_navigate', { url: STORE_HOME });
  const openExternal = () => window.open(STORE_HOME, '_blank', 'noopener');

  onMount(() => {
    raf = requestAnimationFrame(loop);
    refreshInstalled();
    const onChange = () => refreshInstalled();
    window.addEventListener(ICON_THEMES_CHANGED, onChange);
    window.addEventListener(ICON_THEME_APPLIED, onChange);
    unlisten.push(() => { window.removeEventListener(ICON_THEMES_CHANGED, onChange); window.removeEventListener(ICON_THEME_APPLIED, onChange); });
    createView();
  });
  // Section was opened after mount (lazy) — create once it becomes visible and measurable.
  $: if (visible && !created && !creating && inTauri && pageEl) createView();

  onDestroy(() => {
    cancelAnimationFrame(raf);
    unlisten.forEach((u) => u());
    if (created) SystemBridge.invokeCommand('web_view_close', { tabId }).catch(() => {});
  });
</script>

<div class="flex flex-col h-full min-h-0 bg-slate-900" class:hidden={!visible}>
  <!-- toolbar -->
  <div class="flex items-center gap-1.5 px-3 py-2 border-b border-white/5 shrink-0">
    <button class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-40" title={$t('iconstore.back')} disabled={!created} on:click={() => nav('web_view_history_go', { delta: -1 })}><ArrowLeft size={15} /></button>
    <button class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-40" title={$t('iconstore.forward')} disabled={!created} on:click={() => nav('web_view_history_go', { delta: 1 })}><ArrowRight size={15} /></button>
    <button class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-40" title={$t('iconstore.reload')} disabled={!created} on:click={() => nav('web_view_reload')}><RotateCw size={15} /></button>
    <button class="p-1.5 rounded-lg hover:bg-white/10 disabled:opacity-40" title={$t('iconstore.home')} disabled={!created} on:click={goHome}><Home size={15} /></button>
    <div class="flex-1 mx-2 px-3 py-1 rounded-lg bg-slate-800 text-xs text-slate-400 truncate">{host}</div>
    <button class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs transition-colors {showInstalled ? 'bg-blue-600/30 text-blue-200' : 'hover:bg-white/10 text-slate-300'}" on:click={() => (showInstalled = !showInstalled)}>
      <Palette size={14} /> {$t('iconstore.installed')} ({installed.length})
    </button>
  </div>

  <div class="flex flex-1 min-h-0">
    <!-- the native page -->
    <div class="flex-1 min-w-0 relative bg-slate-950" bind:this={pageEl}>
      {#if !inTauri}
        <div class="absolute inset-0 flex flex-col items-center justify-center gap-3 text-slate-400 text-sm p-6 text-center">
          <PackageOpen size={36} class="opacity-50" />
          <p>{$t('iconstore.needs_desktop')}</p>
          <button class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-xs" on:click={openExternal}><ExternalLink size={13} /> {$t('iconstore.open_in_browser')}</button>
        </div>
      {:else if createError}
        <div class="absolute inset-0 flex flex-col items-center justify-center gap-3 text-sm text-slate-400 p-6 text-center">
          <p class="text-red-300 break-words max-w-md">{createError}</p>
          <button class="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 text-xs" on:click={createView}>{$t('iconstore.reload')}</button>
        </div>
      {:else if !created}
        <div class="absolute inset-0 flex items-center justify-center text-slate-500"><Loader2 size={26} class="animate-spin" /></div>
      {/if}
    </div>

    <!-- installed themes: BESIDE the page, never over it -->
    {#if showInstalled}
      <aside class="w-64 shrink-0 border-l border-white/5 flex flex-col min-h-0 bg-slate-900">
        <div class="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-slate-500">{$t('iconstore.installed')}</div>
        <div class="flex-1 overflow-y-auto px-2 pb-2 space-y-1">
          <button class="w-full flex items-center gap-2 px-2.5 py-2 rounded-lg text-left text-xs hover:bg-white/5 {installed.every((x) => !x.active) ? 'bg-blue-600/20' : ''}" on:click={() => apply(null)}>
            <span class="flex-1 truncate">{$t('iconstore.system_theme')}</span>
            {#if installed.every((x) => !x.active)}<Check size={13} class="text-blue-400" />{/if}
          </button>
          {#each installed as th (th.name)}
            <div class="group flex items-center gap-1 rounded-lg {th.active ? 'bg-blue-600/20' : 'hover:bg-white/5'}">
              <button class="flex-1 min-w-0 flex items-center gap-2 px-2.5 py-2 text-left text-xs" on:click={() => apply(th.name)} title={th.path}>
                <span class="flex-1 truncate">{th.name}</span>
                {#if th.active}<Check size={13} class="text-blue-400 shrink-0" />{/if}
              </button>
              {#if th.removable}
                <button class="p-1.5 mr-1 rounded-md text-slate-500 hover:text-red-400 hover:bg-white/5 opacity-0 group-hover:opacity-100" title={$t('iconstore.remove')} on:click={() => remove(th)}><Trash2 size={13} /></button>
              {/if}
            </div>
          {:else}
            <p class="px-2.5 py-3 text-xs text-slate-600">{$t('iconstore.none')}</p>
          {/each}
        </div>
      </aside>
    {/if}
  </div>

  <!-- status line -->
  <div class="shrink-0 border-t border-white/5 px-3 py-2 text-xs flex items-center gap-2 min-h-[2.25rem] {status?.kind === 'error' ? 'text-red-300' : status?.kind === 'ok' ? 'text-emerald-300' : 'text-slate-400'}">
    {#if installing}<Loader2 size={13} class="animate-spin shrink-0" />{/if}
    <span class="truncate flex-1">{status ? status.text : $t('iconstore.hint')}</span>
    {#if status?.apply}
      {#each status.apply as name (name)}
        <button class="shrink-0 px-2.5 py-1 rounded-md bg-blue-600 hover:bg-blue-500 text-white" on:click={() => apply(name)}>{$t('iconstore.apply')} {status.apply.length > 1 ? name : ''}</button>
      {/each}
    {/if}
  </div>
</div>
