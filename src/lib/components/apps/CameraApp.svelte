<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import {
    Camera, Video, Image, FlipHorizontal, Settings, Circle, Square,
    RefreshCw, AlertTriangle, SwitchCamera, Grid3x3, X, Info,
  } from 'lucide-svelte';

  type Mode = 'photo' | 'video';
  type TimerSecs = 0 | 3 | 5 | 10;
  interface NativeDevice { path: string; name: string; }
  interface Capture { url: string; type: 'photo' | 'video'; name: string; }

  let videoEl: HTMLVideoElement;
  let canvasEl: HTMLCanvasElement;
  let mediaRecorder: MediaRecorder | null = null;
  let chunks: Blob[] = [];
  let stream: MediaStream | null = null;
  let recordTimer: ReturnType<typeof setInterval>;
  let pollTimer: ReturnType<typeof setInterval>;

  let mode: Mode = 'photo';
  let recording = false;
  let recordTime = 0;
  let captures: Capture[] = [];
  let mirrored = false;
  let showGrid = false;
  let timer: TimerSecs = 0;
  let countdown = 0;
  let error: string | null = null;
  let devices: MediaDeviceInfo[] = [];
  let deviceIdx = 0;
  let resolution = { w: 1280, h: 720 };
  let showSettings = false;
  let flashAnim = false;

  // Native fallback (webview lacks getUserMedia support) — see CameraApp/mod.rs
  let nativeMode = false;
  let nativeDevices: NativeDevice[] = [];
  let nativeFrame: string | null = null;
  let ffmpegMissing = false;
  let busy = false;

  function stopNativePolling() { clearInterval(pollTimer); }

  async function startNativeCamera(devIdx = deviceIdx) {
    error = null;
    nativeMode = true;
    stopNativePolling();

    const available = await SystemBridge.cameraCheckAvailable();
    if (!available) { ffmpegMissing = true; return; }
    ffmpegMissing = false;

    const list = await SystemBridge.cameraListDevices();
    nativeDevices = list;
    if (list.length === 0) { error = 'No camera device found under /dev/video*.'; return; }
    const device = list[devIdx % list.length].path;

    const poll = async () => {
      try { nativeFrame = await SystemBridge.cameraCaptureFrame(device, resolution.w, resolution.h); } catch {}
    };
    poll();
    pollTimer = setInterval(poll, 600);
  }

  async function startCamera(devIdx = deviceIdx) {
    stream?.getTracks().forEach((t) => t.stop());
    error = null;
    try {
      const allDevices = await navigator.mediaDevices.enumerateDevices();
      const cams = allDevices.filter((d) => d.kind === 'videoinput');
      devices = cams;
      const constraints: MediaStreamConstraints = {
        video: { deviceId: cams[devIdx]?.deviceId ? { ideal: cams[devIdx].deviceId } : undefined, width: { ideal: resolution.w }, height: { ideal: resolution.h } },
        audio: mode === 'video',
      };
      const s = await navigator.mediaDevices.getUserMedia(constraints);
      stream = s;
      nativeMode = false;
      stopNativePolling();
      if (videoEl) { videoEl.srcObject = s; await videoEl.play(); }
    } catch {
      await startNativeCamera(devIdx);
    }
  }

  onMount(() => {
    startCamera();
  });
  onDestroy(() => {
    stream?.getTracks().forEach((t) => t.stop());
    clearInterval(recordTimer);
    stopNativePolling();
  });

  async function doCapture() {
    flashAnim = true;
    setTimeout(() => (flashAnim = false), 300);

    if (nativeMode) {
      const device = nativeDevices[deviceIdx % Math.max(nativeDevices.length, 1)]?.path;
      if (!device) return;
      busy = true;
      try {
        const path = await SystemBridge.cameraCapturePhoto(device, resolution.w, resolution.h);
        const name = path.split('/').pop() || 'photo.jpg';
        captures = [{ url: `file://${path}`, type: 'photo', name }, ...captures];
      } catch (e: any) {
        error = `Capture failed: ${e?.message || e}`;
      } finally { busy = false; }
      return;
    }

    const canvas = canvasEl;
    const video = videoEl;
    if (!canvas || !video) return;
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    const ctx = canvas.getContext('2d')!;
    if (mirrored) { ctx.translate(canvas.width, 0); ctx.scale(-1, 1); }
    ctx.drawImage(video, 0, 0);

    canvas.toBlob((blob) => {
      if (!blob) return;
      const url = URL.createObjectURL(blob);
      const name = `photo-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.jpg`;
      captures = [{ url, type: 'photo', name }, ...captures];
      const a = document.createElement('a');
      a.href = url; a.download = name; a.click();
    }, 'image/jpeg', 0.92);
  }

  function takePhoto() {
    if (timer === 0) { doCapture(); return; }
    let count = timer;
    countdown = count;
    const id = setInterval(() => {
      count--;
      countdown = count;
      if (count <= 0) { clearInterval(id); countdown = 0; doCapture(); }
    }, 1000);
  }

  async function startNativeRecording() {
    const device = nativeDevices[deviceIdx % Math.max(nativeDevices.length, 1)]?.path;
    if (!device) return;
    const duration = timer || 5;
    recording = true;
    recordTime = 0;
    recordTimer = setInterval(() => (recordTime += 1), 1000);
    try {
      const path = await SystemBridge.cameraRecordVideo(device, resolution.w, resolution.h, duration);
      const name = path.split('/').pop() || 'video.mp4';
      captures = [{ url: `file://${path}`, type: 'video', name }, ...captures];
    } catch (e: any) {
      error = `Recording failed: ${e?.message || e}`;
    } finally {
      clearInterval(recordTimer);
      recording = false;
      recordTime = 0;
    }
  }

  async function startRecording() {
    if (nativeMode) { startNativeRecording(); return; }
    if (!stream) return;
    if (!stream.getAudioTracks().length) {
      await startCamera(deviceIdx);
      await new Promise((r) => setTimeout(r, 500));
    }
    const s = stream;
    if (!s) return;
    const mime = MediaRecorder.isTypeSupported('video/webm;codecs=vp9') ? 'video/webm;codecs=vp9' : 'video/webm';
    const mr = new MediaRecorder(s, { mimeType: mime });
    chunks = [];
    mr.ondataavailable = (e) => { if (e.data.size > 0) chunks.push(e.data); };
    mr.onstop = () => {
      const blob = new Blob(chunks, { type: mime });
      const url = URL.createObjectURL(blob);
      const name = `video-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.webm`;
      captures = [{ url, type: 'video', name }, ...captures];
      const a = document.createElement('a');
      a.href = url; a.download = name; a.click();
    };
    mr.start(250);
    mediaRecorder = mr;
    recording = true;
    recordTime = 0;
    recordTimer = setInterval(() => (recordTime += 1), 1000);
  }

  function stopRecording() {
    mediaRecorder?.stop();
    clearInterval(recordTimer);
    recording = false;
    recordTime = 0;
  }

  function switchCamera() {
    const count = nativeMode ? nativeDevices.length : devices.length;
    const next = (deviceIdx + 1) % Math.max(count, 1);
    deviceIdx = next;
    if (nativeMode) startNativeCamera(next); else startCamera(next);
  }

  function fmtTime(s: number) { return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`; }

  $: deviceCount = nativeMode ? nativeDevices.length : devices.length;
  $: timerOptions = nativeMode && mode === 'video' ? ([3, 5, 10] as TimerSecs[]) : ([0, 3, 5, 10] as TimerSecs[]);

  function handleDeviceChange(e: Event) {
    const v = +(e.currentTarget as HTMLSelectElement).value;
    deviceIdx = v;
    if (nativeMode) startNativeCamera(v); else startCamera(v);
  }

  function handleResolutionChange(e: Event) {
    const [w, h] = (e.currentTarget as HTMLSelectElement).value.split('x').map(Number);
    resolution = { w, h };
    if (nativeMode) startNativeCamera(); else startCamera();
  }
</script>

<div class="flex flex-col h-full bg-black text-white overflow-hidden">
  <div class="flex-1 relative overflow-hidden bg-black">
    {#if ffmpegMissing}
      <div class="absolute inset-0 flex flex-col items-center justify-center text-center px-8">
        <AlertTriangle size={40} class="text-amber-500 mb-4" />
        <p class="text-slate-200 text-sm mb-1 font-medium">ffmpeg not found</p>
        <p class="text-slate-400 text-xs max-w-xs">
          The browser camera API isn't available in this webview, so Blue Camera captures frames via ffmpeg + V4L2 instead.
          Install ffmpeg to use the camera (e.g. <code class="bg-slate-800 px-1 rounded">sudo dnf install ffmpeg</code>).
        </p>
        <button on:click={() => startNativeCamera()} class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm mt-4">Retry</button>
      </div>
    {:else if error}
      <div class="absolute inset-0 flex flex-col items-center justify-center text-center px-8">
        <Camera size={48} class="text-slate-600 mb-4" />
        <p class="text-slate-300 text-sm mb-2">{error}</p>
        <button on:click={() => startCamera()} class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm mt-2">Retry</button>
      </div>
    {:else}
      {#if nativeMode}
        {#if nativeFrame}
          <img src={nativeFrame} class="w-full h-full object-contain" style="transform:{mirrored ? 'scaleX(-1)' : 'none'};" alt="Camera preview" />
        {:else}
          <div class="absolute inset-0 flex items-center justify-center"><RefreshCw size={28} class="animate-spin text-slate-500" /></div>
        {/if}
      {:else}
        <video bind:this={videoEl} class="w-full h-full object-contain" style="transform:{mirrored ? 'scaleX(-1)' : 'none'};" playsinline muted autoplay>
          <track kind="captions" />
        </video>
      {/if}

      {#if nativeMode}
        <div class="absolute bottom-3 left-3 flex items-center gap-1.5 bg-black/60 backdrop-blur-sm rounded-full px-3 py-1 text-[11px] text-slate-300">
          <Info size={11} /> Native capture mode (ffmpeg / V4L2)
        </div>
      {/if}

      {#if showGrid}
        <div class="absolute inset-0 pointer-events-none">
          {#each [1, 2] as i (i)}<div class="absolute top-0 bottom-0 border-l border-white/20" style="left:{i * 33.33}%;" />{/each}
          {#each [1, 2] as i (i)}<div class="absolute left-0 right-0 border-t border-white/20" style="top:{i * 33.33}%;" />{/each}
        </div>
      {/if}

      {#if flashAnim}<div class="absolute inset-0 bg-white pointer-events-none" style="animation:flash 0.3s ease-out;" />{/if}

      {#if countdown > 0}
        <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
          <span class="text-8xl font-bold text-white drop-shadow-2xl" style="text-shadow:0 0 30px rgba(0,0,0,0.8);">{countdown}</span>
        </div>
      {/if}

      {#if recording}
        <div class="absolute top-4 left-4 flex items-center gap-2 bg-black/60 backdrop-blur-sm rounded-full px-3 py-1.5">
          <div class="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
          <span class="text-sm font-mono text-white">{fmtTime(recordTime)}{nativeMode ? ` / ${fmtTime(timer || 5)}` : ''}</span>
        </div>
      {/if}

      <div class="absolute top-3 right-3 flex gap-2">
        <button on:click={() => (mirrored = !mirrored)} class="p-2 rounded-full backdrop-blur-sm {mirrored ? 'bg-blue-600/80' : 'bg-black/40 hover:bg-black/60'}"><FlipHorizontal size={16} /></button>
        <button on:click={() => (showGrid = !showGrid)} class="p-2 rounded-full backdrop-blur-sm {showGrid ? 'bg-blue-600/80' : 'bg-black/40 hover:bg-black/60'}"><Grid3x3 size={16} /></button>
        {#if deviceCount > 1}
          <button on:click={switchCamera} class="p-2 rounded-full bg-black/40 hover:bg-black/60 backdrop-blur-sm"><SwitchCamera size={16} /></button>
        {/if}
        <button on:click={() => (showSettings = !showSettings)} class="p-2 rounded-full backdrop-blur-sm {showSettings ? 'bg-blue-600/80' : 'bg-black/40 hover:bg-black/60'}"><Settings size={16} /></button>
      </div>

      {#if showSettings}
        <div class="absolute top-14 right-3 bg-slate-900/95 backdrop-blur-sm rounded-xl border border-white/10 p-4 w-56 space-y-3">
          <div>
            <label class="text-xs text-slate-400 block mb-1">Camera</label>
            <select value={deviceIdx} on:change={handleDeviceChange} class="w-full bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white focus:outline-none">
              {#if nativeMode}
                {#each nativeDevices as d, i (d.path)}<option value={i}>{d.name}</option>{/each}
              {:else}
                {#each devices as d, i (d.deviceId)}<option value={i}>{d.label || `Camera ${i + 1}`}</option>{/each}
              {/if}
              {#if deviceCount === 0}<option value={0}>Default Camera</option>{/if}
            </select>
          </div>
          <div>
            <label class="text-xs text-slate-400 block mb-1">Resolution</label>
            <select value="{resolution.w}x{resolution.h}" on:change={handleResolutionChange} class="w-full bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white focus:outline-none">
              <option value="1920x1080">FHD (1920x1080)</option>
              <option value="1280x720">HD (1280x720)</option>
              <option value="640x480">SD (640x480)</option>
            </select>
          </div>
          <button on:click={() => (showSettings = false)} class="w-full text-center text-xs text-slate-500 hover:text-white pt-1">Close</button>
        </div>
      {/if}
    {/if}
  </div>

  <div class="shrink-0 bg-slate-900 border-t border-white/5">
    <div class="flex items-center justify-between px-4 pt-3 pb-1">
      <div class="flex bg-slate-800 rounded-xl p-0.5">
        <button on:click={() => (mode = 'photo')} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors {mode === 'photo' ? 'bg-white text-black' : 'text-slate-400'}"><Camera size={13} /> Photo</button>
        <button on:click={() => (mode = 'video')} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors {mode === 'video' ? 'bg-white text-black' : 'text-slate-400'}"><Video size={13} /> Video</button>
      </div>
      {#if mode === 'photo' || nativeMode}
        <div class="flex items-center gap-1">
          {#each timerOptions as t (t)}
            <button on:click={() => (timer = t)} class="w-8 h-8 rounded-lg text-xs font-medium transition-colors {timer === t ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'}">
              {#if t === 0}<X size={12} class="mx-auto" />{:else}{t}s{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="flex items-center justify-center gap-8 px-4 py-4 relative">
      <div class="w-14 h-14 rounded-xl overflow-hidden bg-slate-800 border border-white/10 shrink-0">
        {#if captures.length > 0}
          {#if captures[0].type === 'photo'}
            <img src={captures[0].url} class="w-full h-full object-cover" alt="" />
          {:else}
            <video src={captures[0].url} class="w-full h-full object-cover">
              <track kind="captions" />
            </video>
          {/if}
        {:else}
          <div class="w-full h-full flex items-center justify-center"><Image size={20} class="text-slate-600" /></div>
        {/if}
      </div>

      <div class="relative">
        {#if mode === 'photo'}
          <button on:click={takePhoto} disabled={countdown > 0 || busy}
            class="w-16 h-16 rounded-full bg-white hover:bg-slate-200 active:scale-95 transition-all disabled:opacity-50 flex items-center justify-center shadow-lg shadow-white/20">
            <div class="w-12 h-12 rounded-full border-2 border-slate-300" />
          </button>
        {:else}
          <button on:click={recording ? stopRecording : startRecording} disabled={nativeMode && recording}
            class="w-16 h-16 rounded-full flex items-center justify-center transition-all active:scale-95 shadow-lg disabled:opacity-60 {recording ? 'bg-red-500 hover:bg-red-400 shadow-red-500/30' : 'bg-red-600 hover:bg-red-500 shadow-red-600/30'}">
            {#if recording}<Square size={20} fill="white" />{:else}<Circle size={20} fill="white" />{/if}
          </button>
        {/if}
      </div>

      <div class="w-14 h-14 flex items-center justify-center text-slate-500 text-xs text-center shrink-0">
        {#if captures.length > 0}<span class="font-medium text-slate-300">{captures.length}<br /><span class="text-slate-600 font-normal">saved</span></span>{/if}
      </div>
    </div>
  </div>

  <canvas bind:this={canvasEl} class="hidden" />
</div>

<style>
  @keyframes flash { 0% { opacity: 0.8; } 100% { opacity: 0; } }
</style>
