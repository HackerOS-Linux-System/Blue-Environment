<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    FolderOpen, ZoomIn, ZoomOut, Maximize2, Minimize2,
    RotateCcw, RotateCw, FlipHorizontal, FlipVertical,
    ChevronLeft, ChevronRight, Download, Crop,
    Sliders, X, RefreshCw, Image as ImageIcon,
  } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';

  interface ImageItem { path: string; name: string; url: string; }

  interface Adjustments {
    brightness: number; contrast: number; saturation: number; hue: number;
    blur: number; sepia: number; grayscale: number; opacity: number;
  }

  const DEFAULT_ADJ: Adjustments = { brightness: 100, contrast: 100, saturation: 100, hue: 0, blur: 0, sepia: 0, grayscale: 0, opacity: 100 };

  function adjToFilter(a: Adjustments): string {
    return [
      `brightness(${a.brightness}%)`, `contrast(${a.contrast}%)`, `saturate(${a.saturation}%)`,
      `hue-rotate(${a.hue}deg)`, `blur(${a.blur}px)`, `sepia(${a.sepia}%)`, `grayscale(${a.grayscale}%)`, `opacity(${a.opacity}%)`,
    ].join(' ');
  }

  interface CropBox { x: number; y: number; w: number; h: number; }

  let images: ImageItem[] = [];
  let idx = 0;
  let zoom = 1;
  let rotation = 0;
  let flipH = false;
  let flipV = false;
  let adj: Adjustments = { ...DEFAULT_ADJ };
  let showAdj = false;
  let fullscreen = false;
  let cropMode = false;
  let cropBox: CropBox | null = null;
  let cropStart: { x: number; y: number } | null = null;
  let imgEl: HTMLImageElement;

  $: current = images[idx] ?? null;
  $: cssFilter = adjToFilter(adj);
  $: transform = `rotate(${rotation}deg) scaleX(${flipH ? -1 : 1}) scaleY(${flipV ? -1 : 1})`;

  async function openFiles() {
    const paths = await SystemBridge.pickFiles([{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'svg', 'avif', 'tiff'] }], 'Open Images', 'blue-images.open-files');
    if (!paths || paths.length === 0) return;
    images = paths.map((p) => ({ path: p, name: p.split('/').pop() ?? p, url: `asset://localhost/${p}` }));
    idx = 0;
    resetEdits();
  }

  function resetEdits() {
    zoom = 1; rotation = 0; flipH = false; flipV = false;
    adj = { ...DEFAULT_ADJ }; cropBox = null; cropMode = false;
  }

  function prev() { idx = Math.max(0, idx - 1); resetEdits(); }
  function next() { idx = Math.min(images.length - 1, idx + 1); resetEdits(); }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft') prev();
    if (e.key === 'ArrowRight') next();
    if (e.key === '+' || e.key === '=') zoom = Math.min(5, zoom + 0.1);
    if (e.key === '-') zoom = Math.max(0.1, zoom - 0.1);
    if (e.key === '0') zoom = 1;
    if (e.key === 'Escape') { cropMode = false; cropBox = null; fullscreen = false; }
  }

  onMount(() => window.addEventListener('keydown', handleKeyDown));
  onDestroy(() => window.removeEventListener('keydown', handleKeyDown));

  function handleScroll(e: WheelEvent) {
    e.preventDefault();
    zoom = Math.max(0.1, Math.min(5, zoom - e.deltaY * 0.001));
  }

  function onCropMouseDown(e: MouseEvent) {
    if (!cropMode || !imgEl) return;
    const rect = imgEl.getBoundingClientRect();
    cropStart = { x: e.clientX - rect.left, y: e.clientY - rect.top };
    cropBox = null;
  }
  function onCropMouseMove(e: MouseEvent) {
    if (!cropMode || !cropStart || !imgEl) return;
    const rect = imgEl.getBoundingClientRect();
    const cx = e.clientX - rect.left, cy = e.clientY - rect.top;
    cropBox = { x: Math.min(cx, cropStart.x), y: Math.min(cy, cropStart.y), w: Math.abs(cx - cropStart.x), h: Math.abs(cy - cropStart.y) };
  }
  function onCropMouseUp() { cropStart = null; }

  async function exportImage(format: 'png' | 'jpg' = 'png') {
    if (!imgEl || !current) return;
    const img = imgEl;
    const canvas = document.createElement('canvas');
    let w = img.naturalWidth, h = img.naturalHeight;
    if (rotation % 180 !== 0) [w, h] = [h, w];
    canvas.width = w; canvas.height = h;

    const ctx = canvas.getContext('2d')!;
    ctx.filter = cssFilter;
    ctx.translate(w / 2, h / 2);
    ctx.rotate((rotation * Math.PI) / 180);
    ctx.scale(flipH ? -1 : 1, flipV ? -1 : 1);
    ctx.drawImage(img, -img.naturalWidth / 2, -img.naturalHeight / 2);

    if (cropBox && cropBox.w > 0 && cropBox.h > 0) {
      const scaleX = img.naturalWidth / img.clientWidth;
      const scaleY = img.naturalHeight / img.clientHeight;
      const croppedCanvas = document.createElement('canvas');
      croppedCanvas.width = cropBox.w * scaleX;
      croppedCanvas.height = cropBox.h * scaleY;
      croppedCanvas.getContext('2d')!.drawImage(canvas, cropBox.x * scaleX, cropBox.y * scaleY, cropBox.w * scaleX, cropBox.h * scaleY, 0, 0, croppedCanvas.width, croppedCanvas.height);
      canvas.width = croppedCanvas.width;
      canvas.height = croppedCanvas.height;
      canvas.getContext('2d')!.drawImage(croppedCanvas, 0, 0);
    }

    const mime = format === 'jpg' ? 'image/jpeg' : 'image/png';
    canvas.toBlob((blob) => {
      if (!blob) return;
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = current!.name.replace(/\.[^.]+$/, '') + '-edited.' + format;
      a.click();
      URL.revokeObjectURL(url);
    }, mime, 0.92);
  }

  const sliders: { label: string; key: keyof Adjustments; min: number; max: number; step?: number }[] = [
    { label: 'Brightness', key: 'brightness', min: 0, max: 200 },
    { label: 'Contrast', key: 'contrast', min: 0, max: 200 },
    { label: 'Saturation', key: 'saturation', min: 0, max: 200 },
    { label: 'Hue Rotate', key: 'hue', min: -180, max: 180 },
    { label: 'Blur', key: 'blur', min: 0, max: 20, step: 0.5 },
    { label: 'Sepia', key: 'sepia', min: 0, max: 100 },
    { label: 'Grayscale', key: 'grayscale', min: 0, max: 100 },
    { label: 'Opacity', key: 'opacity', min: 0, max: 100 },
  ];
</script>

<div class="flex flex-col bg-slate-950 text-white overflow-hidden {fullscreen ? 'fixed inset-0 z-50' : 'h-full'}">
  <div class="shrink-0 flex items-center gap-1 px-3 py-2 bg-slate-900 border-b border-white/5">
    <button on:click={openFiles} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Open images"><FolderOpen size={15} /></button>
    <div class="w-px h-5 bg-white/10 mx-1" />
    <button on:click={() => (zoom = Math.min(5, zoom + 0.15))} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Zoom in (+)"><ZoomIn size={15} /></button>
    <button on:click={() => (zoom = Math.max(0.1, zoom - 0.15))} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Zoom out (-)"><ZoomOut size={15} /></button>
    <button on:click={() => (zoom = 1)} class="px-2 py-1 hover:bg-white/10 rounded-lg text-xs text-slate-400 hover:text-white font-mono">{Math.round(zoom * 100)}%</button>
    <div class="w-px h-5 bg-white/10 mx-1" />
    <button on:click={() => (rotation -= 90)} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Rotate left"><RotateCcw size={15} /></button>
    <button on:click={() => (rotation += 90)} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Rotate right"><RotateCw size={15} /></button>
    <button on:click={() => (flipH = !flipH)} class="p-1.5 rounded-lg text-sm {flipH ? 'bg-blue-600/25 text-blue-400' : 'hover:bg-white/10 text-slate-400 hover:text-white'}" title="Flip horizontal"><FlipHorizontal size={15} /></button>
    <button on:click={() => (flipV = !flipV)} class="p-1.5 rounded-lg {flipV ? 'bg-blue-600/25 text-blue-400' : 'hover:bg-white/10 text-slate-400 hover:text-white'}" title="Flip vertical"><FlipVertical size={15} /></button>
    <div class="w-px h-5 bg-white/10 mx-1" />
    <button on:click={() => { cropMode = !cropMode; if (cropMode) cropBox = null; }}
      class="p-1.5 rounded-lg {cropMode ? 'bg-orange-600/25 text-orange-400' : 'hover:bg-white/10 text-slate-400 hover:text-white'}" title="Crop"><Crop size={15} /></button>
    <button on:click={() => (showAdj = !showAdj)} class="p-1.5 rounded-lg {showAdj ? 'bg-blue-600/25 text-blue-400' : 'hover:bg-white/10 text-slate-400 hover:text-white'}" title="Adjustments"><Sliders size={15} /></button>
    <button on:click={() => (adj = { ...DEFAULT_ADJ })} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Reset adjustments"><RefreshCw size={14} /></button>
    <div class="w-px h-5 bg-white/10 mx-1" />
    <button on:click={() => exportImage('png')} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Export PNG"><Download size={15} /></button>
    <div class="ml-auto flex gap-1">
      {#if current}<span class="text-xs text-slate-600 self-center mr-2 font-mono">{current.name}</span>{/if}
      <button on:click={() => (fullscreen = !fullscreen)} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white">
        {#if fullscreen}<Minimize2 size={15} />{:else}<Maximize2 size={15} />{/if}
      </button>
    </div>
  </div>

  <div class="flex-1 flex overflow-hidden">
    {#if showAdj}
      <div class="w-52 shrink-0 bg-slate-900 border-r border-white/5 overflow-y-auto">
        <div class="p-4 space-y-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-xs font-medium text-slate-400 uppercase tracking-wider">Adjustments</span>
            <button on:click={() => (showAdj = false)} class="text-slate-600 hover:text-white"><X size={13} /></button>
          </div>
          {#each sliders as s (s.key)}
            <div class="space-y-1">
              <div class="flex justify-between text-xs">
                <span class="text-slate-400">{s.label}</span>
                <span class="text-slate-500 font-mono">{adj[s.key]}</span>
              </div>
              <input type="range" min={s.min} max={s.max} step={s.step ?? 1} value={adj[s.key]}
                on:input={(e) => (adj = { ...adj, [s.key]: parseFloat(e.currentTarget.value) })}
                class="w-full h-1.5 accent-blue-500 cursor-pointer" />
            </div>
          {/each}
          <div class="pt-2 border-t border-white/5 flex gap-2">
            <button on:click={() => exportImage('png')} class="flex-1 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs text-white font-medium">Export PNG</button>
            <button on:click={() => exportImage('jpg')} class="flex-1 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs text-slate-300">Export JPG</button>
          </div>
        </div>
      </div>
    {/if}

    <div class="flex-1 overflow-hidden relative flex items-center justify-center bg-slate-950" on:wheel={handleScroll}>
      {#if !current}
        <div class="flex flex-col items-center gap-4 text-slate-600">
          <ImageIcon size={56} strokeWidth={1} />
          <p class="text-sm">No image open</p>
          <button on:click={openFiles} class="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm text-white">Open images</button>
        </div>
      {:else}
        <div class="relative" on:mousedown={onCropMouseDown} on:mousemove={onCropMouseMove} on:mouseup={onCropMouseUp} style="cursor:{cropMode ? 'crosshair' : 'default'};">
          <img
            bind:this={imgEl}
            src={current.url}
            alt={current.name}
            draggable="false"
            style="transform:scale({zoom}) {transform}; filter:{cssFilter}; max-width:90vw; max-height:70vh; transition:transform 0.15s ease, filter 0.1s ease; user-select:none; transform-origin:center center;"
          />
          {#if cropMode && cropBox && cropBox.w > 0}
            <div style="position:absolute; left:{cropBox.x}px; top:{cropBox.y}px; width:{cropBox.w}px; height:{cropBox.h}px; border:2px dashed #f97316; background:rgba(249,115,22,0.1); pointer-events:none;" />
          {/if}
        </div>
      {/if}

      {#if images.length > 1}
        <button on:click={prev} disabled={idx === 0} class="absolute left-3 top-1/2 -translate-y-1/2 p-2 rounded-xl bg-black/40 hover:bg-black/60 text-white disabled:opacity-20 transition-all"><ChevronLeft size={22} /></button>
        <button on:click={next} disabled={idx === images.length - 1} class="absolute right-3 top-1/2 -translate-y-1/2 p-2 rounded-xl bg-black/40 hover:bg-black/60 text-white disabled:opacity-20 transition-all"><ChevronRight size={22} /></button>
      {/if}
    </div>
  </div>

  {#if images.length > 1}
    <div class="shrink-0 flex gap-1.5 px-3 py-2 bg-slate-900 border-t border-white/5 overflow-x-auto scrollbar-hide">
      {#each images as img, i (img.path)}
        <button on:click={() => { idx = i; resetEdits(); }} class="w-12 h-12 rounded-lg overflow-hidden shrink-0 border-2 transition-all {i === idx ? 'border-blue-500' : 'border-transparent hover:border-slate-500'}">
          <img src={img.url} alt={img.name} class="w-full h-full object-cover" />
        </button>
      {/each}
    </div>
  {/if}

  {#if current}
    <div class="shrink-0 flex items-center justify-between px-4 py-1 bg-slate-900 border-t border-white/5 text-xs text-slate-600">
      <span>{idx + 1} / {images.length}</span>
      <span class="font-mono">{Math.round(zoom * 100)}%{rotation !== 0 ? ` · ${rotation}°` : ''}</span>
      {#if cropMode}<span class="text-orange-500">Crop mode — drag to select</span>{/if}
    </div>
  {/if}
</div>
