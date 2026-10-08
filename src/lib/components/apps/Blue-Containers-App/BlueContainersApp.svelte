<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Play, Square, RotateCw, Trash2, Terminal, Download, Plus, RefreshCw, FileText, Box } from 'lucide-svelte';

  // Backend: src-tauri/src/BlueContainers (bollard + distrobox).
  interface Status { engine: string | null; connected: boolean; distrobox: boolean; error: string | null }
  interface Ctr { id: string; name: string; image: string; state: string; status: string; created: number; distrobox: boolean }
  interface Img { id: string; tags: string[]; size: number; created: number }

  let tab: 'containers' | 'images' = 'containers';
  let status: Status | null = null;
  let containers: Ctr[] = [];
  let images: Img[] = [];
  let busy = '';
  let error = '';
  let logs = '';
  let logsFor = '';
  let timer: ReturnType<typeof setInterval>;

  // dialog "Nowa skrzynka"
  let showCreate = false;
  let newName = '';
  let newImage = 'docker.io/library/ubuntu:24.04';
  let nvidia = false;
  let init = false;
  const SUGGESTED = [
    'docker.io/library/ubuntu:24.04', 'docker.io/library/debian:stable', 'docker.io/library/archlinux:latest',
    'registry.fedoraproject.org/fedora-toolbox:41', 'quay.io/toolbx/arch-toolbox:latest',
    'docker.io/library/alpine:latest', 'docker.io/opensuse/tumbleweed:latest',
  ];
  let pullImage = '';

  async function run<T>(label: string, fn: () => Promise<T>): Promise<T | undefined> {
    busy = label; error = '';
    try { return await fn(); } catch (e) { error = String(e); } finally { busy = ''; }
  }

  async function refresh() {
    try {
      status = await invoke<Status>('containers_status');
      if (!status.connected) return;
      containers = await invoke<Ctr[]>('containers_list');
      if (tab === 'images') images = await invoke<Img[]>('containers_images');
    } catch (e) { error = String(e); }
  }

  const act = (c: Ctr, action: string) => run(`${action} ${c.name}`, async () => { await invoke('containers_action', { id: c.id, action }); await refresh(); });
  async function showLogs(c: Ctr) {
    logsFor = c.name;
    logs = (await run('logs', () => invoke<string>('containers_logs', { id: c.id, tail: 300 }))) ?? '';
  }
  const enter = (c: Ctr) => run('enter', () => invoke('distrobox_enter', { name: c.name }));
  async function createBox() {
    await run('create', async () => {
      await invoke('distrobox_create', { name: newName.trim(), image: newImage.trim(), nvidia, init });
      showCreate = false; newName = '';
      await refresh();
    });
  }
  const pull = () => run(`pull ${pullImage}`, async () => { await invoke('containers_pull_image', { image: pullImage.trim() }); pullImage = ''; images = await invoke<Img[]>('containers_images'); });
  const rmImage = (i: Img) => run('rmi', async () => { await invoke('containers_remove_image', { id: i.id }); images = await invoke<Img[]>('containers_images'); });
  const fmtSize = (b: number) => (b > 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);
  const stateColor = (s: string) => (s === 'running' ? 'bg-green-500' : s === 'paused' ? 'bg-yellow-500' : 'bg-slate-500');

  onMount(() => { refresh(); timer = setInterval(refresh, 5000); });
  onDestroy(() => clearInterval(timer));
  $: if (tab === 'images' && status?.connected) invoke<Img[]>('containers_images').then((i) => (images = i)).catch(() => {});
</script>

<div class="h-full flex flex-col bg-slate-900 text-slate-200 text-sm">
  <div class="flex items-center gap-2 px-3 py-2 border-b border-white/5">
    <Box size={18} class="text-blue-400" />
    <div class="flex gap-1">
      <button class="px-3 py-1 rounded-lg {tab === 'containers' ? 'bg-blue-600 text-white' : 'hover:bg-white/5'}" on:click={() => (tab = 'containers')}>Kontenery</button>
      <button class="px-3 py-1 rounded-lg {tab === 'images' ? 'bg-blue-600 text-white' : 'hover:bg-white/5'}" on:click={() => (tab = 'images')}>Obrazy</button>
    </div>
    <div class="flex-1" />
    {#if status?.engine}<span class="text-xs text-slate-500">{status.engine}</span>{/if}
    <button class="p-1.5 rounded hover:bg-white/10" title="Odśwież" on:click={refresh}><RefreshCw size={15} /></button>
    {#if status?.distrobox}
      <button class="flex items-center gap-1 px-3 py-1 rounded-lg bg-blue-600 hover:bg-blue-500 text-white" on:click={() => (showCreate = true)}><Plus size={14} /> Nowa skrzynka</button>
    {/if}
  </div>

  {#if error}<div class="px-3 py-2 text-xs bg-red-500/10 text-red-300">{error}</div>{/if}
  {#if busy}<div class="px-3 py-1 text-xs text-blue-300">Trwa: {busy}…</div>{/if}

  {#if status && !status.connected}
    <div class="flex-1 flex flex-col items-center justify-center gap-2 p-6 text-center text-slate-400">
      <Box size={36} class="opacity-40" />
      <p>Nie wykryto silnika kontenerów.</p>
      <p class="text-xs">{status.error}</p>
      <code class="text-xs bg-slate-800 rounded px-2 py-1">systemctl --user enable --now podman.socket</code>
      {#if !status.distrobox}<p class="text-xs">Do tworzenia skrzynek zainstaluj też <b>distrobox</b>.</p>{/if}
    </div>
  {:else if tab === 'containers'}
    <div class="flex-1 overflow-y-auto p-3 space-y-2">
      {#each containers as c (c.id)}
        <div class="bg-slate-800 rounded-xl p-3 border border-white/5">
          <div class="flex items-center gap-3">
            <span class="w-2.5 h-2.5 rounded-full {stateColor(c.state)}" />
            <div class="flex-1 min-w-0">
              <div class="font-medium text-white truncate">{c.name}{#if c.distrobox}<span class="ml-2 text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-300">distrobox</span>{/if}</div>
              <div class="text-xs text-slate-500 truncate">{c.image} · {c.status}</div>
            </div>
            {#if c.distrobox}<button class="p-1.5 rounded hover:bg-white/10" title="Otwórz terminal" on:click={() => enter(c)}><Terminal size={15} /></button>{/if}
            {#if c.state === 'running'}
              <button class="p-1.5 rounded hover:bg-white/10" title="Zatrzymaj" on:click={() => act(c, 'stop')}><Square size={15} /></button>
              <button class="p-1.5 rounded hover:bg-white/10" title="Restart" on:click={() => act(c, 'restart')}><RotateCw size={15} /></button>
            {:else}
              <button class="p-1.5 rounded hover:bg-white/10" title="Uruchom" on:click={() => act(c, 'start')}><Play size={15} /></button>
            {/if}
            <button class="p-1.5 rounded hover:bg-white/10" title="Logi" on:click={() => showLogs(c)}><FileText size={15} /></button>
            <button class="p-1.5 rounded hover:bg-red-500/20 text-red-400" title="Usuń" on:click={() => confirm(`Usunąć ${c.name}?`) && act(c, 'remove')}><Trash2 size={15} /></button>
          </div>
        </div>
      {:else}
        <div class="text-center text-slate-500 py-10">Brak kontenerów. Utwórz pierwszą skrzynkę przyciskiem „Nowa skrzynka".</div>
      {/each}
      {#if logsFor}
        <div class="bg-black/40 rounded-xl p-3 border border-white/5">
          <div class="flex justify-between text-xs text-slate-400 mb-1"><span>Logi: {logsFor}</span><button on:click={() => (logsFor = '')}>Zamknij</button></div>
          <pre class="text-[11px] text-slate-300 max-h-60 overflow-auto whitespace-pre-wrap">{logs || '(puste)'}</pre>
        </div>
      {/if}
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto p-3 space-y-2">
      <div class="flex gap-2">
        <input class="flex-1 bg-slate-800 rounded-lg px-3 py-1.5 outline-none" list="suggested-images" placeholder="docker.io/library/ubuntu:24.04" bind:value={pullImage} />
        <datalist id="suggested-images">{#each SUGGESTED as s}<option value={s} />{/each}</datalist>
        <button class="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-40" disabled={!pullImage.trim() || !!busy} on:click={pull}><Download size={14} /> Pobierz</button>
      </div>
      {#each images as i (i.id)}
        <div class="bg-slate-800 rounded-xl p-3 border border-white/5 flex items-center gap-3">
          <div class="flex-1 min-w-0"><div class="text-white truncate">{i.tags[0] ?? i.id.slice(7, 19)}</div><div class="text-xs text-slate-500">{fmtSize(i.size)}</div></div>
          <button class="p-1.5 rounded hover:bg-red-500/20 text-red-400" on:click={() => confirm('Usunąć obraz?') && rmImage(i)}><Trash2 size={15} /></button>
        </div>
      {/each}
    </div>
  {/if}

  {#if showCreate}
    <div class="absolute inset-0 bg-black/60 flex items-center justify-center z-10">
      <div class="bg-slate-800 rounded-2xl p-5 w-96 space-y-3 border border-white/10">
        <div class="text-white font-semibold">Nowa skrzynka (distrobox)</div>
        <input class="w-full bg-slate-700 rounded-lg px-3 py-1.5 outline-none" placeholder="Nazwa" bind:value={newName} />
        <input class="w-full bg-slate-700 rounded-lg px-3 py-1.5 outline-none" list="suggested-images" bind:value={newImage} />
        <label class="flex items-center gap-2"><input type="checkbox" bind:checked={nvidia} /> Integracja NVIDIA</label>
        <label class="flex items-center gap-2"><input type="checkbox" bind:checked={init} /> Z systemd (init)</label>
        <div class="flex justify-end gap-2">
          <button class="px-3 py-1.5 rounded-lg hover:bg-white/10" on:click={() => (showCreate = false)}>Anuluj</button>
          <button class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-40" disabled={!newName.trim() || !!busy} on:click={createBox}>Utwórz</button>
        </div>
      </div>
    </div>
  {/if}
</div>
