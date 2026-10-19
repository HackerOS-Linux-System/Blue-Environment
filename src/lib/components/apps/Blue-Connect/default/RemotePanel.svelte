<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import qrcode from 'qrcode-generator';
  import { Tv, Smartphone, QrCode, Trash2, Loader2, AlertTriangle, ChevronUp, ChevronDown, ChevronLeft, ChevronRight, ArrowLeft, Home, Menu as MenuIcon, Play, SkipBack, SkipForward, Square, Volume1, Volume2, VolumeX, MousePointer2, Link2, ShieldAlert } from 'lucide-svelte';
  import { t } from '../../../../stores/language';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { dialogConfirm } from '../../../../stores/dialog';
  import type { DiscoveredDevice } from './types';

  interface RemoteInfo { id: string; name: string; created: number; lastSeen: number }
  interface RemoteStatus { enabled: boolean; running: boolean; systemInput: boolean; ydotool: boolean; port: number; urls: string[]; pairingOpen: boolean; pairingCode: string | null; pairingExpiresAt: number | null; remotes: RemoteInfo[] }
  interface Target { id: string; name: string; host: string; port: number }

  const call = <T,>(cmd: string, args: Record<string, unknown> = {}) => SystemBridge.invokeCommand<T>(cmd, args);

  // ── receiver (this device is the TV / is controlled) ──
  let status: RemoteStatus | null = null;
  let error = '';
  let notice = '';
  let now = Math.floor(Date.now() / 1000);
  let timer: ReturnType<typeof setInterval> | undefined;
  let unlistenPaired: UnlistenFn | undefined;

  // ── controller (this device controls a TV) ──
  let targets: Target[] = [];
  let current = '';
  let hostInput = '';
  let codeInput = '';
  let busy = false;
  let suggestions: { name: string; host: string }[] = [];

  $: remaining = status?.pairingExpiresAt ? Math.max(0, status.pairingExpiresAt - now) : 0;
  $: url = status?.urls[0] ?? '';
  $: qrSvg = status?.pairingOpen && url ? makeQr(url) : '';
  $: isTv = typeof document !== 'undefined' && !!document.querySelector('[data-alt-shell="tv"]');

  function makeQr(text: string): string {
    const qr = qrcode(0, 'M');
    qr.addData(text);
    qr.make();
    const n = qr.getModuleCount();
    const cell = 6; const m = 3; const size = (n + m * 2) * cell;
    let d = '';
    for (let r = 0; r < n; r++) for (let c = 0; c < n; c++) if (qr.isDark(r, c)) d += `M${(c + m) * cell} ${(r + m) * cell}h${cell}v${cell}h-${cell}z`;
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" shape-rendering="crispEdges" role="img" aria-label="QR"><rect width="${size}" height="${size}" fill="#fff"/><path d="${d}" fill="#000"/></svg>`;
  }

  async function refresh() { status = await call<RemoteStatus>('remote_server_status').catch(() => null); }
  async function loadTargets() {
    targets = await call<Target[]>('remote_targets').catch(() => []);
    if (!targets.some((x) => x.id === current)) current = targets[0]?.id ?? '';
  }

  onMount(async () => {
    await Promise.all([refresh(), loadTargets()]);
    timer = setInterval(() => { now = Math.floor(Date.now() / 1000); if (status?.pairingOpen) void refresh(); }, 1000);
    unlistenPaired = await listen<{ name: string }>('blue-remote://paired', (e) => { notice = $t('remote.paired_ok').replace('{name}', e.payload.name); void refresh(); });
    try {
      const ds: DiscoveredDevice[] = await SystemBridge.bcGetDevices();
      suggestions = ds.filter((d) => d.deviceType === 'tv').map((d) => ({ name: d.name, host: d.address.replace(/:\d+$/, '') }));
    } catch { suggestions = []; }
  });
  onDestroy(() => { if (timer) clearInterval(timer); unlistenPaired?.(); });

  async function guard<T>(fn: () => Promise<T>): Promise<T | undefined> {
    error = ''; notice = '';
    try { return await fn(); } catch (e: any) { error = String(e?.message ?? e); return undefined; }
  }

  async function setEnabled(enabled: boolean, systemInput = status?.systemInput ?? false) {
    status = (await guard(() => call<RemoteStatus>('remote_set_enabled', { enabled, systemInput }))) ?? status;
  }
  async function openPairing() { status = (await guard(() => call<RemoteStatus>('remote_pairing_open', { minutes: 5 }))) ?? status; }
  async function closePairing() { status = (await guard(() => call<RemoteStatus>('remote_pairing_close'))) ?? status; }
  async function revoke(r: RemoteInfo) {
    const ok = await dialogConfirm({ title: $t('remote.revoke_title'), message: $t('remote.revoke_body').replace('{name}', r.name), confirmLabel: $t('remote.revoke'), cancelLabel: $t('common.cancel'), danger: true });
    if (ok) status = (await guard(() => call<RemoteStatus>('remote_revoke', { id: r.id }))) ?? status;
  }

  async function pairWith(host: string, code: string) {
    busy = true;
    const r = await guard(() => call<Target>('remote_pair_with', { host: host.trim(), port: null, code: code.trim() }));
    busy = false;
    if (r) { notice = $t('remote.pair_done').replace('{name}', r.name); codeInput = ''; hostInput = ''; await loadTargets(); current = r.id; }
  }
  async function forget(id: string) { await guard(() => call('remote_forget_target', { id })); await loadTargets(); }
  let textValue = '';
  function sendText() { if (textValue) { send({ type: 'text', text: textValue }); textValue = ''; } }
  const send = (input: object) => { if (current) void guard(() => call('remote_send', { target: current, input })); };
  const fmtAgo = (ts: number) => { const s = Math.max(0, now - ts); return s < 90 ? $t('remote.just_now') : s < 5400 ? `${Math.round(s / 60)} min` : s < 129600 ? `${Math.round(s / 3600)} h` : `${Math.round(s / 86400)} d`; };
  const mmss = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
</script>

<div class="flex-1 overflow-y-auto p-4 space-y-6">
  {#if error}<div class="flex items-start gap-2 bg-red-500/10 border border-red-500/30 text-red-200 rounded-lg px-3 py-2 text-xs" role="alert"><AlertTriangle class="w-4 h-4 mt-0.5 shrink-0" /><span class="break-words">{error}</span></div>{/if}
  {#if notice}<div class="bg-green-500/10 border border-green-500/30 text-green-200 rounded-lg px-3 py-2 text-xs" role="status">{notice}</div>{/if}

  <!-- ───────── receiver ───────── -->
  <section class="space-y-3 {isTv ? 'order-first' : ''}">
    <div class="flex items-center gap-2"><Tv class="w-4 h-4 text-blue-400" /><h2 class="text-sm font-semibold">{$t('remote.receiver_title')}</h2></div>
    <p class="text-xs text-slate-400">{$t('remote.receiver_desc')}</p>

    {#if status}
      <label class="flex items-center justify-between bg-slate-900 rounded-xl px-4 py-3 text-sm">
        <span>{$t('remote.enable')}<span class="block text-[11px] text-slate-500">{status.running ? $t('remote.listening').replace('{port}', String(status.port)) : $t('remote.not_listening')}</span></span>
        <input type="checkbox" checked={status.enabled} on:change={(e) => setEnabled(e.currentTarget.checked)} class="w-5 h-5" />
      </label>

      {#if status.enabled}
        <label class="flex items-center justify-between bg-slate-900 rounded-xl px-4 py-3 text-sm {status.ydotool ? '' : 'opacity-60'}">
          <span>{$t('remote.system_input')}<span class="block text-[11px] text-slate-500">{status.ydotool ? $t('remote.system_input_hint') : $t('remote.no_ydotool')}</span></span>
          <input type="checkbox" checked={status.systemInput} disabled={!status.ydotool} on:change={(e) => setEnabled(true, e.currentTarget.checked)} class="w-5 h-5" />
        </label>
      {/if}

      {#if status.pairingOpen}
        <div class="bg-slate-900 rounded-2xl p-5 grid sm:grid-cols-[auto_1fr] gap-5 items-center border border-blue-500/30">
          {#if qrSvg}<div class="w-40 h-40 sm:w-48 sm:h-48 rounded-xl overflow-hidden bg-white">{@html qrSvg}</div>{/if}
          <div class="space-y-2 min-w-0">
            <div class="text-xs text-slate-400">{$t('remote.step1')}</div>
            <div class="font-mono text-sm break-all text-blue-300">{status.urls.join('  ·  ') || $t('remote.no_network')}</div>
            <div class="text-xs text-slate-400 pt-1">{$t('remote.step2')}</div>
            <div class="text-4xl font-bold tracking-[0.35em] tabular-nums" aria-live="polite">{status.pairingCode}</div>
            <div class="text-[11px] text-slate-500">{$t('remote.expires').replace('{t}', mmss(remaining))}</div>
            <button on:click={closePairing} class="text-xs px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700">{$t('common.cancel')}</button>
          </div>
        </div>
      {:else}
        <button on:click={openPairing} class="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm"><QrCode class="w-4 h-4" /> {$t('remote.pair_a_remote')}</button>
      {/if}

      {#if status.remotes.length}
        <div class="grid gap-1.5">
          {#each status.remotes as r (r.id)}
            <div class="flex items-center gap-3 bg-slate-900 rounded-xl px-4 py-2.5">
              <Smartphone class="w-4 h-4 text-slate-400 shrink-0" />
              <div class="min-w-0 flex-1"><div class="text-sm truncate">{r.name}</div><div class="text-[11px] text-slate-500">{$t('remote.last_used').replace('{t}', fmtAgo(r.lastSeen))}</div></div>
              <button on:click={() => revoke(r)} class="p-1.5 rounded hover:bg-red-500/20 text-slate-400 hover:text-red-300" aria-label={$t('remote.revoke')}><Trash2 class="w-4 h-4" /></button>
            </div>
          {/each}
        </div>
      {/if}

      <p class="flex items-start gap-2 text-[11px] text-slate-500"><ShieldAlert class="w-3.5 h-3.5 mt-0.5 shrink-0" /> {$t('remote.security_note')}</p>
    {:else}
      <Loader2 class="w-5 h-5 animate-spin text-slate-400" />
    {/if}
  </section>

  <!-- ───────── controller ───────── -->
  <section class="space-y-3">
    <div class="flex items-center gap-2"><MousePointer2 class="w-4 h-4 text-blue-400" /><h2 class="text-sm font-semibold">{$t('remote.controller_title')}</h2></div>
    <p class="text-xs text-slate-400">{$t('remote.controller_desc')}</p>

    {#if targets.length}
      <div class="flex flex-wrap gap-2">
        {#each targets as tg (tg.id)}
          <div class="flex items-center rounded-lg border {current === tg.id ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 bg-slate-900'}">
            <button class="flex items-center gap-1.5 px-3 py-1.5 text-xs" on:click={() => (current = tg.id)}><Tv class="w-3.5 h-3.5" /> {tg.name} <span class="text-slate-500">{tg.host}</span></button>
            <button class="px-2 text-slate-500 hover:text-red-300" on:click={() => forget(tg.id)} aria-label={$t('remote.forget')}>×</button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="bg-slate-900 rounded-xl p-3 space-y-2">
      <div class="text-xs text-slate-400">{$t('remote.add_tv')}</div>
      {#if suggestions.length}
        <div class="flex flex-wrap gap-1.5">{#each suggestions as s}<button class="text-[11px] px-2 py-1 rounded-full bg-slate-800 hover:bg-slate-700" on:click={() => (hostInput = s.host)}>{s.name} · {s.host}</button>{/each}</div>
      {/if}
      <div class="flex flex-wrap items-end gap-2">
        <input bind:value={hostInput} placeholder="192.168.1.30" class="bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs w-44" aria-label={$t('remote.tv_address')} />
        <input bind:value={codeInput} inputmode="numeric" maxlength="6" placeholder="000000" class="bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs w-24 tracking-widest" aria-label={$t('remote.code')} />
        <button on:click={() => pairWith(hostInput, codeInput)} disabled={busy || !hostInput.trim() || codeInput.trim().length !== 6} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-40"><Link2 class="w-3.5 h-3.5" /> {$t('blueconnect.pair')}</button>
      </div>
    </div>

    {#if current}
      <div class="max-w-xs mx-auto bg-slate-900 rounded-3xl p-4 space-y-3">
        <div class="grid grid-cols-3 gap-2">
          <span></span><button class="rb" on:click={() => send({ type: 'key', name: 'up' })} aria-label="Up"><ChevronUp class="w-6 h-6 mx-auto" /></button><span></span>
          <button class="rb" on:click={() => send({ type: 'key', name: 'left' })} aria-label="Left"><ChevronLeft class="w-6 h-6 mx-auto" /></button>
          <button class="rb !bg-blue-600 hover:!bg-blue-500 font-semibold" on:click={() => send({ type: 'key', name: 'select' })}>OK</button>
          <button class="rb" on:click={() => send({ type: 'key', name: 'right' })} aria-label="Right"><ChevronRight class="w-6 h-6 mx-auto" /></button>
          <span></span><button class="rb" on:click={() => send({ type: 'key', name: 'down' })} aria-label="Down"><ChevronDown class="w-6 h-6 mx-auto" /></button><span></span>
        </div>
        <div class="grid grid-cols-3 gap-2">
          <button class="rb" on:click={() => send({ type: 'key', name: 'back' })} aria-label={$t('phone.back')}><ArrowLeft class="w-5 h-5 mx-auto" /></button>
          <button class="rb" on:click={() => send({ type: 'key', name: 'home' })} aria-label={$t('phone.home')}><Home class="w-5 h-5 mx-auto" /></button>
          <button class="rb" on:click={() => send({ type: 'key', name: 'menu' })} aria-label="Menu"><MenuIcon class="w-5 h-5 mx-auto" /></button>
        </div>
        <div class="grid grid-cols-4 gap-2">
          <button class="rb" on:click={() => send({ type: 'media', action: 'prev' })} aria-label="Previous"><SkipBack class="w-4 h-4 mx-auto" /></button>
          <button class="rb" on:click={() => send({ type: 'media', action: 'play_pause' })} aria-label="Play/Pause"><Play class="w-4 h-4 mx-auto" /></button>
          <button class="rb" on:click={() => send({ type: 'media', action: 'next' })} aria-label="Next"><SkipForward class="w-4 h-4 mx-auto" /></button>
          <button class="rb" on:click={() => send({ type: 'media', action: 'stop' })} aria-label="Stop"><Square class="w-4 h-4 mx-auto" /></button>
        </div>
        <div class="grid grid-cols-3 gap-2">
          <button class="rb" on:click={() => send({ type: 'volume', delta: -1 })} aria-label={$t('phone.vol_down')}><Volume1 class="w-5 h-5 mx-auto" /></button>
          <button class="rb" on:click={() => send({ type: 'mute' })} aria-label="Mute"><VolumeX class="w-5 h-5 mx-auto" /></button>
          <button class="rb" on:click={() => send({ type: 'volume', delta: 1 })} aria-label={$t('phone.vol_up')}><Volume2 class="w-5 h-5 mx-auto" /></button>
        </div>
        <form class="flex gap-2" on:submit|preventDefault={sendText}>
          <input bind:value={textValue} placeholder={$t('kde.text_or_link')} class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs" autocomplete="off" />
          <button class="px-3 py-1.5 rounded-lg text-xs bg-slate-700 hover:bg-slate-600">{$t('kde.send')}</button>
        </form>
      </div>
    {/if}
  </section>
</div>

<style>
  .rb { min-height: 48px; border-radius: 14px; background: rgb(30 41 59); }
  .rb:hover { background: rgb(51 65 85); }
  .rb:active { transform: scale(.96); }
</style>
