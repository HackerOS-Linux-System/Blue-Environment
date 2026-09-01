<script lang="ts">
  // Blue Music — was registered in `constants.ts` (AppId.BLUE_MUSIC
  // exists) but marked `isExternal: true` with no `component` at all —
  // no actual implementation ever existed. `StartMenu.svelte`'s
  // `internalApps` list explicitly filters out anything with
  // `isExternal` (`if (app.isExternal || !app.component) return
  // false`), and `systemApps` only ever contains real `.desktop` files
  // discovered on the host — since no "blue-music" system app exists
  // either, Blue Music appeared in *neither* list. That's the actual
  // bug: not a filtering mistake, a missing app. This is a real,
  // minimal local-library player (scans a folder for audio files,
  // play/pause/seek/queue via a plain HTML5 `<audio>` element — the
  // same `asset://` URL scheme `toAssetUrl` already uses for wallpaper
  // previews elsewhere in this codebase), following the same "internal
  // Svelte app with its own UI" pattern as Blue Play (which also
  // launches/manages external processes but still has a real
  // component) rather than the broken external-app-with-no-component
  // shape.
  import { onMount, onDestroy } from 'svelte';
  import { Play, Pause, SkipBack, SkipForward, Music, FolderOpen, Repeat, Shuffle, Volume2, VolumeX, ListMusic } from 'lucide-svelte';
  import { SystemBridge, toAssetUrl } from '../../../utils/systemBridge';
  import { t } from '../../../stores/language';

  export let windowId: string;

  interface Track {
    name: string;
    path: string;
    mimeType: string;
  }

  let tracks: Track[] = [];
  let loading = true;
  let currentIndex = -1;
  let isPlaying = false;
  let currentTime = 0;
  let duration = 0;
  let volume = 0.8;
  let muted = false;
  let repeat = false;
  let shuffle = false;
  let audioEl: HTMLAudioElement;
  let musicDir = '';

  $: currentTrack = currentIndex >= 0 ? tracks[currentIndex] : null;

  async function scanLibrary() {
    loading = true;
    const home = await SystemBridge.getHomePath();
    // ~/Music is the conventional XDG user directory for audio — same
    // convention Blue Docs/Explorer already assume for their own
    // default locations.
    musicDir = `${home}/Music`;
    const entries = await SystemBridge.getFiles(musicDir);
    tracks = (entries ?? [])
      .filter((e: any) => !e.is_dir && typeof e.mime_type === 'string' && e.mime_type.startsWith('audio/'))
      .map((e: any) => ({ name: e.name, path: e.path, mimeType: e.mime_type }))
      .sort((a: Track, b: Track) => a.name.localeCompare(b.name));
    loading = false;
  }

  function playTrack(index: number) {
    if (index < 0 || index >= tracks.length) return;
    currentIndex = index;
    // `audioEl.src` is set reactively below (bind via {#key} on the
    // `<audio>` element's src attribute) — explicitly call `.play()`
    // after the src change has had a chance to take, on the next tick.
    requestAnimationFrame(() => audioEl?.play());
  }

  function togglePlay() {
    if (!currentTrack) {
      if (tracks.length > 0) playTrack(0);
      return;
    }
    if (isPlaying) audioEl.pause();
    else audioEl.play();
  }

  function next() {
    if (tracks.length === 0) return;
    if (shuffle) {
      playTrack(Math.floor(Math.random() * tracks.length));
      return;
    }
    playTrack((currentIndex + 1) % tracks.length);
  }

  function prev() {
    if (tracks.length === 0) return;
    if (currentTime > 3) {
      // Scrubbing back to the start of the current track is what
      // "previous" means once you're a few seconds in — matches every
      // real media player's convention, not a Blue-specific choice.
      audioEl.currentTime = 0;
      return;
    }
    playTrack((currentIndex - 1 + tracks.length) % tracks.length);
  }

  function handleEnded() {
    if (repeat) {
      audioEl.currentTime = 0;
      audioEl.play();
      return;
    }
    if (currentIndex < tracks.length - 1 || shuffle) next();
    else isPlaying = false;
  }

  function seek(e: Event) {
    const target = e.currentTarget as HTMLInputElement;
    audioEl.currentTime = Number(target.value);
  }

  function fmtTime(s: number): string {
    if (!isFinite(s) || s < 0) return '0:00';
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60).toString().padStart(2, '0');
    return `${m}:${sec}`;
  }

  $: if (audioEl) audioEl.volume = muted ? 0 : volume;

  onMount(scanLibrary);
  onDestroy(() => audioEl?.pause());
</script>

<div class="flex flex-col h-full bg-slate-900 text-white text-sm">
  <div class="flex items-center justify-between px-4 h-11 border-b border-white/5 shrink-0">
    <div class="flex items-center gap-2">
      <Music size={16} class="text-blue-400" />
      <span class="font-semibold">{$t('music.title')}</span>
    </div>
    <button on:click={scanLibrary} title={$t('music.rescan')} class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400">
      <FolderOpen size={14} />
    </button>
  </div>

  <div class="flex-1 overflow-y-auto">
    {#if loading}
      <div class="flex items-center justify-center h-full text-slate-500 text-xs">{$t('music.loading')}</div>
    {:else if tracks.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-slate-500 gap-2 px-8 text-center">
        <ListMusic size={28} class="opacity-30" />
        <span class="text-xs">{$t('music.empty')}</span>
        <span class="text-[10px] text-slate-600 font-mono">{musicDir}</span>
      </div>
    {:else}
      {#each tracks as track, i (track.path)}
        <button on:click={() => playTrack(i)}
          class="w-full flex items-center gap-3 px-4 py-2.5 hover:bg-white/5 text-left transition-colors {i === currentIndex ? 'bg-blue-500/10' : ''}">
          <div class="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 {i === currentIndex ? 'bg-blue-500/20' : 'bg-slate-800'}">
            {#if i === currentIndex && isPlaying}
              <div class="flex items-end gap-0.5 h-3">
                <span class="w-0.5 bg-blue-400 animate-pulse" style="height:60%" />
                <span class="w-0.5 bg-blue-400 animate-pulse" style="height:100%; animation-delay:150ms" />
                <span class="w-0.5 bg-blue-400 animate-pulse" style="height:40%; animation-delay:300ms" />
              </div>
            {:else}
              <Music size={13} class="text-slate-500" />
            {/if}
          </div>
          <span class="flex-1 min-w-0 truncate {i === currentIndex ? 'text-blue-300' : 'text-slate-200'}">{track.name}</span>
        </button>
      {/each}
    {/if}
  </div>

  <div class="border-t border-white/5 px-4 py-3 shrink-0 flex flex-col gap-2">
    <div class="flex items-center gap-2 text-[10px] text-slate-500">
      <span class="w-9 text-right tabular-nums">{fmtTime(currentTime)}</span>
      <input type="range" min="0" max={duration || 0} value={currentTime} on:input={seek}
        disabled={!currentTrack} class="flex-1 accent-blue-500" />
      <span class="w-9 tabular-nums">{fmtTime(duration)}</span>
    </div>
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-1">
        <button on:click={() => (shuffle = !shuffle)} title={$t('music.shuffle')} class="p-1.5 rounded-lg hover:bg-white/10 {shuffle ? 'text-blue-400' : 'text-slate-500'}"><Shuffle size={14} /></button>
        <button on:click={() => (repeat = !repeat)} title={$t('music.repeat')} class="p-1.5 rounded-lg hover:bg-white/10 {repeat ? 'text-blue-400' : 'text-slate-500'}"><Repeat size={14} /></button>
      </div>
      <div class="flex items-center gap-1">
        <button on:click={prev} class="p-2 rounded-full hover:bg-white/10"><SkipBack size={16} /></button>
        <button on:click={togglePlay} class="p-3 rounded-full bg-blue-600 hover:bg-blue-500 mx-1">
          {#if isPlaying}<Pause size={16} />{:else}<Play size={16} />{/if}
        </button>
        <button on:click={next} class="p-2 rounded-full hover:bg-white/10"><SkipForward size={16} /></button>
      </div>
      <div class="flex items-center gap-1.5 w-20">
        <button on:click={() => (muted = !muted)} class="text-slate-500 hover:text-white shrink-0">
          {#if muted || volume === 0}<VolumeX size={14} />{:else}<Volume2 size={14} />{/if}
        </button>
        <input type="range" min="0" max="1" step="0.01" bind:value={volume} class="flex-1 accent-blue-500" />
      </div>
    </div>
    {#if currentTrack}
      <div class="text-center text-xs text-slate-400 truncate">{currentTrack.name}</div>
    {/if}
  </div>

  {#if currentTrack}
    <audio bind:this={audioEl} src={toAssetUrl(currentTrack.path)}
      on:play={() => (isPlaying = true)} on:pause={() => (isPlaying = false)}
      on:timeupdate={() => (currentTime = audioEl.currentTime)}
      on:durationchange={() => (duration = audioEl.duration)}
      on:ended={handleEnded} />
  {/if}
</div>
