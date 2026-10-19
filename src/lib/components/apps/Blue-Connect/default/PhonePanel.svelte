<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    Smartphone, RefreshCw, Play, Square, Camera, ArrowLeft, Circle, LayoutGrid, Power, Volume1, Volume2, Lock, Unlock, Wifi, WifiOff,
    Folder, File as FileIcon, Download, Upload, Phone, PhoneOff, MessageSquare, Bell, Trash2, MonitorUp, Loader2, AlertTriangle, Usb, ChevronRight, PackagePlus, X,
  } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { dialogConfirm } from '../../../../stores/dialog';
  import { formatBytes } from '../../../../utils/mediaWriter';
  import { mapPointToDevice, classifyGesture, wheelToSwipe, keyFromEvent, coalesce, filterApps, formatKb, parentRemote, joinRemote, validHostPort, type Gesture, type KeyAction, type PhoneApp } from './phoneView';

  interface AdbStatus { installed: boolean; version: string; ffmpeg: boolean; scrcpy: boolean }
  interface PhoneDevice { serial: string; state: string; model: string; product: string; isWifi: boolean }
  interface PhoneInfo { manufacturer: string; model: string; android: string; sdk: number; screenW: number; screenH: number; batteryLevel: number | null; charging: boolean; batteryTempC: number | null; storageTotalKb: number; storageUsedKb: number; wifiIp: string; locked: boolean }
  interface RemoteEntry { name: string; isDir: boolean; size: number; modified: string }
  interface PhoneNotification { package: string; title: string; text: string }

  type SideTab = 'apps' | 'files' | 'notifications' | 'controls';

  let adb: AdbStatus | null = null;
  let devices: PhoneDevice[] = [];
  let serial = '';
  let info: PhoneInfo | null = null;
  let error = '';
  let notice = '';
  let loadingDevices = false;

  // connection helpers
  let showWifi = false;
  let pairAddr = '';
  let pairCode = '';
  let connectAddr = '';
  let busyConn = false;

  // screen
  let streaming = false;
  let frameSrc = '';
  let frameAspect = 0.5;
  let fps = 0;
  let quality: 'low' | 'medium' | 'high' = 'medium';
  let screenEl: HTMLDivElement | undefined;
  let downAt: { x: number; y: number; t: number } | null = null;
  let typed: KeyAction[] = [];
  let typeTimer: ReturnType<typeof setTimeout> | undefined;

  // side panel
  let tab: SideTab = 'apps';
  let apps: PhoneApp[] = [];
  let appQuery = '';
  let includeSystem = false;
  let appsLoading = false;
  let path = '/sdcard';
  let entries: RemoteEntry[] = [];
  let filesLoading = false;
  let notifications: PhoneNotification[] = [];
  let pin = '';
  let dialNumber = '';
  let smsBody = '';

  let unlistenFrame: UnlistenFn | undefined;
  let unlistenErr: UnlistenFn | undefined;
  let unlistenEnd: UnlistenFn | undefined;
  let pollTimer: ReturnType<typeof setInterval> | undefined;
  let frames = 0;
  let fpsTimer: ReturnType<typeof setInterval> | undefined;

  const call = <T,>(cmd: string, args: Record<string, unknown> = {}) => SystemBridge.invokeCommand<T>(cmd, args);
  $: selected = devices.find((d) => d.serial === serial) ?? null;
  $: ready = selected?.state === 'device';
  $: filteredApps = filterApps(apps, appQuery);
  // the phone may be rotated: swap the reported size when the frame is landscape
  $: dev = (() => {
    const w = info?.screenW || 1080, h = info?.screenH || 2400;
    const frameLandscape = frameAspect > 1;
    return frameLandscape === (w > h) ? { w, h } : { w: h, h: w };
  })();

  onMount(async () => {
    unlistenFrame = await listen<{ serial: string; src: string }>('blue-phone://frame', (e) => {
      if (e.payload.serial !== serial) return;
      frameSrc = e.payload.src; frames++;
    });
    unlistenErr = await listen<{ serial: string; message: string }>('blue-phone://stream-error', (e) => { if (e.payload.serial === serial) error = e.payload.message; });
    unlistenEnd = await listen<{ serial: string }>('blue-phone://stream-ended', (e) => { if (e.payload.serial === serial) streaming = false; });
    fpsTimer = setInterval(() => { fps = frames; frames = 0; }, 1000);
    adb = await call<AdbStatus>('phone_adb_status').catch(() => null);
    await refreshDevices();
    pollTimer = setInterval(() => { if (!streaming) void refreshDevices(true); }, 4000);
  });

  onDestroy(() => {
    unlistenFrame?.(); unlistenErr?.(); unlistenEnd?.();
    if (pollTimer) clearInterval(pollTimer); if (fpsTimer) clearInterval(fpsTimer);
    if (serial) void call('phone_stream_stop', { serial }).catch(() => {});
  });

  async function refreshDevices(quiet = false) {
    if (!quiet) loadingDevices = true;
    try {
      devices = await call<PhoneDevice[]>('phone_devices');
      if (!serial || !devices.some((d) => d.serial === serial)) {
        const first = devices.find((d) => d.state === 'device') ?? devices[0];
        if (first && first.serial !== serial) await select(first.serial);
        else if (!first) { serial = ''; info = null; streaming = false; }
      }
    } catch (e: any) { if (!quiet) error = String(e?.message ?? e); }
    loadingDevices = false;
  }

  async function select(s: string) {
    if (serial && serial !== s) { await call('phone_stream_stop', { serial }).catch(() => {}); streaming = false; frameSrc = ''; }
    serial = s; error = ''; notice = ''; info = null; apps = []; entries = []; notifications = [];
    if (devices.find((d) => d.serial === s)?.state === 'device') {
      info = await call<PhoneInfo>('phone_info', { serial: s }).catch((e) => { error = String(e?.message ?? e); return null; });
      void loadTab();
    }
  }

  function onFrameLoad(e: Event) {
    const i = e.currentTarget as HTMLImageElement;
    if (i.naturalHeight) frameAspect = i.naturalWidth / i.naturalHeight;
  }

  async function loadTab() {
    if (!ready) return;
    if (tab === 'apps' && !apps.length) await loadApps();
    else if (tab === 'files') await loadDir();
    else if (tab === 'notifications') await loadNotifications();
  }
  const setTab = (x: string) => { tab = x as SideTab; void loadTab(); };

  async function guard<T>(fn: () => Promise<T>, ok = ''): Promise<T | undefined> {
    error = ''; notice = '';
    try { const r = await fn(); if (ok) notice = ok; return r; } catch (e: any) { error = String(e?.message ?? e); return undefined; }
  }

  // ── connection ──
  async function doPair() {
    busyConn = true;
    const r = await guard(() => call<string>('phone_pair', { hostPort: pairAddr.trim(), code: pairCode.trim() }), $t('phone.paired'));
    busyConn = false;
    if (r !== undefined) { pairCode = ''; }
  }
  async function doConnect() {
    busyConn = true;
    const r = await guard(() => call<string>('phone_connect', { hostPort: connectAddr.trim() }), $t('phone.connected'));
    busyConn = false;
    if (r !== undefined) await refreshDevices();
  }
  async function toWifi() {
    busyConn = true;
    const r = await guard(() => call<string>('phone_enable_wifi', { serial }), $t('phone.wifi_on'));
    busyConn = false;
    if (r !== undefined) await refreshDevices();
  }

  // ── stream ──
  async function startStream() {
    if (!ready) return;
    frames = 0;
    const r = await guard(() => call('phone_stream_start', { serial, maxSide: quality === 'high' ? 1920 : quality === 'low' ? 960 : 1280, quality }));
    if (r !== undefined || !error) streaming = true;
  }
  async function stopStream() { await call('phone_stream_stop', { serial }).catch(() => {}); streaming = false; }
  async function screenshot() {
    const dir = await SystemBridge.pickDirectory('blue-phone.screenshots');
    if (dir) await guard(() => call<string>('phone_screenshot', { serial, destDir: dir }).then((p) => { notice = $t('phone.screenshot_saved').replace('{path}', p); }));
  }
  const scrcpy = () => guard(() => call('phone_open_scrcpy', { serial, turnScreenOff: false, stayAwake: true }));

  // ── input ──
  const send = (input: object) => call('phone_input', { serial, input }).catch((e) => { error = String(e?.message ?? e); });
  const key = (name: string) => send({ kind: 'key', name });

  function pointerDown(e: PointerEvent) {
    if (!screenEl) return;
    screenEl.focus();
    const p = mapPointToDevice(e.clientX, e.clientY, screenEl.getBoundingClientRect(), dev.w, dev.h);
    if (!p) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    downAt = { ...p, t: performance.now() };
  }
  function pointerUp(e: PointerEvent) {
    if (!downAt || !screenEl) return;
    const r = screenEl.getBoundingClientRect();
    const up = mapPointToDevice(e.clientX, e.clientY, r, dev.w, dev.h) ?? mapPointToDevice(Math.max(r.left, Math.min(r.right, e.clientX)), Math.max(r.top, Math.min(r.bottom, e.clientY)), r, dev.w, dev.h);
    const g: Gesture = classifyGesture(downAt, up ?? downAt, performance.now() - downAt.t);
    downAt = null;
    void send(g);
  }
  function wheel(e: WheelEvent) {
    if (!screenEl) return;
    const p = mapPointToDevice(e.clientX, e.clientY, screenEl.getBoundingClientRect(), dev.w, dev.h);
    if (!p) return;
    e.preventDefault();
    const g = wheelToSwipe(e.deltaY, p, dev.h);
    if (g) void send(g);
  }
  function keydown(e: KeyboardEvent) {
    const a = keyFromEvent(e);
    if (!a) return;
    e.preventDefault();
    typed.push(a);
    if (typeTimer) clearTimeout(typeTimer);
    typeTimer = setTimeout(flushTyped, 110);
  }
  async function flushTyped() {
    const batch = coalesce(typed); typed = [];
    for (const a of batch) await send(a);
  }

  async function unlock() {
    if (!info) return;
    await guard(() => call('phone_unlock', { serial, pin: pin || null, screenW: info!.screenW, screenH: info!.screenH }), $t('phone.unlocked'));
    pin = '';
  }

  // ── apps ──
  async function loadApps() {
    appsLoading = true;
    apps = (await guard(() => call<PhoneApp[]>('phone_apps', { serial, includeSystem }))) ?? [];
    appsLoading = false;
  }
  const launch = (a: PhoneApp) => guard(() => call('phone_launch_app', { serial, package: a.package }));
  const stopApp = (a: PhoneApp) => guard(() => call('phone_stop_app', { serial, package: a.package }), $t('phone.app_stopped'));
  async function uninstall(a: PhoneApp) {
    const ok = await dialogConfirm({ title: $t('phone.uninstall_title'), message: $t('phone.uninstall_body').replace('{app}', a.label), confirmLabel: $t('phone.uninstall'), cancelLabel: $t('common.cancel'), danger: true });
    if (ok && (await guard(() => call('phone_uninstall_app', { serial, package: a.package }), $t('phone.uninstalled'))) !== undefined) await loadApps();
  }
  async function installApk() {
    const f = await SystemBridge.pickFile([{ name: 'APK', extensions: ['apk'] }], $t('phone.install_apk'), 'blue-phone.apk');
    if (f && (await guard(() => call('phone_install_apk', { serial, path: f }), $t('phone.installed'))) !== undefined) await loadApps();
  }

  // ── files ──
  async function loadDir() {
    filesLoading = true;
    entries = (await guard(() => call<RemoteEntry[]>('phone_ls', { serial, path }))) ?? [];
    filesLoading = false;
  }
  async function openEntry(e: RemoteEntry) { if (e.isDir) { path = joinRemote(path, e.name); await loadDir(); } }
  async function up() { path = parentRemote(path); await loadDir(); }
  async function pull(e: RemoteEntry) {
    const dir = await SystemBridge.pickDirectory('blue-phone.pull');
    if (dir) await guard(() => call<string>('phone_pull', { serial, remote: joinRemote(path, e.name), localDir: dir }), $t('phone.copied_to_pc'));
  }
  async function push() {
    const f = await SystemBridge.pickFile([], $t('phone.send_file'), 'blue-phone.push');
    if (f && (await guard(() => call<string>('phone_push', { serial, local: f, remoteDir: path }), $t('phone.sent_to_phone'))) !== undefined) await loadDir();
  }

  // ── notifications, calls, toggles ──
  async function loadNotifications() { notifications = (await guard(() => call<PhoneNotification[]>('phone_notifications', { serial }))) ?? []; }
  const dial = (placeCall: boolean) => guard(() => call('phone_dial', { serial, number: dialNumber, placeCall }));
  const callCtl = (action: 'answer' | 'end') => guard(() => call('phone_call_control', { serial, action }));
  const composeSms = () => guard(() => call('phone_compose_sms', { serial, number: dialNumber, body: smsBody }), $t('phone.sms_opened'));
  const toggle = (what: string, enabled: boolean) => guard(() => call('phone_toggle', { serial, what, enabled }));
</script>

<div class="flex flex-col h-full min-h-0">
  <!-- toolbar -->
  <div class="flex flex-wrap items-center gap-2 px-4 py-2 border-b border-white/5 bg-slate-900/60">
    <Smartphone class="w-4 h-4 text-blue-400" />
    <select class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1 text-xs max-w-[16rem]" value={serial} on:change={(e) => select(e.currentTarget.value)} aria-label={$t('phone.device')}>
      {#if devices.length === 0}<option value="">{$t('phone.no_device')}</option>{/if}
      {#each devices as d}<option value={d.serial}>{d.model || d.serial}{d.isWifi ? ' · Wi-Fi' : ' · USB'}{d.state !== 'device' ? ` (${d.state})` : ''}</option>{/each}
    </select>
    <button class="p-1.5 rounded hover:bg-white/10" on:click={() => refreshDevices()} aria-label={$t('phone.refresh')}><RefreshCw class="w-3.5 h-3.5 {loadingDevices ? 'animate-spin' : ''}" /></button>
    <button class="flex items-center gap-1 px-2 py-1 rounded-lg text-xs bg-slate-800 hover:bg-slate-700" on:click={() => (showWifi = !showWifi)}><Wifi class="w-3.5 h-3.5" /> {$t('phone.wireless')}</button>
    <div class="flex-1"></div>
    {#if ready}
      {#if streaming}
        <span class="text-[11px] text-slate-400 tabular-nums">{fps} fps</span>
        <button class="flex items-center gap-1 px-2.5 py-1 rounded-lg text-xs bg-red-600 hover:bg-red-500" on:click={stopStream}><Square class="w-3 h-3" /> {$t('phone.stop')}</button>
      {:else}
        <select class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1 text-xs" bind:value={quality} aria-label={$t('phone.quality')}>
          <option value="low">{$t('phone.q_low')}</option><option value="medium">{$t('phone.q_medium')}</option><option value="high">{$t('phone.q_high')}</option>
        </select>
        <button class="flex items-center gap-1 px-2.5 py-1 rounded-lg text-xs bg-blue-600 hover:bg-blue-500" on:click={startStream}><Play class="w-3 h-3" /> {$t('phone.show_screen')}</button>
      {/if}
      <button class="p-1.5 rounded hover:bg-white/10" on:click={screenshot} title={$t('phone.screenshot')} aria-label={$t('phone.screenshot')}><Camera class="w-3.5 h-3.5" /></button>
      {#if adb?.scrcpy}<button class="p-1.5 rounded hover:bg-white/10" on:click={scrcpy} title={$t('phone.scrcpy')} aria-label={$t('phone.scrcpy')}><MonitorUp class="w-3.5 h-3.5" /></button>{/if}
    {/if}
  </div>

  {#if showWifi}
    <div class="px-4 py-3 border-b border-white/5 bg-slate-900/40 grid gap-3 text-xs">
      <p class="text-slate-400">{$t('phone.wireless_help')}</p>
      <div class="flex flex-wrap items-end gap-2">
        <label class="grid gap-1 text-slate-400">{$t('phone.pair_addr')}<input bind:value={pairAddr} placeholder="192.168.1.20:37123" class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-white w-44" /></label>
        <label class="grid gap-1 text-slate-400">{$t('phone.pair_code')}<input bind:value={pairCode} inputmode="numeric" maxlength="6" placeholder="123456" class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-white w-24" /></label>
        <button on:click={doPair} disabled={busyConn || !validHostPort(pairAddr) || pairCode.length !== 6} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 disabled:opacity-40">{$t('phone.pair')}</button>
        <label class="grid gap-1 text-slate-400 ml-3">{$t('phone.connect_addr')}<input bind:value={connectAddr} placeholder="192.168.1.20:41234" class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-white w-44" /></label>
        <button on:click={doConnect} disabled={busyConn || !validHostPort(connectAddr)} class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-40">{$t('phone.connect')}</button>
        {#if ready && selected && !selected.isWifi}<button on:click={toWifi} disabled={busyConn} class="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 ml-3"><Usb class="w-3.5 h-3.5" /> {$t('phone.usb_to_wifi')}</button>{/if}
        {#if ready && selected?.isWifi}<button on:click={() => call('phone_disconnect', { serial }).then(() => refreshDevices())} class="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600 ml-3"><WifiOff class="w-3.5 h-3.5" /> {$t('phone.disconnect')}</button>{/if}
      </div>
    </div>
  {/if}

  {#if error}<div class="mx-4 mt-3 flex items-start gap-2 bg-red-500/10 border border-red-500/30 text-red-200 rounded-lg px-3 py-2 text-xs" role="alert"><AlertTriangle class="w-4 h-4 mt-0.5 shrink-0" /><span class="break-words min-w-0 flex-1">{error}</span><button on:click={() => (error = '')} aria-label={$t('common.close')}><X class="w-3.5 h-3.5" /></button></div>{/if}
  {#if notice}<div class="mx-4 mt-3 bg-green-500/10 border border-green-500/30 text-green-200 rounded-lg px-3 py-2 text-xs break-words" role="status">{notice}</div>{/if}

  {#if adb && !adb.installed}
    <div class="m-4 p-4 rounded-xl bg-slate-900 border border-white/10 text-sm space-y-2">
      <div class="font-medium text-white">{$t('phone.need_adb')}</div>
      <p class="text-slate-400 text-xs">{$t('phone.need_adb_hint')}</p>
      <code class="block bg-black/40 rounded px-3 py-2 text-xs text-slate-200">sudo apt install adb ffmpeg scrcpy</code>
    </div>
  {:else if devices.length === 0 && !loadingDevices}
    <div class="m-4 p-5 rounded-xl bg-slate-900 border border-dashed border-white/10 text-sm space-y-2">
      <div class="font-medium text-white flex items-center gap-2"><Smartphone class="w-4 h-4" /> {$t('phone.setup_title')}</div>
      <ol class="list-decimal pl-5 text-xs text-slate-400 space-y-1">
        <li>{$t('phone.setup_1')}</li><li>{$t('phone.setup_2')}</li><li>{$t('phone.setup_3')}</li><li>{$t('phone.setup_4')}</li>
      </ol>
    </div>
  {:else if selected && selected.state !== 'device'}
    <div class="m-4 p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 text-xs text-amber-100">{selected.state === 'unauthorized' ? $t('phone.unauthorized') : $t('phone.offline')}</div>
  {:else if ready}
    <div class="flex-1 min-h-0 flex gap-4 p-4">
      <!-- phone screen -->
      <div class="flex flex-col items-center gap-2 shrink-0" style="width:min(42%,340px)">
        <!-- svelte-ignore a11y-no-noninteractive-tabindex a11y-no-noninteractive-element-interactions -->
        <div bind:this={screenEl} tabindex="0" role="application" aria-label={$t('phone.screen')}
             class="relative w-full flex-1 min-h-[200px] rounded-3xl bg-black border-4 border-slate-700 overflow-hidden outline-none focus:border-blue-500 touch-none select-none"
             on:pointerdown={pointerDown} on:pointerup={pointerUp} on:pointercancel={() => (downAt = null)} on:wheel={wheel} on:keydown={keydown}>
          {#if frameSrc && streaming}
            <img src={frameSrc} alt="" draggable="false" class="absolute inset-0 w-full h-full object-contain pointer-events-none"
                 on:load={onFrameLoad} />
          {:else}
            <div class="absolute inset-0 grid place-items-center text-center text-slate-500 text-xs p-4">
              {#if streaming}<Loader2 class="w-6 h-6 animate-spin" />{:else}<div><Smartphone class="w-10 h-10 mx-auto mb-2 text-slate-600" />{$t('phone.press_show')}</div>{/if}
            </div>
          {/if}
        </div>
        <div class="flex items-center gap-1.5">
          <button class="p-2 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => key('back')} aria-label={$t('phone.back')}><ArrowLeft class="w-4 h-4" /></button>
          <button class="p-2 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => key('home')} aria-label={$t('phone.home')}><Circle class="w-4 h-4" /></button>
          <button class="p-2 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => key('recents')} aria-label={$t('phone.recents')}><LayoutGrid class="w-4 h-4" /></button>
          <div class="w-px h-5 bg-white/10 mx-1"></div>
          <button class="p-2 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => key('volume_down')} aria-label={$t('phone.vol_down')}><Volume1 class="w-4 h-4" /></button>
          <button class="p-2 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => key('volume_up')} aria-label={$t('phone.vol_up')}><Volume2 class="w-4 h-4" /></button>
          <button class="p-2 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => key('power')} aria-label={$t('phone.power')}><Power class="w-4 h-4" /></button>
        </div>
        <p class="text-[10px] text-slate-500 text-center">{$t('phone.screen_hint')}{adb && !adb.ffmpeg ? ` ${$t('phone.no_ffmpeg')}` : ''}</p>
      </div>

      <!-- side panel -->
      <div class="flex-1 min-w-0 flex flex-col min-h-0">
        <div class="flex gap-1 mb-3 text-xs" role="tablist">
          {#each [['apps', 'phone.tab.apps'], ['files', 'phone.tab.files'], ['notifications', 'phone.tab.notifications'], ['controls', 'phone.tab.controls']] as [id, label]}
            <button role="tab" aria-selected={tab === id} on:click={() => setTab(id)} class="px-3 py-1.5 rounded-lg {tab === id ? 'bg-blue-600' : 'bg-slate-800 hover:bg-slate-700 text-slate-300'}">{$t(label)}</button>
          {/each}
        </div>

        <div class="flex-1 min-h-0 overflow-y-auto pr-1">
          {#if tab === 'apps'}
            <div class="flex items-center gap-2 mb-2">
              <input bind:value={appQuery} placeholder={$t('phone.search_apps')} class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs outline-none focus:border-blue-500" />
              <label class="flex items-center gap-1 text-[11px] text-slate-400"><input type="checkbox" bind:checked={includeSystem} on:change={loadApps} /> {$t('phone.system_apps')}</label>
              <button on:click={installApk} class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-xs bg-slate-800 hover:bg-slate-700"><PackagePlus class="w-3.5 h-3.5" /> APK</button>
            </div>
            {#if appsLoading}<Loader2 class="w-4 h-4 animate-spin text-slate-400" />{/if}
            <div class="grid gap-1">
              {#each filteredApps as a (a.package)}
                <div class="group flex items-center gap-2 px-3 py-2 rounded-lg bg-slate-900 hover:bg-slate-800">
                  <button class="flex-1 min-w-0 text-left" on:click={() => launch(a)}>
                    <div class="text-sm truncate">{a.label}</div><div class="text-[10px] text-slate-500 truncate">{a.package}</div>
                  </button>
                  <button class="opacity-0 group-hover:opacity-100 focus:opacity-100 text-[11px] px-2 py-1 rounded bg-slate-700 hover:bg-slate-600" on:click={() => stopApp(a)}>{$t('phone.force_stop')}</button>
                  <button class="opacity-0 group-hover:opacity-100 focus:opacity-100 p-1 rounded hover:bg-red-500/20 text-slate-400 hover:text-red-300" on:click={() => uninstall(a)} aria-label={$t('phone.uninstall')}><Trash2 class="w-3.5 h-3.5" /></button>
                </div>
              {:else}
                {#if !appsLoading}<div class="text-xs text-slate-500">{$t('phone.no_apps')}</div>{/if}
              {/each}
            </div>

          {:else if tab === 'files'}
            <div class="flex items-center gap-2 mb-2 text-xs">
              <button class="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700" on:click={up} disabled={path === '/'}>↑</button>
              <div class="flex-1 min-w-0 truncate font-mono text-slate-300">{path}</div>
              <button on:click={push} class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700"><Upload class="w-3.5 h-3.5" /> {$t('phone.send_file')}</button>
            </div>
            <div class="flex gap-1.5 mb-2 text-[11px]">
              {#each ['/sdcard', '/sdcard/DCIM', '/sdcard/Download', '/sdcard/Pictures'] as p}<button class="px-2 py-0.5 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => { path = p; loadDir(); }}>{p.replace('/sdcard', '') || 'sdcard'}</button>{/each}
            </div>
            {#if filesLoading}<Loader2 class="w-4 h-4 animate-spin text-slate-400" />{/if}
            <div class="grid gap-0.5">
              {#each entries as e (e.name)}
                <div class="group flex items-center gap-2 px-2 py-1.5 rounded hover:bg-slate-800">
                  <button class="flex items-center gap-2 flex-1 min-w-0 text-left" on:click={() => openEntry(e)} disabled={!e.isDir}>
                    {#if e.isDir}<Folder class="w-4 h-4 text-blue-400 shrink-0" />{:else}<FileIcon class="w-4 h-4 text-slate-400 shrink-0" />{/if}
                    <span class="text-sm truncate">{e.name}</span>
                    {#if !e.isDir}<span class="ml-auto text-[11px] text-slate-500 shrink-0">{formatBytes(e.size)}</span>{/if}
                  </button>
                  <button class="opacity-0 group-hover:opacity-100 focus:opacity-100 p-1 rounded hover:bg-white/10" on:click={() => pull(e)} aria-label={$t('phone.copy_to_pc')} title={$t('phone.copy_to_pc')}><Download class="w-3.5 h-3.5" /></button>
                </div>
              {/each}
            </div>

          {:else if tab === 'notifications'}
            <div class="flex items-center justify-between mb-2"><h3 class="text-sm font-medium flex items-center gap-2"><Bell class="w-4 h-4" /> {$t('phone.tab.notifications')}</h3><button on:click={loadNotifications} class="text-xs text-slate-400 hover:text-white flex items-center gap-1"><RefreshCw class="w-3 h-3" /> {$t('phone.refresh')}</button></div>
            <div class="grid gap-1.5">
              {#each notifications as n}
                <button class="text-left px-3 py-2 rounded-lg bg-slate-900 hover:bg-slate-800" on:click={() => key('home')}>
                  <div class="text-[10px] text-slate-500 truncate">{n.package}</div>
                  <div class="text-sm truncate">{n.title}</div>
                  {#if n.text}<div class="text-xs text-slate-400 line-clamp-2">{n.text}</div>{/if}
                </button>
              {:else}<div class="text-xs text-slate-500">{$t('phone.no_notifications')}</div>{/each}
            </div>

          {:else}
            {#if info}
              <div class="grid grid-cols-2 gap-2 mb-4 text-xs">
                <div class="bg-slate-900 rounded-lg p-3"><div class="text-slate-500">{$t('phone.model')}</div><div class="text-sm">{info.manufacturer} {info.model}</div><div class="text-slate-500 mt-0.5">Android {info.android}</div></div>
                <div class="bg-slate-900 rounded-lg p-3"><div class="text-slate-500">{$t('phone.battery')}</div><div class="text-sm">{info.batteryLevel ?? '–'}%{info.charging ? ' ⚡' : ''}</div>{#if info.batteryTempC}<div class="text-slate-500 mt-0.5">{info.batteryTempC.toFixed(1)} °C</div>{/if}</div>
                <div class="bg-slate-900 rounded-lg p-3"><div class="text-slate-500">{$t('phone.storage')}</div><div class="text-sm">{formatKb(info.storageUsedKb)} / {formatKb(info.storageTotalKb)}</div></div>
                <div class="bg-slate-900 rounded-lg p-3"><div class="text-slate-500">Wi-Fi</div><div class="text-sm font-mono">{info.wifiIp || '–'}</div></div>
              </div>
            {/if}

            <section class="mb-4">
              <h3 class="text-sm font-medium mb-1.5 flex items-center gap-2"><Lock class="w-4 h-4" /> {$t('phone.unlock')}</h3>
              <p class="text-[11px] text-slate-500 mb-2">{$t('phone.unlock_hint')}</p>
              <div class="flex gap-2"><input type="password" inputmode="numeric" autocomplete="off" bind:value={pin} placeholder={$t('phone.pin_optional')} class="bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs w-40" /><button on:click={unlock} class="flex items-center gap-1 px-3 py-1.5 rounded-lg text-xs bg-blue-600 hover:bg-blue-500"><Unlock class="w-3.5 h-3.5" /> {$t('phone.unlock')}</button></div>
            </section>

            <section class="mb-4">
              <h3 class="text-sm font-medium mb-1.5 flex items-center gap-2"><Phone class="w-4 h-4" /> {$t('phone.calls_sms')}</h3>
              <input bind:value={dialNumber} placeholder="+48 600 700 800" class="bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs w-full mb-2" />
              <div class="flex flex-wrap gap-2 mb-2 text-xs">
                <button on:click={() => dial(false)} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600">{$t('phone.open_dialer')}</button>
                <button on:click={() => dial(true)} class="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-green-700 hover:bg-green-600"><Phone class="w-3.5 h-3.5" /> {$t('phone.call')}</button>
                <button on:click={() => callCtl('answer')} class="px-3 py-1.5 rounded-lg bg-slate-700 hover:bg-slate-600">{$t('phone.answer')}</button>
                <button on:click={() => callCtl('end')} class="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-red-700 hover:bg-red-600"><PhoneOff class="w-3.5 h-3.5" /> {$t('phone.hang_up')}</button>
              </div>
              <div class="flex gap-2"><input bind:value={smsBody} placeholder={$t('phone.sms_text')} class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs" /><button on:click={composeSms} disabled={!dialNumber || !smsBody} class="flex items-center gap-1 px-3 py-1.5 rounded-lg text-xs bg-slate-700 hover:bg-slate-600 disabled:opacity-40"><MessageSquare class="w-3.5 h-3.5" /> {$t('phone.sms_compose')}</button></div>
            </section>

            <section>
              <h3 class="text-sm font-medium mb-1.5">{$t('phone.quick')}</h3>
              <div class="grid grid-cols-2 gap-2 text-xs">
                {#each [['wifi', 'Wi-Fi'], ['data', 'phone.mobile_data'], ['stay_awake', 'phone.stay_awake'], ['auto_rotate', 'phone.auto_rotate']] as [w, label]}
                  <div class="flex items-center justify-between bg-slate-900 rounded-lg px-3 py-2"><span>{label.startsWith('phone.') ? $t(label) : label}</span>
                    <span class="flex gap-1"><button class="px-2 py-0.5 rounded bg-green-700/70 hover:bg-green-600" on:click={() => toggle(w, true)}>{$t('phone.on')}</button><button class="px-2 py-0.5 rounded bg-slate-700 hover:bg-slate-600" on:click={() => toggle(w, false)}>{$t('phone.off')}</button></span></div>
                {/each}
              </div>
            </section>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>
