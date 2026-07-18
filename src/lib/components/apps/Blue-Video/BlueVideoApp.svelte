<script lang="ts">
  import { Video, X, Plus } from 'lucide-svelte';
  import { createPlaylist } from './playlist';
  import Controls from './Controls.svelte';
  import SubtitleTrack from './SubtitleTrack.svelte';

  const speeds = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2];
  const { playlist, currentIdx, openFiles, remove } = createPlaylist();

  let videoEl: HTMLVideoElement;
  let containerEl: HTMLDivElement;
  let playing = false;
  let current = 0;
  let duration = 0;
  let volume = 1;
  let muted = false;
  let speed = 1;
  let fullscreen = false;
  let showPlaylist = false;
  let showSettings = false;

  $: currentItem = $playlist[$currentIdx];

  $: if (videoEl && currentItem) {
    videoEl.src = currentItem.url;
    videoEl.play().catch(() => {});
  }

  function togglePlay() { if (!videoEl) return; playing ? videoEl.pause() : videoEl.play(); }
  function seek(e: CustomEvent<number>) { if (videoEl) videoEl.currentTime = e.detail; }
  function changeVolume(e: CustomEvent<number>) {
    if (!videoEl) return;
    videoEl.volume = e.detail; videoEl.muted = false; volume = e.detail;
  }
  function changeSpeed(s: number) { if (videoEl) videoEl.playbackRate = s; speed = s; showSettings = false; }
  async function toggleFullscreen() {
    if (!document.fullscreenElement) { await containerEl?.requestFullscreen(); fullscreen = true; }
    else { await document.exitFullscreen(); fullscreen = false; }
  }
  function prev() { if ($currentIdx > 0) currentIdx.set($currentIdx - 1); }
  function next() { if ($currentIdx < $playlist.length - 1) currentIdx.set($currentIdx + 1); }

  // True OS-level Picture-in-Picture (the browser floats the video in its
  // own always-on-top mini window, independent of this app's window — it
  // keeps playing even if Blue Video itself is minimized or closed-to-tray).
  // This is distinct from the generic window-level PiP available on every
  // app via the titlebar button (see Window.svelte) — that one keeps the
  // whole app window small and floating, this one floats just the video.
  $: canPiP = typeof document !== 'undefined' && 'pictureInPictureEnabled' in document;
  async function togglePiP() {
    if (!videoEl) return;
    try {
      if (document.pictureInPictureElement) await document.exitPictureInPicture();
      else await videoEl.requestPictureInPicture();
    } catch { /* not supported for this stream/codec — silently ignore */ }
  }
</script>

<div bind:this={containerEl} class="flex flex-col h-full bg-black text-white overflow-hidden relative">
  <div class="flex flex-1 overflow-hidden">
    <div class="flex-1 flex items-center justify-center bg-black relative">
      {#if !currentItem}
        <div class="flex flex-col items-center gap-4 text-slate-600">
          <Video size={48} />
          <p class="text-sm text-slate-400">No video open</p>
          <button on:click={openFiles} class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm text-white">Open video files</button>
        </div>
      {:else}
        <video
          bind:this={videoEl}
          class="max-w-full max-h-full"
          on:play={() => (playing = true)}
          on:pause={() => (playing = false)}
          on:timeupdate={() => (current = videoEl?.currentTime ?? 0)}
          on:loadedmetadata={() => (duration = videoEl?.duration ?? 0)}
          on:ended={() => { if ($currentIdx < $playlist.length - 1) currentIdx.set($currentIdx + 1); }}
        >
          <track kind="captions" />
        </video>
        <SubtitleTrack videoPath={currentItem?.url.replace('file://', '')} currentTime={current} />
      {/if}
    </div>

    {#if showPlaylist}
      <div class="w-56 bg-slate-900 border-l border-white/5 flex flex-col overflow-hidden shrink-0">
        <div class="flex items-center justify-between px-3 py-2 border-b border-white/5">
          <span class="text-xs font-medium">Playlist ({$playlist.length})</span>
          <button on:click={openFiles} class="text-blue-400 hover:text-blue-300"><Plus size={14} /></button>
        </div>
        <div class="flex-1 overflow-y-auto">
          {#each $playlist as item, i (i)}
            <div on:click={() => currentIdx.set(i)}
              class="flex items-center gap-2 px-3 py-2 cursor-pointer border-b border-white/5 group {i === $currentIdx ? 'bg-blue-600/20 text-white' : 'text-slate-400 hover:bg-white/5'}">
              <span class="flex-1 text-xs truncate">{item.name}</span>
              <button on:click={(e) => { e.stopPropagation(); remove(i); }} class="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400">
                <X size={11} />
              </button>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if showSettings}
      <div class="absolute bottom-20 right-4 bg-slate-800 rounded-xl p-3 border border-white/10 shadow-xl z-10 w-40">
        <div class="text-xs text-slate-400 mb-2">Playback Speed</div>
        {#each speeds as s (s)}
          <button on:click={() => changeSpeed(s)} class="w-full text-left px-2 py-1.5 rounded text-xs {speed === s ? 'bg-blue-600 text-white' : 'hover:bg-white/10 text-slate-300'}">
            {s}x
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <Controls
    {playing} {current} {duration} {volume} {muted} {speed} {showPlaylist} {showSettings} {canPiP}
    on:play={togglePlay}
    on:prev={prev}
    on:next={next}
    on:seek={seek}
    on:volume={changeVolume}
    on:mute={() => { if (videoEl) videoEl.muted = !videoEl.muted; muted = !muted; }}
    on:fullscreen={toggleFullscreen}
    on:togglePlaylist={() => (showPlaylist = !showPlaylist)}
    on:toggleSettings={() => { showSettings = !showSettings; if (showPlaylist) showPlaylist = false; }}
    on:openFiles={openFiles}
    on:pip={togglePiP}
  />
</div>
