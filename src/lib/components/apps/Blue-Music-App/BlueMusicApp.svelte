<script lang="ts">
  /**
   * Blue Music — local library player.
   *
   * Scans your music folders recursively (tags, covers, durations come from the backend and are
   * cached), browses by Songs / Albums / Artists, keeps a real queue (proper shuffle, repeat
   * off/all/one), user playlists, remembers where you stopped, and registers as an MPRIS player
   * so media keys, headphones and desktop widgets can control it.
   */
  import { onMount, onDestroy, tick } from 'svelte';
  import {
    Play, Pause, SkipBack, SkipForward, Music, FolderOpen, Repeat, Repeat1, Shuffle, Volume2, VolumeX, ListMusic, Disc3, Mic2,
    Search, Plus, ListPlus, ChevronLeft, RefreshCw, X, ListEnd, Trash2, ListOrdered,
  } from 'lucide-svelte';
  import { SystemBridge, toAssetUrl } from '../../../utils/systemBridge';
  import { t } from '../../../stores/language';
  import { showContextMenu, type MenuItem } from '../../../stores/contextMenu';
  import { dialogPrompt, dialogConfirm } from '../../../stores/dialog';
  import { activeWindowId } from '../../../stores/windowManager';
  import { pickPaths } from '../../../stores/filePicker';
  import Cover from './Cover.svelte';
  import TrackList from './TrackList.svelte';
  import {
    type Track, type QueueState, type RepeatMode, type Playlist, type AlbumGroup,
    sortTracks, groupAlbums, groupArtists, searchTracks, fmtTime, totalDuration,
    makeQueue, emptyQueue, nextPos, prevPos, setShuffle, cycleRepeat, enqueueNext, enqueueEnd, removeAt, pruneQueue, currentPath as queueCurrent,
    newPlaylistId, addToPlaylist, removeFromPlaylist, resolvePlaylist, repeatToMpris, repeatFromMpris, defaultState, shuffled,
  } from './musicLogic';
  import { loadState, loadPlaylists, stateWriter, playlistWriter, scanLibrary, coverFile, clearCoverCache } from './musicIO';
  import { startMpris, pushMpris, type MprisCmd } from './mpris';

  export let windowId: string;
  /** Set when a single audio file was opened from Files. */
  export let openPath: string | undefined = undefined;

  type Section = 'songs' | 'albums' | 'artists' | 'queue' | `pl:${string}`;

  // ── Library ──────────────────────────────────────────────────────────
  let tracks: Track[] = [];
  let byPath = new Map<string, Track>();
  let folders: string[] = defaultState().folders;
  let loading = true;
  let scanNote = '';
  let showFolders = false;

  $: byPath = new Map(tracks.map((x) => [x.path, x]));
  $: albums = groupAlbums(tracks);
  $: artists = groupArtists(tracks);

  async function scan(initial = false) {
    loading = true;
    try {
      const res = await scanLibrary(folders);
      tracks = sortTracks(res.tracks);
      scanNote = res.missingFolders.length ? $t('music.missing_folder', { path: res.missingFolders[0] }) : '';
      queue = pruneQueue(queue, (p) => byPath.has(p) || extraTracks.has(p));
      if (!initial) flash($t('music.scan_done', { n: tracks.length }));
    } catch (e: any) {
      scanNote = String(e?.message ?? e);
    } finally { loading = false; }
  }

  // ── Navigation ───────────────────────────────────────────────────────
  let section: Section = 'songs';
  let detail: { type: 'album' | 'artist'; key: string } | null = null;
  let query = '';
  let searchEl: HTMLInputElement;
  let listRef: TrackList | undefined;

  $: playlistId = section.startsWith('pl:') ? section.slice(3) : null;
  $: playlist = playlistId ? playlists.find((p) => p.id === playlistId) ?? null : null;
  $: searching = query.trim().length > 0;
  $: detailAlbum = detail?.type === 'album' ? albums.find((a) => a.key === detail!.key) ?? null : null;
  $: detailArtist = detail?.type === 'artist' ? artists.find((a) => a.key === detail!.key) ?? null : null;

  /** The tracks shown as a list in the main pane (null → a grid/list of albums/artists is shown instead). */
  $: visibleTracks = (() => {
    if (searching) return searchTracks(tracks, query);
    if (detailAlbum) return detailAlbum.tracks;
    if (detailArtist) return detailArtist.albums.flatMap((a) => a.tracks);
    if (section === 'songs') return tracks;
    if (section === 'queue') return queueTracks;
    if (playlist) return resolvePlaylist(playlist, byPath);
    return null;
  })();

  function go(s: Section) { section = s; detail = null; query = ''; }
  const heading = () =>
    searching ? $t('music.search_results') : detailAlbum ? (detailAlbum.album || $t('music.unknown_album')) : detailArtist ? (detailArtist.name || $t('music.unknown_artist'))
    : section === 'songs' ? $t('music.songs') : section === 'albums' ? $t('music.albums') : section === 'artists' ? $t('music.artists') : section === 'queue' ? $t('music.queue') : (playlist?.name ?? '');

  // ── Playback ─────────────────────────────────────────────────────────
  let audioEl: HTMLAudioElement;
  let queue: QueueState = emptyQueue();
  let extraTracks = new Map<string, Track>();        // files opened from outside the library
  let isPlaying = false;
  let currentTime = 0;
  let duration = 0;
  let volume = 0.8;
  let muted = false;
  let failed = new Set<string>();
  let consecutiveFailures = 0;
  let pendingSeek = 0;                                // resume position, applied once metadata is loaded

  const lookup = (p: string | null) => (p ? byPath.get(p) ?? extraTracks.get(p) ?? null : null);
  $: currentTrack = lookup(queueCurrent(queue));
  $: queueTracks = queue.items.map((p) => lookup(p)).filter((x): x is Track => !!x);
  $: albumOfCurrent = currentTrack ? albums.find((a) => a.tracks.some((x) => x.path === currentTrack!.path)) : null;
  $: if (audioEl) audioEl.volume = muted ? 0 : volume;

  function setSource(path: string | null) {
    if (!audioEl) return;
    if (!path) { audioEl.removeAttribute('src'); audioEl.load(); duration = 0; currentTime = 0; return; }
    audioEl.src = toAssetUrl(path);
    currentTime = 0; duration = lookup(path)?.durationSecs ?? 0;
  }

  async function playCurrent(autoplay = true) {
    setSource(queueCurrent(queue));
    if (autoplay && audioEl) { try { await audioEl.play(); } catch { /* the error event handles unplayable files */ } }
  }

  function startQueue(list: Track[], startPath: string | null, forceShuffle?: boolean) {
    const shuffleOn = forceShuffle ?? queue.shuffle;
    queue = makeQueue(list.map((x) => x.path), startPath, shuffleOn, queue.repeat);
    playCurrent(true);
  }
  /** Click on a row of the list that is currently displayed → that list becomes the queue. */
  function playFromList(list: Track[], index: number) {
    const target = list[index];
    if (!target) return;
    if (section === 'queue' && !searching && !detail) { queue = { ...queue, pos: queue.items.indexOf(target.path) }; playCurrent(true); return; }
    startQueue(list, target.path);
  }

  function togglePlay() {
    if (!currentTrack) {
      const list = visibleTracks && visibleTracks.length ? visibleTracks : tracks;
      if (list.length) startQueue(list, queue.shuffle ? null : list[0].path);
      return;
    }
    if (!audioEl.src) { playCurrent(true); return; }
    if (audioEl.paused) audioEl.play().catch(() => {}); else audioEl.pause();
  }

  function step(to: number | null) {
    if (to === null) { audioEl?.pause(); if (audioEl) audioEl.currentTime = 0; return; }
    queue = { ...queue, pos: to };
    playCurrent(true);
  }
  const next = (auto = false) => step(nextPos(queue, auto));
  function prev() {
    if (currentTime > 3 && audioEl) { seekTo(0); return; }
    step(prevPos(queue));
  }
  function stop() { audioEl?.pause(); seekTo(0); }

  function seekTo(secs: number, notify = true) {
    if (!audioEl) return;
    const max = isFinite(audioEl.duration) ? audioEl.duration : duration;
    audioEl.currentTime = Math.max(0, max ? Math.min(max, secs) : secs);
    currentTime = audioEl.currentTime;
    if (notify) pushRemote(true);
  }
  const seekBy = (d: number) => seekTo(currentTime + d);
  function onSeekInput(e: Event) { seekTo(Number((e.currentTarget as HTMLInputElement).value)); }

  const toggleShuffle = () => { queue = setShuffle(queue, !queue.shuffle); };
  const toggleRepeat = () => { queue = { ...queue, repeat: cycleRepeat(queue.repeat) }; };

  function onPlaying() { isPlaying = true; consecutiveFailures = 0; }
  function onLoadedMetadata() {
    if (isFinite(audioEl.duration)) duration = audioEl.duration;
    if (pendingSeek > 0) { audioEl.currentTime = Math.min(pendingSeek, audioEl.duration || pendingSeek); pendingSeek = 0; }
  }
  function onError() {
    const p = queueCurrent(queue);
    if (!p || !audioEl?.getAttribute('src')) return;
    failed = new Set(failed).add(p);
    isPlaying = false;
    flash($t('music.cant_play', { name: lookup(p)?.title ?? p.split('/').pop() ?? p }));
    consecutiveFailures++;
    // skip forward, but give up once we've failed on (about) every track — avoids spinning forever
    if (consecutiveFailures < Math.min(5, queue.items.length)) { const to = nextPos(queue, false); if (to !== null && to !== queue.pos) step(to); }
  }

  // ── Playlists ────────────────────────────────────────────────────────
  let playlists: Playlist[] = [];
  const savePlaylists = (next: Playlist[]) => { playlists = next; playlistWriter.write(next); };

  async function createPlaylist(initial: string[] = []): Promise<Playlist | null> {
    const name = await dialogPrompt({ title: $t('music.new_playlist'), label: $t('music.playlist_name'), placeholder: $t('music.playlist_name'), confirmLabel: $t('music.create') });
    if (!name) return null;
    const pl: Playlist = { id: newPlaylistId(), name: name.trim().slice(0, 120), tracks: [...new Set(initial)] };
    savePlaylists([...playlists, pl]);
    return pl;
  }
  async function renamePlaylist(pl: Playlist) {
    const name = await dialogPrompt({ title: $t('music.rename'), label: $t('music.playlist_name'), defaultValue: pl.name, confirmLabel: $t('music.rename') });
    if (name) savePlaylists(playlists.map((p) => (p.id === pl.id ? { ...p, name: name.trim().slice(0, 120) } : p)));
  }
  async function deletePlaylist(pl: Playlist) {
    if (!(await dialogConfirm({ title: $t('music.delete_playlist'), message: pl.name, confirmLabel: $t('music.delete'), danger: true }))) return;
    savePlaylists(playlists.filter((p) => p.id !== pl.id));
    if (section === `pl:${pl.id}`) go('songs');
  }
  function addPaths(pl: Playlist, paths: string[]) {
    savePlaylists(playlists.map((p) => (p.id === pl.id ? addToPlaylist(p, paths) : p)));
    flash($t('music.added_to', { name: pl.name }));
  }

  // ── Context menus ────────────────────────────────────────────────────
  const unknownArtist = () => $t('music.unknown_artist');

  function addToMenu(paths: string[]): MenuItem {
    return {
      label: $t('music.add_to_playlist'), icon: ListPlus,
      children: [
        { label: $t('music.new_playlist') + '…', icon: Plus, action: async () => { const pl = await createPlaylist(paths); if (pl) flash($t('music.added_to', { name: pl.name })); } },
        ...(playlists.length ? [{ separator: true } as MenuItem] : []),
        ...playlists.map((pl) => ({ label: pl.name, action: () => addPaths(pl, paths) })),
      ],
    };
  }

  function trackMenu(e: MouseEvent, list: Track[], index: number) {
    const track = list[index];
    if (!track) return;
    const items: MenuItem[] = [
      { label: $t('music.play'), icon: Play, action: () => playFromList(list, index) },
      { label: $t('music.play_next'), icon: ListOrdered, action: () => { queue = queue.items.length ? enqueueNext(queue, track.path) : makeQueue([track.path], track.path, false, queue.repeat); if (queue.items.length === 1) playCurrent(true); } },
      { label: $t('music.add_to_queue'), icon: ListEnd, action: () => { const was = queue.items.length; queue = enqueueEnd(queue, [track.path]); if (!was) playCurrent(true); else flash($t('music.added_to_queue')); } },
      addToMenu([track.path]),
      { separator: true },
      ...(track.album ? [{ label: $t('music.go_album'), icon: Disc3, action: () => { const a = albums.find((x) => x.tracks.some((y) => y.path === track.path)); if (a) { section = 'albums'; detail = { type: 'album', key: a.key }; query = ''; } } } as MenuItem] : []),
      ...(track.artist || track.albumArtist ? [{ label: $t('music.go_artist'), icon: Mic2, action: () => { const a = artists.find((x) => x.albums.some((al) => al.tracks.some((y) => y.path === track.path))); if (a) { section = 'artists'; detail = { type: 'artist', key: a.key }; query = ''; } } } as MenuItem] : []),
      ...(playlist && !searching && !detail ? [{ label: $t('music.remove_from_playlist'), icon: X, action: () => savePlaylists(playlists.map((p) => (p.id === playlist!.id ? removeFromPlaylist(p, playlist!.tracks.indexOf(track.path)) : p))) } as MenuItem] : []),
      ...(section === 'queue' && !searching && !detail ? [{ label: $t('music.remove_from_queue'), icon: X, action: () => { const was = queue.pos === index; queue = removeAt(queue, index); if (was) playCurrent(isPlaying); } } as MenuItem] : []),
      { separator: true },
      { label: $t('music.show_in_folder'), icon: FolderOpen, action: () => SystemBridge.executeCommand(`xdg-open '${(track.path.slice(0, track.path.lastIndexOf('/')) || '/').replace(/'/g, "'\\''")}' >/dev/null 2>&1 &`) },
      { label: $t('music.move_to_trash'), icon: Trash2, danger: true, action: async () => {
          try { await SystemBridge.moveToTrash([track.path]); tracks = tracks.filter((x) => x.path !== track.path); const was = queueCurrent(queue) === track.path; queue = pruneQueue(queue, (p) => p !== track.path); if (was) playCurrent(isPlaying); }
          catch (err: any) { flash(String(err?.message ?? err)); }
      } },
    ];
    showContextMenu(e, items);
  }

  // ── Folders ──────────────────────────────────────────────────────────
  async function addFolder() {
    const [dir] = await pickPaths({ mode: 'directory', title: $t('music.add_folder'), rememberKey: 'music.folders' });
    if (!dir || folders.includes(dir)) return;
    folders = [...folders, dir];
    await scan();
  }
  async function removeFolder(f: string) {
    if (folders.length <= 1) return;
    folders = folders.filter((x) => x !== f);
    await scan();
  }

  // ── Status line ──────────────────────────────────────────────────────
  let status = '';
  let statusTimer: ReturnType<typeof setTimeout>;
  function flash(text: string) { status = text; clearTimeout(statusTimer); statusTimer = setTimeout(() => (status = ''), 5000); }

  // ── Persistence ──────────────────────────────────────────────────────
  let restored = false;
  function persist() {
    if (!restored) return;
    stateWriter.write({ folders, volume, muted, shuffle: queue.shuffle, repeat: queue.repeat, queue: queue.items, pos: queue.pos, positionSecs: currentTime });
  }
  $: if (restored) { folders; volume; muted; queue; persist(); }
  let posTimer: ReturnType<typeof setInterval>;

  // ── MPRIS ────────────────────────────────────────────────────────────
  let mprisTimer: ReturnType<typeof setTimeout>;
  let artUrl = '';
  $: if (currentTrack) coverFile(currentTrack.path, albumOfCurrent?.key ?? currentTrack.path).then((p) => (artUrl = p ? `file://${p}` : ''));
  $: if (!currentTrack) artUrl = '';

  function pushRemote(seeked = false) {
    clearTimeout(mprisTimer);
    mprisTimer = setTimeout(() => {
      pushMpris({
        hasTrack: !!currentTrack, playing: isPlaying, title: currentTrack?.title ?? '', artist: currentTrack?.artist ?? '', album: currentTrack?.album ?? '', artUrl,
        lengthSecs: duration || currentTrack?.durationSecs || 0, positionSecs: currentTime, volume: muted ? 0 : volume, shuffle: queue.shuffle,
        loopStatus: repeatToMpris(queue.repeat), canNext: queue.items.length > 1 || queue.repeat === 'all', canPrev: queue.items.length > 0, seeked,
      });
    }, 80);
  }
  $: { currentTrack; isPlaying; volume; muted; queue.shuffle; queue.repeat; artUrl; queue.items.length; if (restored) pushRemote(); }

  function onRemote(c: MprisCmd) {
    switch (c.cmd) {
      case 'play': if (audioEl?.paused) togglePlay(); break;
      case 'pause': audioEl?.pause(); break;
      case 'playPause': togglePlay(); break;
      case 'stop': stop(); break;
      case 'next': next(false); break;
      case 'previous': prev(); break;
      case 'seek': seekBy(c.value / 1_000_000); break;
      case 'setPosition': seekTo(c.value / 1_000_000); break;
      case 'setVolume': volume = Math.min(1, Math.max(0, c.value)); muted = false; break;
      case 'setShuffle': if (c.value !== queue.shuffle) toggleShuffle(); break;
      case 'setLoop': queue = { ...queue, repeat: repeatFromMpris(c.value) }; break;
    }
  }

  // ── Keyboard (only for the focused window, never while typing) ───────
  function onKey(e: KeyboardEvent) {
    if ($activeWindowId !== windowId || e.defaultPrevented) return;
    const el = e.target as HTMLElement | null;
    const typing = !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable);
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') { e.preventDefault(); searchEl?.focus(); return; }
    if (typing || e.ctrlKey || e.metaKey || e.altKey) return;
    switch (e.key) {
      case ' ': e.preventDefault(); togglePlay(); break;
      case 'ArrowRight': e.preventDefault(); e.shiftKey ? next(false) : seekBy(5); break;
      case 'ArrowLeft': e.preventDefault(); e.shiftKey ? prev() : seekBy(-5); break;
      case 'ArrowUp': e.preventDefault(); volume = Math.min(1, volume + 0.05); muted = false; break;
      case 'ArrowDown': e.preventDefault(); volume = Math.max(0, volume - 0.05); break;
      case 'm': case 'M': muted = !muted; break;
      case 's': case 'S': toggleShuffle(); break;
      case 'r': case 'R': toggleRepeat(); break;
    }
  }

  // ── Lifecycle ────────────────────────────────────────────────────────
  let stopMpris: () => void = () => {};
  const lifetime = { destroyed: false };

  onMount(async () => {
    const [state, pls] = await Promise.all([loadState(), loadPlaylists()]);
    playlists = pls;
    folders = state.folders; volume = state.volume; muted = state.muted;
    await scan(true);
    // resume where you left off (paused — never blasts music at startup)
    const q = pruneQueue({ ...emptyQueue(state.shuffle, state.repeat), items: state.queue, original: state.queue, pos: state.pos }, (p) => byPath.has(p));
    queue = q;
    restored = true;
    if (queueCurrent(queue)) { pendingSeek = state.positionSecs; setSource(queueCurrent(queue)); }

    if (openPath) {                                                  // a single file opened from Files
      let t = byPath.get(openPath);
      if (!t) { t = { path: openPath, title: openPath.split('/').pop()?.replace(/\.[^.]+$/, '') ?? openPath, artist: '', album: '', albumArtist: '', trackNo: 0, discNo: 0, year: 0, genre: '', durationSecs: 0 }; extraTracks = new Map(extraTracks).set(openPath, t); }
      pendingSeek = 0;
      queue = makeQueue(byPath.has(openPath) ? tracks.map((x) => x.path) : [openPath], openPath, queue.shuffle, queue.repeat);
      await playCurrent(true);
    }
    stopMpris = await startMpris(onRemote);
    if (lifetime.destroyed) stopMpris();
    posTimer = setInterval(() => { if (isPlaying) persist(); }, 5000);
  });

  onDestroy(() => {
    lifetime.destroyed = true;
    clearInterval(posTimer); clearTimeout(statusTimer); clearTimeout(mprisTimer);
    stateWriter.flush(); playlistWriter.flush();
    stopMpris();
    audioEl?.pause();
    clearCoverCache();
  });

  const NAV: { id: Section; icon: any; key: string }[] = [
    { id: 'songs', icon: Music, key: 'music.songs' }, { id: 'albums', icon: Disc3, key: 'music.albums' },
    { id: 'artists', icon: Mic2, key: 'music.artists' }, { id: 'queue', icon: ListMusic, key: 'music.queue' },
  ];
  const meta = (a: AlbumGroup) => [a.year || '', `${a.tracks.length} ${$t('music.tracks')}`].filter(Boolean).join(' · ');
</script>

<svelte:window on:keydown={onKey} />

<div class="flex h-full bg-slate-900 text-white text-sm overflow-hidden">
  <!-- Sidebar -->
  <aside class="w-52 shrink-0 border-r border-white/5 flex flex-col min-h-0">
    <div class="p-3">
      <div class="relative">
        <Search size={13} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
        <input bind:this={searchEl} bind:value={query} placeholder={$t('music.search')} class="w-full bg-slate-800 border border-white/5 rounded-lg pl-8 pr-7 py-1.5 text-xs placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60" />
        {#if query}<button on:click={() => (query = '')} class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white"><X size={12} /></button>{/if}
      </div>
    </div>
    <nav class="px-2 space-y-0.5">
      {#each NAV as n (n.id)}
        <button on:click={() => go(n.id)} class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-xs transition-colors {section === n.id && !searching ? 'bg-blue-600/25 text-blue-200' : 'text-slate-300 hover:bg-white/5'}">
          <svelte:component this={n.icon} size={14} /> <span class="flex-1 text-left">{$t(n.key)}</span>
          {#if n.id === 'queue' && queue.items.length}<span class="text-[10px] text-slate-500">{queue.items.length}</span>{/if}
        </button>
      {/each}
    </nav>
    <div class="px-4 pt-4 pb-1 flex items-center justify-between text-[10px] font-semibold uppercase tracking-wider text-slate-500">
      <span>{$t('music.playlists')}</span>
      <button on:click={() => createPlaylist()} title={$t('music.new_playlist')} class="p-0.5 hover:text-white"><Plus size={12} /></button>
    </div>
    <div class="flex-1 overflow-y-auto px-2 pb-2 space-y-0.5">
      {#each playlists as pl (pl.id)}
        <button on:click={() => go(`pl:${pl.id}`)} on:contextmenu|preventDefault={(e) => showContextMenu(e, [
            { label: $t('music.play'), icon: Play, disabled: !pl.tracks.length, action: () => { const l = resolvePlaylist(pl, byPath); if (l.length) startQueue(l, queue.shuffle ? null : l[0].path); } },
            { label: $t('music.rename'), action: () => renamePlaylist(pl) }, { separator: true },
            { label: $t('music.delete_playlist'), icon: Trash2, danger: true, action: () => deletePlaylist(pl) } ])}
          class="w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs transition-colors {section === `pl:${pl.id}` && !searching ? 'bg-blue-600/25 text-blue-200' : 'text-slate-400 hover:bg-white/5'}">
          <ListMusic size={13} class="shrink-0" /> <span class="truncate flex-1 text-left">{pl.name}</span><span class="text-[10px] text-slate-600">{pl.tracks.length}</span>
        </button>
      {:else}
        <p class="px-2.5 py-1.5 text-[11px] text-slate-600">{$t('music.no_playlists')}</p>
      {/each}
    </div>
    <button on:click={() => (showFolders = true)} class="m-2 flex items-center gap-2 px-2.5 py-2 rounded-lg text-xs text-slate-400 hover:bg-white/5 hover:text-white">
      <FolderOpen size={13} /> {$t('music.folders')}
    </button>
  </aside>

  <!-- Main -->
  <div class="flex-1 min-w-0 flex flex-col">
    <header class="flex items-center gap-2 px-5 h-12 border-b border-white/5 shrink-0">
      {#if detail && !searching}<button on:click={() => (detail = null)} class="p-1 -ml-2 rounded-lg hover:bg-white/10 text-slate-400"><ChevronLeft size={16} /></button>{/if}
      <h1 class="text-base font-semibold truncate">{heading()}</h1>
      {#if visibleTracks}<span class="text-xs text-slate-500 shrink-0">{visibleTracks.length} {$t('music.tracks')}{#if visibleTracks.length} · {fmtTime(totalDuration(visibleTracks))}{/if}</span>{/if}
      <div class="flex-1" />
      {#if visibleTracks && visibleTracks.length}
        <button on:click={() => startQueue(visibleTracks, queue.shuffle ? null : visibleTracks[0].path)} class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs"><Play size={12} /> {$t('music.play')}</button>
        <button on:click={() => startQueue(visibleTracks, null, true)} class="flex items-center gap-1.5 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs"><Shuffle size={12} /> {$t('music.shuffle')}</button>
      {/if}
      {#if section === 'queue' && !searching && queue.items.length}
        <button on:click={() => { audioEl?.pause(); queue = emptyQueue(queue.shuffle, queue.repeat); setSource(null); }} class="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">{$t('music.clear_queue')}</button>
      {/if}
      {#if playlist && !searching && !detail}
        <button on:click={() => renamePlaylist(playlist)} class="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">{$t('music.rename')}</button>
      {/if}
      <button on:click={() => { clearCoverCache(); scan(); }} title={$t('music.rescan')} class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400"><RefreshCw size={14} class={loading ? 'animate-spin' : ''} /></button>
    </header>

    {#if status}<div class="px-5 py-1.5 bg-blue-500/10 text-blue-200 text-xs shrink-0 truncate">{status}</div>{/if}
    {#if scanNote}<div class="px-5 py-1.5 bg-amber-500/10 text-amber-300 text-xs shrink-0 truncate">{scanNote}</div>{/if}

    <div class="flex-1 min-h-0">
      {#if loading && !tracks.length}
        <div class="h-full flex items-center justify-center text-slate-500 text-xs gap-2"><RefreshCw size={14} class="animate-spin" /> {$t('music.loading')}</div>
      {:else if visibleTracks}
        {#if visibleTracks.length === 0}
          <div class="h-full flex flex-col items-center justify-center text-slate-500 gap-2 px-8 text-center">
            <ListMusic size={30} class="opacity-30" />
            <span class="text-xs">{searching ? $t('music.no_results') : section === 'queue' ? $t('music.queue_empty') : playlist ? $t('music.playlist_empty') : $t('music.empty')}</span>
            {#if section === 'songs' && !searching}<span class="text-[10px] text-slate-600 font-mono">{folders.join(' · ')}</span>{/if}
          </div>
        {:else}
          <TrackList bind:this={listRef} tracks={visibleTracks} currentPath={queueCurrent(queue)} playing={isPlaying} numbered={!!detailAlbum} {failed}
            on:play={(e) => playFromList(visibleTracks, e.detail)} on:menu={(e) => trackMenu(e.detail.event, visibleTracks, e.detail.index)} />
        {/if}
      {:else if section === 'albums' && !detail}
        <div class="h-full overflow-y-auto p-5 grid gap-4 content-start" style="grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));">
          {#each albums as a (a.key)}
            <button on:click={() => (detail = { type: 'album', key: a.key })} class="text-left group">
              <div class="aspect-square w-full rounded-xl overflow-hidden ring-1 ring-white/5 group-hover:ring-blue-500/50 transition-shadow">
                <Cover trackPath={a.tracks[0].path} cacheKey={a.key} size={400} rounded="rounded-none" />
              </div>
              <div class="mt-2 text-xs font-medium truncate">{a.album || $t('music.unknown_album')}</div>
              <div class="text-[11px] text-slate-500 truncate">{a.artist || unknownArtist()}</div>
              <div class="text-[10px] text-slate-600">{meta(a)}</div>
            </button>
          {/each}
        </div>
      {:else if section === 'artists' && !detail}
        <div class="h-full overflow-y-auto">
          {#each artists as ar (ar.key)}
            <button on:click={() => (detail = { type: 'artist', key: ar.key })} class="w-full flex items-center gap-3 px-5 py-2.5 hover:bg-white/5 text-left">
              <Cover trackPath={ar.albums[0].tracks[0].path} cacheKey={ar.albums[0].key} size={40} rounded="rounded-full" />
              <div class="min-w-0 flex-1"><div class="text-[13px] truncate">{ar.name || unknownArtist()}</div>
                <div class="text-[11px] text-slate-500">{ar.albums.length} {$t('music.albums_n')} · {ar.trackCount} {$t('music.tracks')}</div></div>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Now playing -->
    <div class="border-t border-white/5 px-4 py-2.5 shrink-0 flex items-center gap-4 bg-slate-900/80">
      <div class="flex items-center gap-3 w-64 min-w-0">
        <Cover trackPath={currentTrack?.path ?? null} cacheKey={albumOfCurrent?.key ?? currentTrack?.path} size={44} />
        <div class="min-w-0">
          <div class="text-[13px] truncate">{currentTrack?.title ?? ''}</div>
          <div class="text-[11px] text-slate-500 truncate">{currentTrack ? (currentTrack.artist || unknownArtist()) : ''}</div>
        </div>
      </div>
      <div class="flex-1 min-w-0 flex flex-col gap-1">
        <div class="flex items-center justify-center gap-1">
          <button on:click={toggleShuffle} title="{$t('music.shuffle')} (S)" class="p-1.5 rounded-lg hover:bg-white/10 {queue.shuffle ? 'text-blue-400' : 'text-slate-500'}"><Shuffle size={14} /></button>
          <button on:click={prev} class="p-2 rounded-full hover:bg-white/10"><SkipBack size={16} /></button>
          <button on:click={togglePlay} class="p-2.5 rounded-full bg-blue-600 hover:bg-blue-500 mx-1">{#if isPlaying}<Pause size={16} />{:else}<Play size={16} />{/if}</button>
          <button on:click={() => next(false)} class="p-2 rounded-full hover:bg-white/10"><SkipForward size={16} /></button>
          <button on:click={toggleRepeat} title="{$t(`music.repeat_${queue.repeat}`)} (R)" class="p-1.5 rounded-lg hover:bg-white/10 {queue.repeat !== 'off' ? 'text-blue-400' : 'text-slate-500'}">
            {#if queue.repeat === 'one'}<Repeat1 size={14} />{:else}<Repeat size={14} />{/if}
          </button>
        </div>
        <div class="flex items-center gap-2 text-[10px] text-slate-500">
          <span class="w-10 text-right tabular-nums">{fmtTime(currentTime)}</span>
          <input type="range" min="0" max={duration || 0} step="0.5" value={currentTime} on:input={onSeekInput} disabled={!currentTrack} class="flex-1 accent-blue-500" />
          <span class="w-10 tabular-nums">{fmtTime(duration)}</span>
        </div>
      </div>
      <div class="flex items-center gap-1.5 w-32 justify-end">
        <button on:click={() => (muted = !muted)} class="text-slate-500 hover:text-white shrink-0" title="{$t('music.mute')} (M)">{#if muted || volume === 0}<VolumeX size={15} />{:else}<Volume2 size={15} />{/if}</button>
        <input type="range" min="0" max="1" step="0.01" value={muted ? 0 : volume} on:input={(e) => { volume = Number(e.currentTarget.value); muted = false; }} class="w-20 accent-blue-500" />
      </div>
    </div>
  </div>
</div>

<audio bind:this={audioEl} on:playing={onPlaying} on:pause={() => (isPlaying = false)} on:ended={() => next(true)} on:error={onError}
  on:timeupdate={() => (currentTime = audioEl.currentTime)} on:durationchange={() => { if (isFinite(audioEl.duration)) duration = audioEl.duration; }} on:loadedmetadata={onLoadedMetadata} />

{#if showFolders}
  <div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm" on:mousedown={() => (showFolders = false)} role="presentation">
    <div class="w-[28rem] max-h-[80vh] flex flex-col bg-slate-800 border border-white/10 rounded-2xl shadow-2xl p-5" on:mousedown|stopPropagation role="dialog" aria-modal="true">
      <div class="flex items-center justify-between mb-1"><h3 class="text-sm font-semibold">{$t('music.folders')}</h3><button on:click={() => (showFolders = false)} class="p-1 hover:bg-white/10 rounded text-slate-500"><X size={14} /></button></div>
      <p class="text-[11px] text-slate-400 mb-3">{$t('music.folders_desc')}</p>
      <div class="flex-1 overflow-y-auto space-y-1.5">
        {#each folders as f (f)}
          <div class="flex items-center gap-2 p-2 rounded-lg bg-slate-900/60 border border-white/5">
            <FolderOpen size={13} class="text-slate-500 shrink-0" /><span class="flex-1 truncate text-xs font-mono" title={f}>{f.replace(/^HOME/, '~')}</span>
            <button on:click={() => removeFolder(f)} disabled={folders.length <= 1} class="p-1 text-slate-500 hover:text-red-400 disabled:opacity-30"><X size={13} /></button>
          </div>
        {/each}
      </div>
      <button on:click={addFolder} class="mt-3 self-start flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs"><Plus size={12} /> {$t('music.add_folder')}</button>
    </div>
  </div>
{/if}
