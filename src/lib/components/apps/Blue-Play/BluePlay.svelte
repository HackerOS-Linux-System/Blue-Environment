<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    Gamepad2, ArrowLeft, Trophy, PlayCircle, RotateCcw, Plus, X, HardDrive,
    Monitor, Loader2, CheckCircle2, XCircle, Clock, Trash2, FolderOpen,
  } from 'lucide-svelte';
  import { configStore } from '../../../utils/configStore';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { notificationManager } from '../../../utils/notificationManager';
  import { GAMES, type GameDef } from './gameRegistry';
  import type { BlueGameLibraryEntry } from '../../../types';

  // Scope note (still honest about what this is, expanded from last
  // session): Blue Play is a library + launcher for both Blue
  // Environment's own original games AND user-added native Linux /
  // Windows (Wine, Proton, or umu-run) games, with persisted stats. The
  // "runs in the background" part is real, not just a claim: once a
  // game is launched, a backend thread tracks it independently of
  // whether this window stays open, focused, or gets closed entirely —
  // playtime keeps accumulating and the exit event still lands whenever
  // Blue Play is next open. It is NOT a persistent system service that
  // survives a full logout/reboot and re-attaches to already-running
  // games — that would need a proper always-on daemon (like BEDM), left
  // as a real follow-up rather than pretended-away.

  type Tab = 'games' | 'library';
  let tab: Tab = 'games';

  let activeGame: GameDef | null = null;
  let stats: Record<string, { highScore: number; playCount: number; lastPlayed?: string; playtimeSeconds?: number }> = {};
  let library: BlueGameLibraryEntry[] = [];
  let liveScore = 0;
  let unlistenExit: (() => void) | null = null;

  // Runtime (Wine/Proton/umu) detection
  let runtimes: Awaited<ReturnType<typeof SystemBridge.bluePlayDetectRuntimes>> | null = null;
  let runtimesLoading = false;

  // Add-game dialog
  let showAddDialog = false;
  let addKind: 'native' | 'windows' = 'native';
  let addTitle = '';
  let addPath = '';
  let addRuntime: 'wine' | 'proton' | 'umu' = 'wine';
  let addProtonPath = '';
  let launchingId: string | null = null;
  let launchError: string | null = null;

  onMount(async () => {
    const cfg = await configStore.init();
    stats = cfg.blueGames ?? {};
    library = cfg.blueGamesLibrary ?? [];
    unlistenExit = await SystemBridge.bluePlayOnGameExited((payload) => {
      const s = stats[payload.game_id] ?? { highScore: 0, playCount: 0, playtimeSeconds: 0 };
      stats = {
        ...stats,
        [payload.game_id]: { ...s, playtimeSeconds: (s.playtimeSeconds ?? 0) + payload.playtime_seconds },
      };
      persistStats();
      const entry = library.find((g) => g.id === payload.game_id);
      if (entry) {
        notificationManager.add({
          title: entry.title,
          message: `Session: ${formatPlaytime(payload.playtime_seconds)}${payload.exit_success ? '' : ' — exited with an error'}`,
        });
      }
      if (launchingId === payload.game_id) launchingId = null;
    });
    loadRuntimes();
  });
  onDestroy(() => unlistenExit?.());

  async function loadRuntimes() {
    runtimesLoading = true;
    runtimes = await SystemBridge.bluePlayDetectRuntimes();
    runtimesLoading = false;
    if (runtimes.umu_available) addRuntime = 'umu';
    else if (runtimes.steam_proton_versions.length > 0 || runtimes.proton_available) addRuntime = 'proton';
    else addRuntime = 'wine';
    if (runtimes.steam_proton_versions.length > 0) addProtonPath = runtimes.steam_proton_versions[0].path;
    else if (runtimes.proton_path) addProtonPath = runtimes.proton_path;
  }

  function formatPlaytime(seconds: number): string {
    if (seconds < 60) return `${seconds}s`;
    const mins = Math.round(seconds / 60);
    if (mins < 60) return `${mins} min`;
    return `${(mins / 60).toFixed(1)} hrs`;
  }

  async function persistStats() {
    await configStore.save({ blueGames: stats });
  }
  async function persistLibrary() {
    await configStore.save({ blueGamesLibrary: library });
  }

  // ── Built-in games ───────────────────────────────────────────────────
  function playGame(game: GameDef) {
    activeGame = game;
    liveScore = 0;
    const s = stats[game.id] ?? { highScore: 0, playCount: 0 };
    stats = { ...stats, [game.id]: { ...s, playCount: s.playCount + 1, lastPlayed: new Date().toISOString() } };
    persistStats();
  }

  function handleGameEvent(e: Event) {
    if (!activeGame) return;
    onScore(activeGame.id, (e as CustomEvent<number>).detail);
  }

  function onScore(gameId: string, score: number) {
    liveScore = score;
    const s = stats[gameId] ?? { highScore: 0, playCount: 0 };
    if (score > s.highScore) {
      stats = { ...stats, [gameId]: { ...s, highScore: score } };
      persistStats();
    }
  }

  function highScore(id: string): number { return stats[id]?.highScore ?? 0; }
  function playCount(id: string): number { return stats[id]?.playCount ?? 0; }
  function playtime(id: string): number { return stats[id]?.playtimeSeconds ?? 0; }

  function backToLibrary() {
    activeGame = null;
  }

  // ── External (native / Windows) library ─────────────────────────────
  async function pickExecutable() {
    const filters = addKind === 'windows'
      ? [{ name: 'Windows executable', extensions: ['exe'] }]
      : [];
    const path = await SystemBridge.pickFile(filters, addKind === 'windows' ? 'Select a .exe' : 'Select a game binary');
    if (path) {
      addPath = path;
      if (!addTitle) addTitle = path.split('/').pop()?.replace(/\.exe$/i, '') ?? '';
    }
  }

  function openAddDialog(kind: 'native' | 'windows') {
    addKind = kind;
    addTitle = '';
    addPath = '';
    showAddDialog = true;
  }

  async function confirmAddGame() {
    if (!addTitle.trim() || !addPath.trim()) return;
    const entry: BlueGameLibraryEntry = {
      id: `ext-${Date.now()}`,
      title: addTitle.trim(),
      kind: addKind,
      execPath: addPath.trim(),
      addedAt: new Date().toISOString(),
      ...(addKind === 'windows' ? { runtime: addRuntime, runtimePath: addRuntime === 'proton' ? addProtonPath : undefined } : {}),
    };
    library = [...library, entry];
    await persistLibrary();
    showAddDialog = false;
  }

  async function removeGame(entry: BlueGameLibraryEntry) {
    library = library.filter((g) => g.id !== entry.id);
    await persistLibrary();
  }

  async function launchExternal(entry: BlueGameLibraryEntry) {
    launchError = null;
    launchingId = entry.id;
    const s = stats[entry.id] ?? { highScore: 0, playCount: 0 };
    stats = { ...stats, [entry.id]: { ...s, playCount: s.playCount + 1, lastPlayed: new Date().toISOString() } };
    persistStats();

    const result = entry.kind === 'native'
      ? await SystemBridge.bluePlayLaunchNative(entry.id, entry.execPath)
      : await SystemBridge.bluePlayLaunchWindows(entry.id, entry.execPath, entry.runtime ?? 'wine', entry.runtimePath);

    if (!result.launched) {
      launchError = result.error ?? 'Failed to launch';
      launchingId = null;
    }
    // On success, launchingId clears when the game-exited event arrives
    // (see onMount) — that's how we know it actually finished running,
    // not just that the spawn call returned.
  }

  $: canAddWindows = runtimes ? (runtimes.wine_available || runtimes.umu_available || runtimes.proton_available || runtimes.steam_proton_versions.length > 0) : true;
</script>

<div class="h-full flex flex-col bg-slate-900 text-white">
  <div class="shrink-0 border-b border-white/5 px-4 py-3 flex items-center gap-3">
    {#if activeGame}
      <button on:click={backToLibrary} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-300"><ArrowLeft size={16} /></button>
      <Gamepad2 size={16} class="text-purple-400" />
      <span class="font-semibold text-sm">{activeGame.title}</span>
      <span class="ml-auto flex items-center gap-3 text-xs text-slate-400">
        <span>Score: <strong class="text-white">{liveScore}</strong></span>
        <span class="flex items-center gap-1"><Trophy size={12} class="text-amber-400" /> Best: {highScore(activeGame.id)}</span>
      </span>
    {:else}
      <Gamepad2 size={16} class="text-purple-400" />
      <span class="font-semibold text-sm">Blue Play</span>
      <div class="ml-4 flex bg-slate-800 rounded-lg p-0.5">
        <button on:click={() => (tab = 'games')} class="px-3 py-1 rounded-md text-xs font-medium {tab === 'games' ? 'bg-blue-600 text-white' : 'text-slate-400'}">Games</button>
        <button on:click={() => (tab = 'library')} class="px-3 py-1 rounded-md text-xs font-medium {tab === 'library' ? 'bg-blue-600 text-white' : 'text-slate-400'}">Library</button>
      </div>
      <span class="ml-auto text-xs text-slate-500">{tab === 'games' ? `${GAMES.length} games` : `${library.length} added`}</span>
    {/if}
  </div>

  {#if activeGame}
    <div class="flex-1 overflow-auto flex items-center justify-center p-6">
      <svelte:component this={activeGame.component} on:score={handleGameEvent} on:gameOver={handleGameEvent} />
    </div>

  {:else if tab === 'games'}
    <div class="flex-1 overflow-y-auto p-5">
      <div class="grid grid-cols-2 md:grid-cols-3 gap-4">
        {#each GAMES as game (game.id)}
          <button on:click={() => playGame(game)}
            class="text-left rounded-2xl border border-white/10 overflow-hidden hover:border-white/20 hover:-translate-y-0.5 transition-all group">
            <div class="h-24 bg-gradient-to-br {game.color} flex items-center justify-center">
              <Gamepad2 size={32} class="text-white/70 group-hover:scale-110 transition-transform" />
            </div>
            <div class="p-3 bg-slate-800/60">
              <div class="font-semibold text-sm">{game.title}</div>
              <p class="text-xs text-slate-400 mt-0.5 line-clamp-2">{game.tagline}</p>
              <div class="flex items-center justify-between mt-2.5 text-xs">
                <span class="flex items-center gap-1 text-amber-400"><Trophy size={11} /> {highScore(game.id)}</span>
                {#if playCount(game.id) > 0}
                  <span class="flex items-center gap-1 text-slate-500"><RotateCcw size={10} /> {playCount(game.id)}×</span>
                {:else}
                  <span class="flex items-center gap-1 text-blue-400"><PlayCircle size={11} /> Play</span>
                {/if}
              </div>
            </div>
          </button>
        {/each}
      </div>
      <p class="text-xs text-slate-600 mt-6 text-center">High scores save automatically.</p>
    </div>

  {:else}
    <!-- Library tab: external native/Windows games -->
    <div class="flex-1 overflow-y-auto p-5 space-y-5">
      <div class="bg-slate-800/60 border border-white/5 rounded-xl p-3">
        <div class="flex items-center justify-between mb-2">
          <span class="text-xs font-semibold text-slate-300">Windows game support</span>
          {#if runtimesLoading}<Loader2 size={13} class="animate-spin text-slate-500" />{/if}
        </div>
        {#if runtimes}
          <div class="flex flex-wrap gap-3 text-xs">
            <span class="flex items-center gap-1 {runtimes.wine_available ? 'text-green-400' : 'text-slate-500'}">
              {#if runtimes.wine_available}<CheckCircle2 size={12} />{:else}<XCircle size={12} />{/if} Wine{runtimes.wine_version ? ` (${runtimes.wine_version})` : ''}
            </span>
            <span class="flex items-center gap-1 {runtimes.steam_proton_versions.length > 0 || runtimes.proton_available ? 'text-green-400' : 'text-slate-500'}">
              {#if runtimes.steam_proton_versions.length > 0 || runtimes.proton_available}<CheckCircle2 size={12} />{:else}<XCircle size={12} />{/if}
              Proton{runtimes.steam_proton_versions.length > 0 ? ` (${runtimes.steam_proton_versions.length} version${runtimes.steam_proton_versions.length === 1 ? '' : 's'})` : ''}
            </span>
            <span class="flex items-center gap-1 {runtimes.umu_available ? 'text-green-400' : 'text-slate-500'}">
              {#if runtimes.umu_available}<CheckCircle2 size={12} />{:else}<XCircle size={12} />{/if} umu-run
            </span>
          </div>
          {#if !canAddWindows}
            <p class="text-[11px] text-amber-400 mt-2">None found — install Wine (or umu-launcher) to add Windows games. Native Linux games work either way.</p>
          {/if}
        {/if}
      </div>

      <div class="flex gap-2">
        <button on:click={() => openAddDialog('native')} class="flex-1 flex items-center justify-center gap-2 py-2.5 rounded-xl border border-dashed border-white/10 text-sm text-slate-300 hover:border-white/20 hover:text-white">
          <HardDrive size={14} /> Add native Linux game
        </button>
        <button on:click={() => openAddDialog('windows')} disabled={!canAddWindows} class="flex-1 flex items-center justify-center gap-2 py-2.5 rounded-xl border border-dashed border-white/10 text-sm text-slate-300 hover:border-white/20 hover:text-white disabled:opacity-40">
          <Monitor size={14} /> Add Windows game (.exe)
        </button>
      </div>

      {#if launchError}
        <div class="bg-red-500/10 border border-red-500/20 text-red-300 text-xs rounded-lg px-3 py-2 flex items-start justify-between gap-2">
          <span>{launchError}</span>
          <button on:click={() => (launchError = null)}><X size={12} /></button>
        </div>
      {/if}

      {#if library.length === 0}
        <div class="text-center text-sm text-slate-500 py-10">No games added yet.</div>
      {:else}
        <div class="space-y-2">
          {#each library as entry (entry.id)}
            <div class="flex items-center gap-3 bg-slate-800/60 border border-white/5 rounded-xl px-3 py-2.5">
              {#if entry.kind === 'windows'}<Monitor size={16} class="text-blue-400 shrink-0" />{:else}<HardDrive size={16} class="text-green-400 shrink-0" />{/if}
              <div class="min-w-0 flex-1">
                <div class="text-sm font-medium truncate">{entry.title}</div>
                <div class="text-[11px] text-slate-500 truncate flex items-center gap-2">
                  <span>{entry.execPath}</span>
                  {#if playtime(entry.id) > 0}<span class="flex items-center gap-0.5 shrink-0"><Clock size={9} /> {formatPlaytime(playtime(entry.id))}</span>{/if}
                </div>
              </div>
              <button on:click={() => launchExternal(entry)} disabled={launchingId === entry.id}
                class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-lg text-xs font-medium shrink-0">
                {#if launchingId === entry.id}<Loader2 size={12} class="animate-spin" /> Running…{:else}<PlayCircle size={12} /> Play{/if}
              </button>
              <button on:click={() => removeGame(entry)} class="p-1.5 text-slate-500 hover:text-red-400 shrink-0"><Trash2 size={13} /></button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

{#if showAddDialog}
  <div class="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-4" on:click={() => (showAddDialog = false)} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => (showAddDialog = false))(); } }}>
    <div class="bg-slate-900 border border-white/10 rounded-2xl p-5 w-full max-w-sm space-y-4" on:click|stopPropagation>
      <div class="flex items-center justify-between">
        <h3 class="font-semibold text-sm">Add {addKind === 'windows' ? 'Windows' : 'native Linux'} game</h3>
        <button on:click={() => (showAddDialog = false)} class="text-slate-500 hover:text-white"><X size={16} /></button>
      </div>
      <div>
        <span class="text-xs text-slate-400 block mb-1">Executable</span>
        <button on:click={pickExecutable} class="w-full flex items-center gap-2 px-3 py-2 bg-slate-800 border border-white/10 rounded-lg text-sm text-left text-slate-300 hover:bg-slate-700">
          <FolderOpen size={14} class="shrink-0" />
          <span class="truncate">{addPath || 'Choose file…'}</span>
        </button>
      </div>
      <div>
        <span class="text-xs text-slate-400 block mb-1">Name</span>
        <input bind:value={addTitle} placeholder="Game title" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none" />
      </div>
      {#if addKind === 'windows' && runtimes}
        <div>
          <span class="text-xs text-slate-400 block mb-1">Run with</span>
          <select bind:value={addRuntime} class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none">
            {#if runtimes.umu_available}<option value="umu">umu-run (recommended)</option>{/if}
            {#if runtimes.steam_proton_versions.length > 0 || runtimes.proton_available}<option value="proton">Proton</option>{/if}
            {#if runtimes.wine_available}<option value="wine">Wine</option>{/if}
          </select>
          {#if addRuntime === 'proton' && runtimes.steam_proton_versions.length > 0}
            <select bind:value={addProtonPath} class="w-full mt-2 bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none">
              {#each runtimes.steam_proton_versions as p (p.path)}<option value={p.path}>{p.name}</option>{/each}
            </select>
          {/if}
        </div>
      {/if}
      <button on:click={confirmAddGame} disabled={!addTitle.trim() || !addPath.trim()}
        class="w-full py-2.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded-xl text-sm font-semibold flex items-center justify-center gap-2">
        <Plus size={14} /> Add to library
      </button>
    </div>
  </div>
{/if}
