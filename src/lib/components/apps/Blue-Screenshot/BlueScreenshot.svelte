<script lang="ts">
  import { Camera, Download, Copy, Trash2, Timer, Monitor, Crop, RefreshCw, Check, Square } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';

  type CaptureMode = 'fullscreen' | 'window' | 'region';
  type DelayOption = 0 | 3 | 5 | 10;

  interface Screenshot {
    id: string; dataUrl: string; savedPath: string | null; timestamp: Date;
    mode: CaptureMode; width: number; height: number;
  }

  let mode: CaptureMode = 'fullscreen';
  let delay: DelayOption = 0;
  let screenshots: Screenshot[] = [];
  let capturing = false;
  let countdown = 0;
  let selected: Screenshot | null = null;
  let copied = false;
  let savePath = '~/Pictures/Screenshots';
  let canvasEl: HTMLCanvasElement;

  async function capture() {
    capturing = true;

    if (delay > 0) {
      countdown = delay;
      for (let i = delay; i > 0; i--) {
        await new Promise((r) => setTimeout(r, 1000));
        countdown = i - 1;
      }
    }

    try {
      let dataUrl = '';
      let savedPath = '';

      if (SystemBridge.isTauri()) {
        if (mode === 'region') {
          const geomResult = await SystemBridge.executeCommand('slurp 2>/dev/null');
          const geomRaw = typeof geomResult === 'string' ? geomResult : (geomResult as any)?.stdout ?? '';
          const geom = (geomRaw as string).trim();
          if (!geom) throw new Error('Selection cancelled');

          const ts = new Date().toISOString().replace(/[:.]/g, '-');
          const outPath = `${await SystemBridge.getHomePath()}/Pictures/Screenshots/screenshot-${ts}.png`;
          await SystemBridge.executeCommand(`mkdir -p "$(dirname '${outPath}')" && grim -g "${geom}" "${outPath}" 2>/dev/null || import -geometry "${geom}" "${outPath}" 2>/dev/null`);
          dataUrl = await SystemBridge.readFileAsDataURL(outPath);
          savedPath = outPath;
        } else if (mode === 'window') {
          const ts = new Date().toISOString().replace(/[:.]/g, '-');
          const outPath = `${await SystemBridge.getHomePath()}/Pictures/Screenshots/screenshot-${ts}.png`;
          await SystemBridge.executeCommand(
            `mkdir -p "$(dirname '${outPath}')" && grim -g "$(swaymsg -t get_tree 2>/dev/null | python3 -c "import json,sys; t=json.load(sys.stdin); [print(f\\"{n['rect']['x']},{n['rect']['y']} {n['rect']['width']}x{n['rect']['height']}\\") for n in [t] if n.get('focused')]" 2>/dev/null || slurp)" "${outPath}" 2>/dev/null || import -window root "${outPath}" 2>/dev/null`
          );
          dataUrl = await SystemBridge.readFileAsDataURL(outPath);
          savedPath = outPath;
        } else {
          savedPath = (await SystemBridge.takeScreenshot()) || '';
          if (!savedPath) throw new Error('No screenshot tool found (grim, scrot, or spectacle required)');
          dataUrl = await SystemBridge.readFileAsDataURL(savedPath);
        }
      } else {
        const stream = await (navigator.mediaDevices as any).getDisplayMedia({ video: true });
        const video = document.createElement('video');
        video.srcObject = stream;
        await new Promise<void>((r) => { video.onloadedmetadata = () => { video.play(); r(); }; });
        canvasEl.width = video.videoWidth;
        canvasEl.height = video.videoHeight;
        canvasEl.getContext('2d')!.drawImage(video, 0, 0);
        dataUrl = canvasEl.toDataURL('image/png');
        stream.getTracks().forEach((t: MediaStreamTrack) => t.stop());
      }

      if (!dataUrl) throw new Error('Empty screenshot data');

      const img = new Image();
      img.src = dataUrl;
      await new Promise((r) => { img.onload = r; });

      const ss: Screenshot = {
        id: Date.now().toString(), dataUrl, savedPath: savedPath || null, timestamp: new Date(),
        mode, width: img.naturalWidth, height: img.naturalHeight,
      };

      screenshots = [ss, ...screenshots];
      selected = ss;
    } catch (e) {
      console.error('Screenshot error:', e);
    } finally {
      capturing = false;
      countdown = 0;
    }
  }

  async function saveToFile(ss: Screenshot) {
    const name = `screenshot-${ss.timestamp.toISOString().slice(0, 19).replace(/:/g, '-')}.png`;
    if (SystemBridge.isTauri()) {
      await SystemBridge.saveFile(`${savePath.replace('~', '')}/${name}`, ss.dataUrl);
    } else {
      const a = document.createElement('a');
      a.href = ss.dataUrl; a.download = name; a.click();
    }
  }

  async function copyToClipboard(ss: Screenshot) {
    await SystemBridge.writeClipboardImage(ss.dataUrl);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }

  function deleteScreenshot(id: string) {
    screenshots = screenshots.filter((s) => s.id !== id);
    if (selected?.id === id) selected = null;
  }

  const MODES: { id: CaptureMode; label: string }[] = [
    { id: 'fullscreen', label: 'Full Screen' },
    { id: 'window', label: 'Window' },
    { id: 'region', label: 'Region' },
  ];
  const DELAYS: DelayOption[] = [0, 3, 5, 10];
</script>

<div class="flex h-full bg-slate-900 text-white overflow-hidden">
  <canvas bind:this={canvasEl} class="hidden" />

  <div class="w-64 bg-slate-800/50 border-r border-white/5 flex flex-col">
    <div class="p-4 border-b border-white/5">
      <div class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3">Capture Mode</div>
      <div class="space-y-1">
        {#each MODES as m (m.id)}
          <button on:click={() => (mode = m.id)} class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors {mode === m.id ? 'bg-blue-600 text-white' : 'text-slate-300 hover:bg-white/5'}">
            {#if m.id === 'fullscreen'}<Monitor size={16} />{:else if m.id === 'window'}<Square size={16} />{:else}<Crop size={16} />{/if}
            {m.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="p-4 border-b border-white/5">
      <div class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3 flex items-center gap-1.5"><Timer size={12} /> Delay</div>
      <div class="flex gap-2">
        {#each DELAYS as d (d)}
          <button on:click={() => (delay = d)} class="flex-1 py-1.5 rounded-lg text-xs font-medium transition-colors {delay === d ? 'bg-blue-600 text-white' : 'bg-slate-700 text-slate-300 hover:bg-slate-600'}">
            {d === 0 ? 'Now' : `${d}s`}
          </button>
        {/each}
      </div>
    </div>

    <div class="p-4 border-b border-white/5">
      <div class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Save to</div>
      <div class="flex gap-2">
        <input type="text" bind:value={savePath} class="flex-1 bg-slate-700 border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white focus:outline-none min-w-0" />
        <button on:click={async () => { const p = await SystemBridge.pickDirectory(); if (p) savePath = p; }} class="p-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-slate-300"><RefreshCw size={13} /></button>
      </div>
    </div>

    <div class="p-4">
      <button on:click={capture} disabled={capturing} class="w-full py-3 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-xl font-semibold text-sm flex items-center justify-center gap-2 transition-colors">
        {#if capturing}
          {#if countdown > 0}<Timer size={16} /> {countdown}s{:else}<RefreshCw size={16} class="animate-spin" /> Capturing…{/if}
        {:else}
          <Camera size={16} /> Take Screenshot
        {/if}
      </button>
    </div>

    <div class="flex-1 overflow-y-auto p-2">
      {#if screenshots.length === 0}
        <div class="text-center text-slate-600 text-xs py-8">No screenshots yet</div>
      {:else}
        <div class="space-y-2">
          {#each screenshots as ss (ss.id)}
            <div on:click={() => (selected = ss)} class="rounded-xl overflow-hidden cursor-pointer border-2 transition-colors {selected?.id === ss.id ? 'border-blue-500' : 'border-transparent hover:border-white/20'}">
              <img src={ss.dataUrl} alt="" class="w-full h-24 object-cover" />
              <div class="px-2 py-1 bg-slate-800 flex items-center justify-between">
                <span class="text-[10px] text-slate-400">{ss.width}×{ss.height} · {ss.timestamp.toLocaleTimeString()}</span>
                <button on:click={(e) => { e.stopPropagation(); deleteScreenshot(ss.id); }} class="p-0.5 hover:text-red-400 text-slate-500"><Trash2 size={11} /></button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="flex-1 flex flex-col overflow-hidden">
    {#if selected}
      {@const sel = selected}
      <div class="shrink-0 flex items-center justify-between px-4 py-3 border-b border-white/5 bg-slate-800/30">
        <div class="text-sm text-slate-300">{sel.width}×{sel.height} · {sel.mode} · {sel.timestamp.toLocaleString()}</div>
        <div class="flex gap-2">
          <button on:click={() => copyToClipboard(sel)} class="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm">
            {#if copied}<Check size={14} class="text-green-400" /> Copied!{:else}<Copy size={14} /> Copy{/if}
          </button>
          <button on:click={() => saveToFile(sel)} class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm"><Download size={14} /> Save</button>
          <button on:click={() => deleteScreenshot(sel.id)} class="p-1.5 hover:bg-red-500/20 rounded-lg text-red-400"><Trash2 size={16} /></button>
        </div>
      </div>
      <div class="flex-1 overflow-auto flex items-center justify-center p-6 bg-slate-950/50">
        <img src={selected.dataUrl} alt="Screenshot" class="max-w-full max-h-full object-contain rounded-lg shadow-2xl" />
      </div>
    {:else}
      <div class="flex-1 flex flex-col items-center justify-center text-slate-600 gap-4">
        <Camera size={56} class="opacity-20" />
        <div class="text-center">
          <div class="text-lg font-medium text-slate-400 mb-1">Blue Screenshot</div>
          <div class="text-sm">Choose a mode and click "Take Screenshot"</div>
        </div>
        <button on:click={capture} disabled={capturing} class="mt-2 px-6 py-2.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-xl text-sm text-white font-medium flex items-center gap-2">
          <Camera size={16} /> Take Screenshot
        </button>
      </div>
    {/if}
  </div>
</div>
