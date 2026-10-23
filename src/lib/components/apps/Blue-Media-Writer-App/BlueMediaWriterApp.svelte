<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { HardDriveDownload, Usb, RefreshCw, FolderOpen, Link2, Globe, CheckCircle2, AlertTriangle, Loader2, ShieldCheck, Eraser, X, ExternalLink } from 'lucide-svelte';
  import { t } from '../../../stores/language';
  import { SystemBridge, shellQuote } from '../../../utils/systemBridge';
  import { dialogConfirm } from '../../../stores/dialog';
  import {
    type DriveInfo, type CatalogEntry, type ImageInfo, type MwProgress,
    formatBytes, formatSpeed, formatEta, percent, driveTitle, validateImageUrl, isSha256, canStartWrite,
  } from '../../../utils/mediaWriter';

  interface Props {
    openPath?: string | undefined;
  }

  let { openPath = undefined }: Props = $props();

  type Tab = 'write' | 'restore';
  type Step = 'source' | 'drive' | 'writing' | 'done';
  type SourceKind = 'catalog' | 'file' | 'url';

  let tab = $state<Tab>('write');
  let step = $state<Step>('source');
  let sourceKind = $state<SourceKind>('catalog');
  const SOURCES: { id: SourceKind; label: string; icon: typeof Globe }[] = [
    { id: 'catalog', label: 'mw.source.catalog', icon: Globe },
    { id: 'file', label: 'mw.source.file', icon: FolderOpen },
    { id: 'url', label: 'mw.source.url', icon: Link2 },
  ];

  let catalog = $state<CatalogEntry[]>([]);
  let selectedEntry: CatalogEntry | null = null;
  let customUrl = $state('');
  let expectedSha = $state('');

  let image = $state<ImageInfo | null>(null);
  let drives = $state<DriveInfo[]>([]);
  let drive = $state<DriveInfo | null>(null);
  let drivesLoading = $state(false);
  let verify = $state(true);
  let confirmed = $state(false);

  let busy = $state(false);
  let phaseLabel = $state('');
  let progress = $state<MwProgress | null>(null);
  let error = $state('');
  let doneSha = $state('');
  let restoredOk = $state(false);

  let unlisten: UnlistenFn | undefined;
  let drivePoll: ReturnType<typeof setInterval> | undefined;

  let pct = $derived(progress ? percent(progress) : null);
  let eta = $derived(progress ? formatEta(progress) : '');
  let urlState = $derived(validateImageUrl(customUrl));
  let gate = $derived(canStartWrite({
    image: image?.path ?? null,
    drive,
    imageBytes: image ? (image.uncompressedBytes ?? (image.compression === 'none' ? image.sizeBytes : null)) : null,
    confirmed,
  }));

  const phaseKey: Record<string, string> = {
    download: 'mw.phase.download', hash: 'mw.phase.hash', write: 'mw.phase.write',
    flush: 'mw.phase.flush', verify: 'mw.phase.verify', restore: 'mw.phase.restore', done: 'mw.phase.done',
  };

  onMount(async () => {
    unlisten = await listen<MwProgress>('blue-media-writer://progress', (e) => {
      progress = e.payload;
      phaseLabel = $t(phaseKey[e.payload.phase] ?? 'mw.phase.write');
    });
    try { catalog = await SystemBridge.invokeCommand<CatalogEntry[]>('mw_catalog', {}); } catch { catalog = []; }
    if (openPath) await useLocalFile(openPath);
    // Notice sticks being plugged in / removed while the app is open.
    drivePoll = setInterval(() => { if (!busy && (step === 'drive' || tab === 'restore')) void refreshDrives(true); }, 3000);
  });

  onDestroy(() => { unlisten?.(); if (drivePoll) clearInterval(drivePoll); });

  async function refreshDrives(quiet = false) {
    if (!quiet) drivesLoading = true;
    try {
      drives = await SystemBridge.invokeCommand<DriveInfo[]>('mw_list_drives', {});
      if (drive && !drives.some((d) => d.path === drive!.path)) { drive = null; confirmed = false; }
    } catch (e: any) {
      if (!quiet) error = String(e?.message ?? e);
    } finally {
      drivesLoading = false;
    }
  }

  async function useLocalFile(path: string) {
    error = '';
    try {
      const info = await SystemBridge.invokeCommand<ImageInfo>('mw_image_info', { path });
      if (!info.supported) { error = $t('mw.err.unsupported'); return; }
      image = info;
      sourceKind = 'file';
      await goToDrive();
    } catch (e: any) { error = String(e?.message ?? e); }
  }

  async function pickLocal() {
    const p = await SystemBridge.pickFile(
      [{ name: 'Disk images', extensions: ['iso', 'img', 'raw', 'xz', 'gz', 'zst', 'bz2'] }],
      $t('mw.pick_image'), 'blue-media-writer.image');
    if (p) await useLocalFile(p);
  }

  function openPage(url: string) {
    void SystemBridge.executeCommand(`xdg-open ${shellQuote(url)} >/dev/null 2>&1 &`).catch(() => {});
  }

  async function chooseEntry(e: CatalogEntry) {
    selectedEntry = e;
    if (!e.url) { if (e.pageUrl) openPage(e.pageUrl); return; }
    await downloadAndContinue(e.url, e.checksumUrl ?? null);
  }

  async function downloadAndContinue(url: string, checksumUrl: string | null) {
    error = '';
    busy = true; progress = null; step = 'writing'; phaseLabel = $t('mw.phase.download');
    try {
      const path = await SystemBridge.invokeCommand<string>('mw_download', { url });
      const name = path.split('/').pop() ?? '';
      let expected = isSha256(expectedSha) ? expectedSha : '';
      if (!expected && checksumUrl) {
        try { expected = (await SystemBridge.invokeCommand<string | null>('mw_fetch_checksum', { url: checksumUrl, filename: name })) ?? ''; } catch { /* checksum file unreachable: continue unverified */ }
      }
      if (expected) {
        phaseLabel = $t('mw.phase.hash');
        const actual = await SystemBridge.invokeCommand<string>('mw_hash', { path });
        if (actual.toLowerCase() !== expected.trim().split(/\s+/)[0].replace(/^sha256:/i, '').toLowerCase()) {
          throw new Error($t('mw.err.checksum'));
        }
      }
      image = await SystemBridge.invokeCommand<ImageInfo>('mw_image_info', { path });
      if (!image.supported) throw new Error($t('mw.err.unsupported'));
      busy = false;
      await goToDrive();
    } catch (e: any) {
      busy = false; step = 'source';
      error = String(e?.message ?? e) === 'Cancelled' ? '' : String(e?.message ?? e);
    }
  }

  async function startCustomUrl() {
    if (urlState !== 'ok') return;
    await downloadAndContinue(customUrl.trim(), null);
  }

  async function goToDrive() {
    step = 'drive'; drive = null; confirmed = false;
    await refreshDrives();
  }

  async function write() {
    if (!gate.ok || !image || !drive) return;
    const ok = await dialogConfirm({
      title: $t('mw.confirm.title'),
      message: $t('mw.confirm.body').replace('{drive}', `${driveTitle(drive)} (${drive.path}, ${formatBytes(drive.sizeBytes)})`),
      confirmLabel: $t('mw.confirm.write'), cancelLabel: $t('common.cancel'), danger: true,
    });
    if (!ok) return;
    error = ''; busy = true; progress = null; step = 'writing'; phaseLabel = $t('mw.phase.write');
    try {
      doneSha = await SystemBridge.invokeCommand<string>('mw_write', { image: image.path, device: drive.path, verify });
      step = 'done';
    } catch (e: any) {
      step = 'drive';
      error = String(e?.message ?? e) === 'Cancelled' ? $t('mw.err.cancelled') : String(e?.message ?? e);
    } finally { busy = false; }
  }

  async function restore() {
    if (!drive) return;
    const ok = await dialogConfirm({
      title: $t('mw.restore.confirm_title'),
      message: $t('mw.restore.confirm_body').replace('{drive}', `${driveTitle(drive)} (${drive.path}, ${formatBytes(drive.sizeBytes)})`),
      confirmLabel: $t('mw.restore.confirm'), cancelLabel: $t('common.cancel'), danger: true,
    });
    if (!ok) return;
    error = ''; busy = true; progress = null; restoredOk = false; phaseLabel = $t('mw.phase.restore');
    try { await SystemBridge.invokeCommand('mw_restore', { device: drive.path }); restoredOk = true; }
    catch (e: any) { error = String(e?.message ?? e); }
    finally { busy = false; await refreshDrives(true); }
  }

  async function cancel() { await SystemBridge.invokeCommand('mw_cancel', {}).catch(() => {}); }

  function again() {
    step = 'source'; image = null; drive = null; confirmed = false; error = ''; selectedEntry = null; customUrl = ''; expectedSha = ''; progress = null;
  }

  function switchTab(next: Tab) {
    if (busy) return;
    tab = next; error = ''; restoredOk = false; drive = null; confirmed = false;
    if (next === 'restore') void refreshDrives();
  }
</script>

<div class="flex flex-col h-full bg-slate-900 text-white">
  <div class="flex items-center gap-3 px-4 py-2.5 border-b border-white/5 shrink-0 bg-slate-800/50">
    <HardDriveDownload size={18} class="text-blue-400" />
    <span class="font-semibold text-sm">{$t('mw.title')}</span>
    <div class="flex-1"></div>
    <div class="flex bg-slate-800 rounded-lg p-0.5 text-xs" role="tablist">
      <button role="tab" aria-selected={tab === 'write'} onclick={() => switchTab('write')} disabled={busy}
        class="px-3 py-1 rounded-md transition-colors {tab === 'write' ? 'bg-blue-600' : 'text-slate-400 hover:text-white'}">{$t('mw.tab.write')}</button>
      <button role="tab" aria-selected={tab === 'restore'} onclick={() => switchTab('restore')} disabled={busy}
        class="px-3 py-1 rounded-md transition-colors {tab === 'restore' ? 'bg-blue-600' : 'text-slate-400 hover:text-white'}">{$t('mw.tab.restore')}</button>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto p-5">
    {#if error}
      <div class="mb-4 flex items-start gap-2 bg-red-500/10 border border-red-500/30 text-red-200 rounded-lg px-3 py-2 text-sm" role="alert">
        <AlertTriangle size={16} class="mt-0.5 shrink-0" /> <span class="min-w-0 break-words">{error}</span>
      </div>
    {/if}

    {#if tab === 'write'}
      {#if step === 'source'}
        <div class="flex gap-2 mb-4 text-xs">
          {#each SOURCES as src (src.id)}
            <button onclick={() => (sourceKind = src.id)}
              class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border {sourceKind === src.id ? 'border-blue-500 bg-blue-500/15' : 'border-white/10 hover:bg-white/5'}">
              <src.icon size={13} /> {$t(src.label)}
            </button>
          {/each}
        </div>

        {#if sourceKind === 'catalog'}
          <p class="text-xs text-slate-400 mb-3">{$t('mw.catalog.hint')}</p>
          <div class="grid gap-2">
            {#each catalog as e (e.id)}
              <button onclick={() => chooseEntry(e)} class="text-left p-3 rounded-xl bg-slate-800 hover:bg-slate-700 transition-colors flex items-center gap-3">
                <div class="min-w-0 flex-1">
                  <div class="font-medium text-sm">{e.name} <span class="text-[10px] text-slate-500 ml-1">{e.arch}</span></div>
                  <div class="text-xs text-slate-400 truncate">{e.description}</div>
                </div>
                {#if e.url}
                  <span class="text-xs text-blue-300 shrink-0">{$t('mw.catalog.download')}</span>
                {:else}
                  <span class="text-xs text-slate-300 shrink-0 flex items-center gap-1"><ExternalLink size={12} /> {$t('mw.catalog.open_page')}</span>
                {/if}
              </button>
            {:else}
              <div class="text-sm text-slate-500">{$t('mw.catalog.empty')}</div>
            {/each}
          </div>
          <p class="text-[11px] text-slate-500 mt-3">{$t('mw.catalog.page_note')}</p>
        {:else if sourceKind === 'file'}
          <p class="text-xs text-slate-400 mb-3">{$t('mw.file.hint')}</p>
          <button onclick={pickLocal} class="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm"><FolderOpen size={15} /> {$t('mw.file.choose')}</button>
        {:else}
          <label class="block text-xs text-slate-400 mb-1" for="mw-url">{$t('mw.url.label')}</label>
          <input id="mw-url" bind:value={customUrl} placeholder="https://…/image.iso" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm outline-none focus:border-blue-500" />
          {#if customUrl && urlState !== 'ok'}
            <div class="text-xs text-amber-300 mt-1">{urlState === 'not_https' ? $t('mw.url.not_https') : $t('mw.url.invalid')}</div>
          {/if}
          <label class="block text-xs text-slate-400 mt-3 mb-1" for="mw-sha">{$t('mw.url.sha_label')}</label>
          <input id="mw-sha" bind:value={expectedSha} placeholder="SHA-256" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono outline-none focus:border-blue-500" />
          {#if expectedSha && !isSha256(expectedSha)}<div class="text-xs text-amber-300 mt-1">{$t('mw.url.sha_invalid')}</div>{/if}
          <button onclick={startCustomUrl} disabled={urlState !== 'ok'} class="mt-4 flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm disabled:opacity-40">{$t('mw.catalog.download')}</button>
        {/if}

      {:else if step === 'drive'}
        <div class="mb-4 p-3 rounded-xl bg-slate-800 text-sm flex items-center gap-3">
          <HardDriveDownload size={16} class="text-blue-400 shrink-0" />
          <div class="min-w-0 flex-1">
            <div class="truncate">{image?.name}</div>
            <div class="text-xs text-slate-400">{image ? formatBytes(image.uncompressedBytes ?? image.sizeBytes) : ''}{image && image.compression !== 'none' ? ` · ${image.compression}` : ''}</div>
          </div>
          <button onclick={again} class="text-xs text-slate-400 hover:text-white">{$t('mw.change')}</button>
        </div>

        <div class="flex items-center justify-between mb-2">
          <h3 class="text-sm font-semibold">{$t('mw.drive.title')}</h3>
          <button onclick={() => refreshDrives()} class="text-xs text-slate-400 hover:text-white flex items-center gap-1"><RefreshCw size={12} class={drivesLoading ? 'animate-spin' : ''} /> {$t('mw.drive.refresh')}</button>
        </div>
        {#if drives.length === 0}
          <div class="p-4 rounded-xl border border-dashed border-white/10 text-sm text-slate-400 flex items-center gap-2"><Usb size={16} /> {$t('mw.drive.none')}</div>
        {:else}
          <div class="grid gap-2" role="radiogroup" aria-label={$t('mw.drive.title')}>
            {#each drives as d (d.path)}
              <button role="radio" aria-checked={drive?.path === d.path} onclick={() => { drive = d; confirmed = false; }}
                class="text-left p-3 rounded-xl flex items-center gap-3 border {drive?.path === d.path ? 'border-blue-500 bg-blue-500/10' : 'border-transparent bg-slate-800 hover:bg-slate-700'}">
                <Usb size={18} class="text-slate-300 shrink-0" />
                <div class="min-w-0 flex-1">
                  <div class="text-sm font-medium truncate">{driveTitle(d)}</div>
                  <div class="text-xs text-slate-400 truncate">{d.path} · {formatBytes(d.sizeBytes)}{d.labels.length ? ` · ${d.labels.join(', ')}` : ''}</div>
                </div>
                {#if image && (image.uncompressedBytes ?? (image.compression === 'none' ? image.sizeBytes : 0)) > d.sizeBytes}
                  <span class="text-xs text-red-300">{$t('mw.drive.too_small')}</span>
                {/if}
              </button>
            {/each}
          </div>
        {/if}

        {#if drive}
          <label class="mt-4 flex items-center gap-2 text-sm"><input type="checkbox" bind:checked={verify} /> <ShieldCheck size={14} class="text-green-400" /> {$t('mw.verify')}</label>
          <div class="mt-3 p-3 rounded-xl bg-amber-500/10 border border-amber-500/30 text-xs text-amber-100">
            {$t('mw.warn.erase').replace('{drive}', drive.path)}
            <label class="mt-2 flex items-center gap-2"><input type="checkbox" bind:checked={confirmed} /> {$t('mw.warn.understand')}</label>
          </div>
          <button onclick={write} disabled={!gate.ok} class="mt-4 flex items-center gap-2 px-4 py-2 bg-red-600 hover:bg-red-500 rounded-lg text-sm disabled:opacity-40">
            <HardDriveDownload size={15} /> {$t('mw.write')}
          </button>
          {#if gate.reason === 'too_big'}<div class="text-xs text-red-300 mt-2">{$t('mw.drive.too_small')}</div>{/if}
        {/if}

      {:else if step === 'writing'}
        <div class="max-w-md mx-auto mt-8 text-center">
          <Loader2 size={28} class="animate-spin text-blue-400 mx-auto mb-3" />
          <div class="text-sm font-medium mb-3">{phaseLabel}</div>
          <div class="h-2 bg-slate-800 rounded-full overflow-hidden" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct ?? undefined}>
            {#if pct === null}<div class="h-full w-1/3 bg-blue-500 rounded-full animate-pulse"></div>
            {:else}<div class="h-full bg-blue-500 transition-all" style="width:{pct}%"></div>{/if}
          </div>
          <div class="mt-2 text-xs text-slate-400 flex justify-center gap-3">
            {#if progress}<span>{formatBytes(progress.done)}{progress.total ? ` / ${formatBytes(progress.total)}` : ''}</span>{/if}
            {#if progress?.speed}<span>{formatSpeed(progress.speed)}</span>{/if}
            {#if eta}<span>{$t('mw.eta').replace('{t}', eta)}</span>{/if}
          </div>
          <button onclick={cancel} class="mt-5 flex items-center gap-1.5 mx-auto px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs"><X size={12} /> {$t('common.cancel')}</button>
          {#if progress?.phase === 'write' || progress?.phase === 'flush'}<p class="text-[11px] text-slate-500 mt-3">{$t('mw.dont_unplug')}</p>{/if}
        </div>

      {:else}
        <div class="max-w-md mx-auto mt-8 text-center">
          <CheckCircle2 size={44} class="text-green-400 mx-auto mb-3" />
          <div class="text-lg font-semibold">{$t('mw.done.title')}</div>
          <p class="text-sm text-slate-400 mt-1">{$t('mw.done.body')}</p>
          {#if doneSha}<div class="mt-3 text-[10px] font-mono text-slate-500 break-all">SHA-256 {doneSha}</div>{/if}
          <button onclick={again} class="mt-5 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm">{$t('mw.done.another')}</button>
        </div>
      {/if}

    {:else}
      <p class="text-xs text-slate-400 mb-3">{$t('mw.restore.hint')}</p>
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-semibold">{$t('mw.drive.title')}</h3>
        <button onclick={() => refreshDrives()} class="text-xs text-slate-400 hover:text-white flex items-center gap-1"><RefreshCw size={12} class={drivesLoading ? 'animate-spin' : ''} /> {$t('mw.drive.refresh')}</button>
      </div>
      {#if drives.length === 0}
        <div class="p-4 rounded-xl border border-dashed border-white/10 text-sm text-slate-400 flex items-center gap-2"><Usb size={16} /> {$t('mw.drive.none')}</div>
      {:else}
        <div class="grid gap-2" role="radiogroup" aria-label={$t('mw.drive.title')}>
          {#each drives as d (d.path)}
            <button role="radio" aria-checked={drive?.path === d.path} onclick={() => (drive = d)}
              class="text-left p-3 rounded-xl flex items-center gap-3 border {drive?.path === d.path ? 'border-blue-500 bg-blue-500/10' : 'border-transparent bg-slate-800 hover:bg-slate-700'}">
              <Usb size={18} class="text-slate-300 shrink-0" />
              <div class="min-w-0 flex-1">
                <div class="text-sm font-medium truncate">{driveTitle(d)}</div>
                <div class="text-xs text-slate-400 truncate">{d.path} · {formatBytes(d.sizeBytes)}</div>
              </div>
            </button>
          {/each}
        </div>
      {/if}
      {#if busy}
        <div class="mt-4 flex items-center gap-2 text-sm text-slate-300"><Loader2 size={14} class="animate-spin" /> {phaseLabel}</div>
      {/if}
      {#if restoredOk}
        <div class="mt-4 flex items-center gap-2 text-sm text-green-300"><CheckCircle2 size={16} /> {$t('mw.restore.done')}</div>
      {/if}
      <button onclick={restore} disabled={!drive || busy} class="mt-4 flex items-center gap-2 px-4 py-2 bg-red-600 hover:bg-red-500 rounded-lg text-sm disabled:opacity-40"><Eraser size={15} /> {$t('mw.restore.button')}</button>
    {/if}
  </div>
</div>
