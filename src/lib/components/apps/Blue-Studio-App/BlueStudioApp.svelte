<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { Plus, Scissors, Trash2, Play, Pause, Type, Music, Download, ArrowLeft, ArrowRight } from 'lucide-svelte';
  import { toAssetUrl, SystemBridge } from '../../../utils/systemBridge';
  import { pickPaths } from '../../../stores/filePicker';
  import {
    makeClip, splitAt, moveClip, clipLength, timelineLength, fmtTime, nextId, toExportRequest,
    type Media, type TimelineClip, type TextItem,
  } from './studioModel';

  let ffmpegOk = true;
  let media: Media[] = [];
  let clips: TimelineClip[] = [];
  let texts: TextItem[] = [];
  let music: { path: string; volume: number } | null = null;
  let selected: number | null = null;     // id klipu
  let playhead = 0;                       // s na osi wynikowej
  let playing = false;
  let video: HTMLVideoElement;
  let error = '';
  let tickTimer: ReturnType<typeof setInterval>;
  let unlisten: (() => void) | undefined;

  // eksport
  let showExport = false;
  let exportName = 'film.mp4';
  let exportDir = '';
  let preset = '1080p';
  let fps = 30;
  let crf = 23;
  let exporting = false;
  let progress = 0;
  const PRESETS: Record<string, [number, number]> = { '720p': [1280, 720], '1080p': [1920, 1080], '1440p': [2560, 1440], '4K': [3840, 2160], 'Pion 1080×1920': [1080, 1920] };

  $: total = timelineLength(clips);
  $: sel = clips.find((c) => c.id === selected) ?? null;
  const PX_PER_SEC = 60;

  onMount(async () => {
    ffmpegOk = await invoke<boolean>('studio_check').catch(() => false);
    unlisten = await listen<{ percent: number }>('studio:progress', (e) => (progress = e.payload.percent));
    tickTimer = setInterval(() => { if (playing) advance(0.05); }, 50);
  });
  onDestroy(() => { clearInterval(tickTimer); unlisten?.(); });

  async function addMedia() {
    const paths = await pickPaths({ mode: 'file', title: 'Dodaj multimedia', multiple: true,
      filters: [{ name: 'Wideo/Audio/Obraz', extensions: ['mp4', 'mkv', 'mov', 'webm', 'avi', 'mp3', 'wav', 'flac', 'png', 'jpg'] }] });
    for (const p of paths) {
      try {
        const info = await invoke<any>('studio_probe', { path: p });
        const m: Media = { path: p, name: p.split('/').pop() ?? p, duration: info.duration || 5, width: info.width, height: info.height, hasAudio: info.hasAudio, hasVideo: info.hasVideo };
        if (m.hasVideo) m.thumb = await invoke<string>('studio_thumbnail', { path: p, time: Math.min(1, m.duration / 2) }).catch(() => undefined);
        media = [...media, m];
      } catch (e) { error = `${p}: ${e}`; }
    }
  }
  async function pickMusic() {
    const p = (await pickPaths({ mode: 'file', title: 'Muzyka w tle', multiple: false, filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'flac', 'ogg', 'm4a'] }] }))[0];
    if (p) music = { path: p, volume: 0.4 };
  }
  const addToTimeline = (m: Media) => { if (m.hasVideo) { const c = makeClip(m); clips = [...clips, c]; selected = c.id; } };
  const split = () => { const r = splitAt(clips, playhead); if (r) clips = r; };
  const remove = () => { clips = clips.filter((c) => c.id !== selected); selected = null; };
  const move = (d: number) => { const i = clips.findIndex((c) => c.id === selected); clips = moveClip(clips, i, i + d); };
  const addText = () => { texts = [...texts, { id: nextId(), text: 'Tekst', start: playhead, end: Math.min(total, playhead + 3) || 3, x: 0.5, y: 0.85, size: 48, color: 'white' }]; };

  // Podgląd: znajdź klip pod playheadem i ustaw <video> na odpowiedni moment źródła.
  function locate(t: number) {
    let acc = 0;
    for (const c of clips) { const len = clipLength(c); if (t < acc + len) return { c, src: c.start + (t - acc) * c.speed }; acc += len; }
    return null;
  }
  function seek(t: number) {
    playhead = Math.max(0, Math.min(total, t));
    const loc = locate(playhead);
    if (loc && video) {
      const url = toAssetUrl(loc.c.media.path);
      if (video.dataset.src !== url) { video.src = url; video.dataset.src = url; }
      video.currentTime = loc.src;
      video.playbackRate = loc.c.speed;
      video.volume = Math.min(1, loc.c.volume);
    }
  }
  function advance(dt: number) {
    if (playhead + dt >= total) { playing = false; video?.pause(); return; }
    const before = locate(playhead)?.c.id;
    playhead += dt;
    const loc = locate(playhead);
    if (loc?.c.id !== before) seek(playhead);
  }
  function togglePlay() {
    if (!clips.length) return;
    if (playhead >= total) seek(0);
    playing = !playing;
    if (playing) { seek(playhead); video?.play().catch(() => {}); } else video?.pause();
  }
  $: activeTexts = texts.filter((t) => playhead >= t.start && playhead <= t.end);

  async function chooseDir() { exportDir = (await pickPaths({ mode: 'directory', title: 'Folder eksportu', multiple: false }))[0] ?? exportDir; }
  async function doExport() {
    error = ''; exporting = true; progress = 0;
    try {
      const dir = exportDir || (await SystemBridge.getHomePath().catch(() => '')) || '';
      const [width, height] = PRESETS[preset];
      const output = `${dir.replace(/\/$/, '')}/${exportName}`;
      await invoke('studio_export', { request: toExportRequest(clips, texts, music, { width, height, fps, crf, output }) });
      showExport = false;
      error = `Zapisano: ${output}`;
    } catch (e) { error = String(e); }
    exporting = false;
  }
</script>

<div class="h-full flex flex-col bg-slate-900 text-slate-200 text-sm select-none">
  {#if !ffmpegOk}
    <div class="px-3 py-2 bg-yellow-500/10 text-yellow-300 text-xs">Nie znaleziono ffmpeg/ffprobe — zainstaluj pakiet <b>ffmpeg</b>, aby edytować i eksportować.</div>
  {/if}
  {#if error}<div class="px-3 py-1.5 text-xs bg-slate-800 text-slate-300">{error}</div>{/if}

  <div class="flex flex-1 min-h-0">
    <!-- Biblioteka -->
    <div class="w-56 border-r border-white/5 flex flex-col">
      <div class="p-2 flex gap-1">
        <button class="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white" on:click={addMedia}><Plus size={14} /> Media</button>
        <button class="p-1.5 rounded-lg hover:bg-white/10" title="Muzyka w tle" on:click={pickMusic}><Music size={15} /></button>
      </div>
      <div class="flex-1 overflow-y-auto p-2 space-y-2">
        {#each media as m}
          <button class="w-full text-left bg-slate-800 rounded-lg overflow-hidden hover:ring-1 ring-blue-500" on:click={() => addToTimeline(m)} title="Dodaj do osi czasu">
            {#if m.thumb}<img src={m.thumb} alt="" class="w-full h-20 object-cover" />{:else}<div class="h-20 flex items-center justify-center text-slate-500"><Music size={20} /></div>{/if}
            <div class="px-2 py-1 text-xs truncate">{m.name} · {fmtTime(m.duration)}</div>
          </button>
        {:else}
          <div class="text-xs text-slate-500 text-center mt-6">Dodaj filmy, a potem kliknij miniaturę, aby wrzucić ją na oś czasu.</div>
        {/each}
        {#if music}<div class="text-xs bg-slate-800 rounded-lg p-2">♪ {music.path.split('/').pop()}
          <input type="range" min="0" max="1" step="0.05" bind:value={music.volume} class="w-full" /></div>{/if}
      </div>
    </div>

    <!-- Podgląd + właściwości -->
    <div class="flex-1 flex flex-col min-w-0">
      <div class="flex-1 bg-black relative flex items-center justify-center min-h-0">
        <!-- svelte-ignore a11y-media-has-caption -->
        <video bind:this={video} class="max-h-full max-w-full" muted={false} />
        {#each activeTexts as t (t.id)}
          <div class="absolute pointer-events-none font-bold" style="left:{t.x * 100}%; top:{t.y * 100}%; transform:translate(-50%,-50%); color:{t.color}; font-size:{t.size / 3}px; text-shadow:0 0 4px #000">{t.text}</div>
        {/each}
        {#if !clips.length}<div class="absolute text-slate-500">Podgląd</div>{/if}
      </div>
      <div class="flex items-center gap-2 px-3 py-2 border-t border-white/5">
        <button class="p-1.5 rounded-lg hover:bg-white/10" on:click={togglePlay}>{#if playing}<Pause size={16} />{:else}<Play size={16} />{/if}</button>
        <span class="tabular-nums text-xs">{fmtTime(playhead)} / {fmtTime(total)}</span>
        <input class="flex-1" type="range" min="0" max={total || 1} step="0.05" value={playhead} on:input={(e) => { playing = false; video?.pause(); seek(+e.currentTarget.value); }} />
        <button class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-white/10" on:click={split} title="Podziel w miejscu kursora"><Scissors size={14} /> Podziel</button>
        <button class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-white/10" on:click={addText}><Type size={14} /> Tekst</button>
        <button class="flex items-center gap-1 px-3 py-1 rounded-lg bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-40" disabled={!clips.length} on:click={() => (showExport = true)}><Download size={14} /> Eksportuj</button>
      </div>
    </div>

    <!-- Właściwości -->
    <div class="w-60 border-l border-white/5 p-3 space-y-3 overflow-y-auto">
      {#if sel}
        <div class="font-semibold text-white truncate">{sel.media.name}</div>
        <label class="block text-xs">Początek źródła: {fmtTime(sel.start)}<input type="range" class="w-full" min="0" max={Math.max(0, sel.media.duration - 0.2)} step="0.1" bind:value={sel.start} on:input={() => (clips = clips)} /></label>
        <label class="block text-xs">Długość: {fmtTime(sel.duration)}<input type="range" class="w-full" min="0.2" max={Math.max(0.2, sel.media.duration - sel.start)} step="0.1" bind:value={sel.duration} on:input={() => (clips = clips)} /></label>
        <label class="block text-xs">Tempo: {sel.speed.toFixed(2)}×<input type="range" class="w-full" min="0.25" max="4" step="0.25" bind:value={sel.speed} on:input={() => (clips = clips)} /></label>
        <label class="block text-xs">Głośność: {Math.round(sel.volume * 100)}%<input type="range" class="w-full" min="0" max="2" step="0.05" bind:value={sel.volume} on:input={() => (clips = clips)} /></label>
        <label class="block text-xs">Wejście (fade): {sel.fadeIn.toFixed(1)} s<input type="range" class="w-full" min="0" max="3" step="0.1" bind:value={sel.fadeIn} on:input={() => (clips = clips)} /></label>
        <label class="block text-xs">Wyjście (fade): {sel.fadeOut.toFixed(1)} s<input type="range" class="w-full" min="0" max="3" step="0.1" bind:value={sel.fadeOut} on:input={() => (clips = clips)} /></label>
        <div class="flex gap-1">
          <button class="p-1.5 rounded-lg hover:bg-white/10" on:click={() => move(-1)}><ArrowLeft size={14} /></button>
          <button class="p-1.5 rounded-lg hover:bg-white/10" on:click={() => move(1)}><ArrowRight size={14} /></button>
          <button class="p-1.5 rounded-lg hover:bg-red-500/20 text-red-400" on:click={remove}><Trash2 size={14} /></button>
        </div>
      {:else if texts.length}
        <div class="text-xs text-slate-400">Teksty</div>
        {#each texts as t (t.id)}
          <div class="bg-slate-800 rounded-lg p-2 space-y-1">
            <input class="w-full bg-slate-700 rounded px-2 py-1" bind:value={t.text} on:input={() => (texts = texts)} />
            <div class="flex gap-1 text-xs items-center">od <input type="number" step="0.5" class="w-14 bg-slate-700 rounded px-1" bind:value={t.start} /> do <input type="number" step="0.5" class="w-14 bg-slate-700 rounded px-1" bind:value={t.end} />
              <button class="ml-auto text-red-400" on:click={() => (texts = texts.filter((x) => x.id !== t.id))}><Trash2 size={13} /></button></div>
            <div class="flex gap-1 text-xs items-center">rozm. <input type="number" min="8" max="400" class="w-14 bg-slate-700 rounded px-1" bind:value={t.size} /> kolor <input type="color" class="w-8 h-5" value={t.color.startsWith('#') ? t.color : '#ffffff'} on:input={(e) => (t.color = e.currentTarget.value)} /></div>
          </div>
        {/each}
      {:else}<div class="text-xs text-slate-500">Zaznacz klip na osi czasu, aby edytować jego właściwości.</div>{/if}
    </div>
  </div>

  <!-- Oś czasu -->
  <div class="h-32 border-t border-white/5 overflow-x-auto overflow-y-hidden relative bg-slate-950/50">
    <div class="relative h-full" style="width:{Math.max(total * PX_PER_SEC + 80, 400)}px"
      on:click={(e) => { if (e.target === e.currentTarget) { selected = null; playing = false; seek((e.offsetX) / PX_PER_SEC); } }} role="presentation">
      <div class="flex absolute top-3 left-0 gap-0.5">
        {#each clips as c (c.id)}
          <button class="h-16 rounded-md overflow-hidden text-left text-[11px] px-1.5 py-1 border {selected === c.id ? 'border-blue-400 bg-blue-600/60' : 'border-white/10 bg-blue-900/50 hover:bg-blue-800/60'}"
            style="width:{Math.max(20, clipLength(c) * PX_PER_SEC)}px" on:click={() => (selected = c.id)}>
            <div class="truncate">{c.media.name}</div><div class="text-slate-400">{fmtTime(clipLength(c))}{c.speed !== 1 ? ` · ${c.speed}×` : ''}</div>
          </button>
        {/each}
      </div>
      {#each texts as t (t.id)}
        <div class="absolute h-5 top-[88px] rounded bg-yellow-600/60 text-[10px] px-1 truncate" style="left:{t.start * PX_PER_SEC}px; width:{Math.max(16, (t.end - t.start) * PX_PER_SEC)}px">{t.text}</div>
      {/each}
      <div class="absolute top-0 bottom-0 w-px bg-red-500 pointer-events-none" style="left:{playhead * PX_PER_SEC}px" />
    </div>
  </div>

  {#if showExport}
    <div class="absolute inset-0 bg-black/60 flex items-center justify-center z-10">
      <div class="bg-slate-800 rounded-2xl p-5 w-96 space-y-3 border border-white/10">
        <div class="text-white font-semibold">Eksport wideo (MP4 / H.264)</div>
        <label class="flex justify-between items-center">Rozdzielczość
          <select class="bg-slate-700 rounded px-2 py-1" bind:value={preset}>{#each Object.keys(PRESETS) as k}<option>{k}</option>{/each}</select></label>
        <label class="flex justify-between items-center">Klatki/s
          <select class="bg-slate-700 rounded px-2 py-1" bind:value={fps}><option>24</option><option>30</option><option>60</option></select></label>
        <label class="flex justify-between items-center">Jakość (CRF {crf})<input type="range" min="14" max="32" bind:value={crf} /></label>
        <input class="w-full bg-slate-700 rounded px-2 py-1" bind:value={exportName} />
        <button class="w-full text-left bg-slate-700 rounded px-2 py-1 truncate" on:click={chooseDir}>{exportDir || 'Folder: katalog domowy (kliknij, aby zmienić)'}</button>
        {#if exporting}<div class="h-2 bg-slate-700 rounded overflow-hidden"><div class="h-full bg-blue-500" style="width:{progress}%" /></div>{/if}
        <div class="flex justify-end gap-2">
          <button class="px-3 py-1.5 rounded-lg hover:bg-white/10" disabled={exporting} on:click={() => (showExport = false)}>Anuluj</button>
          <button class="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-40" disabled={exporting || !exportName.trim()} on:click={doExport}>{exporting ? `${Math.round(progress)}%` : 'Eksportuj'}</button>
        </div>
      </div>
    </div>
  {/if}
</div>
