<script lang="ts">
  import { Play, Pause, SkipBack, SkipForward, Volume2, VolumeX, Maximize2, Settings, List, FolderOpen, PictureInPicture2 } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';

  interface Props {
    playing: boolean;
    current: number;
    duration: number;
    volume: number;
    muted: boolean;
    speed: number;
    showPlaylist: boolean;
    showSettings: boolean;
    canPiP?: boolean;
  }

  let {
    playing,
    current,
    duration,
    volume,
    muted,
    speed,
    showPlaylist,
    showSettings,
    canPiP = false
  }: Props = $props();

  const dispatch = createEventDispatcher<{
    play: void; prev: void; next: void; seek: number; volume: number; mute: void;
    fullscreen: void; togglePlaylist: void; toggleSettings: void; openFiles: void; pip: void;
  }>();

  function fmtTime(s: number) {
    const m = Math.floor(s / 60), sec = Math.floor(s % 60);
    return `${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
  }
</script>

<div class="bg-slate-900/95 border-t border-white/5 p-2 shrink-0">
  <input type="range" min="0" max={duration || 1} value={current} step="0.1"
    oninput={(e) => dispatch('seek', parseFloat(e.currentTarget.value))}
    class="w-full h-1 mb-2 accent-blue-500 cursor-pointer" />
  <div class="flex items-center gap-2">
    <button onclick={() => dispatch('prev')} class="p-1.5 hover:bg-white/10 rounded"><SkipBack size={16} /></button>
    <button onclick={() => dispatch('play')} class="p-2 hover:bg-white/10 rounded">
      {#if playing}<Pause size={18} />{:else}<Play size={18} />{/if}
    </button>
    <button onclick={() => dispatch('next')} class="p-1.5 hover:bg-white/10 rounded"><SkipForward size={16} /></button>
    <span class="text-xs text-slate-400 font-mono mx-1">{fmtTime(current)} / {fmtTime(duration)}</span>
    <div class="flex-1"></div>
    <button onclick={() => dispatch('mute')} class="p-1.5 hover:bg-white/10 rounded">
      {#if muted}<VolumeX size={15} />{:else}<Volume2 size={15} />{/if}
    </button>
    <input type="range" min="0" max="1" step="0.05" value={muted ? 0 : volume}
      oninput={(e) => dispatch('volume', parseFloat(e.currentTarget.value))}
      class="w-20 accent-blue-500 cursor-pointer" />
    <span class="text-xs text-slate-500">{speed}x</span>
    <button onclick={() => dispatch('openFiles')} class="p-1.5 hover:bg-white/10 rounded"><FolderOpen size={15} /></button>
    {#if canPiP}
      <button onclick={() => dispatch('pip')} class="p-1.5 hover:bg-white/10 rounded" title="Picture-in-Picture — floats outside this window, even over other apps"><PictureInPicture2 size={15} /></button>
    {/if}
    <button onclick={() => dispatch('togglePlaylist')} class="p-1.5 rounded {showPlaylist ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}"><List size={15} /></button>
    <button onclick={() => dispatch('toggleSettings')} class="p-1.5 rounded {showSettings ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}"><Settings size={15} /></button>
    <button onclick={() => dispatch('fullscreen')} class="p-1.5 hover:bg-white/10 rounded"><Maximize2 size={15} /></button>
  </div>
</div>
