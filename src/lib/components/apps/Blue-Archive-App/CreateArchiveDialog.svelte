<script lang="ts">
  import { Archive, FolderOpen, Plus, X, Save, Loader2 } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { createEventDispatcher } from 'svelte';

  type ArchiveFormat = 'zip' | 'tar.gz' | 'tar.bz2' | 'tar.xz' | '7z';

  const dispatch = createEventDispatcher<{ close: void; created: string }>();

  function getCmd(format: ArchiveFormat, output: string, files: string[]): string {
    const escapedFiles = files.map((f) => `'${f.replace(/'/g, "'\\''")}'`).join(' ');
    const out = `'${output.replace(/'/g, "'\\''")}'`;
    switch (format) {
      case 'zip': return `zip -r ${out} ${escapedFiles}`;
      case 'tar.gz': return `tar -czf ${out} ${escapedFiles}`;
      case 'tar.bz2': return `tar -cjf ${out} ${escapedFiles}`;
      case 'tar.xz': return `tar -cJf ${out} ${escapedFiles}`;
      case '7z': return `7z a ${out} ${escapedFiles}`;
    }
  }

  let name = 'archive';
  let format: ArchiveFormat = 'zip';
  let dir = '~/Documents';
  let files: string[] = [];
  let log = '';
  let busy = false;
  let error = '';

  $: outputPath = `${dir}/${name}.${format}`;

  async function addFiles() {
    const paths = await SystemBridge.pickFiles([], 'Select files to compress');
    if (paths?.length) files = [...new Set([...files, ...paths])];
  }
  async function addFolder() {
    const path = await SystemBridge.pickDirectory?.();
    if (path) files = [...new Set([...files, path])];
  }

  async function create() {
    if (!files.length) { error = 'Add at least one file or folder.'; return; }
    busy = true; error = ''; log = '';

    const expandedDir = dir.startsWith('~/') ? dir.replace('~', '$HOME') : dir;
    const expandedOut = `${expandedDir}/${name}.${format}`;

    await SystemBridge.executeCommand(`mkdir -p "${expandedDir}"`).catch(() => {});

    const cmd = getCmd(format, expandedOut, files);
    log = `Running: ${cmd}\n`;

    try {
      const result = await SystemBridge.executeCommand(cmd);
      const out = typeof result === 'string' ? result : (result as any)?.stdout ?? '';
      log += out || 'Done.';
      dispatch('created', expandedOut);
      setTimeout(() => dispatch('close'), 1200);
    } catch (e: any) {
      error = e?.message ?? 'Archive creation failed';
    } finally {
      busy = false;
    }
  }

  const FORMATS: { id: ArchiveFormat; label: string; note: string }[] = [
    { id: 'zip', label: '.zip', note: 'Universal, good compression' },
    { id: 'tar.gz', label: '.tar.gz', note: 'Fast, Linux standard' },
    { id: 'tar.bz2', label: '.tar.bz2', note: 'Better compression, slower' },
    { id: 'tar.xz', label: '.tar.xz', note: 'Best compression, slowest' },
    { id: '7z', label: '.7z', note: 'Requires p7zip' },
  ];
  const dirShortcuts = ['~/Documents', '~/Desktop', '~/Downloads'];
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
  <div class="bg-slate-900 border border-white/10 rounded-2xl p-6 w-[500px] max-h-[85vh] overflow-y-auto shadow-2xl">
    <div class="flex items-center gap-3 mb-5">
      <div class="w-9 h-9 rounded-xl bg-orange-600/20 flex items-center justify-center"><Archive size={18} class="text-orange-400" /></div>
      <div>
        <h2 class="text-white font-semibold">Create Archive</h2>
        <p class="text-slate-500 text-xs">Compress files into a new archive</p>
      </div>
      <button on:click={() => dispatch('close')} class="ml-auto text-slate-600 hover:text-white"><X size={16} /></button>
    </div>

    <div class="space-y-3 mb-4">
      <div>
        <span class="block text-xs text-slate-500 mb-1.5">Archive name</span>
        <input bind:value={name} class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
      </div>
      <div>
        <span class="block text-xs text-slate-500 mb-1.5">Save to</span>
        <div class="flex gap-2">
          <div class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-slate-300 font-mono truncate">{dir}</div>
        </div>
        <div class="flex gap-1.5 mt-1.5 flex-wrap">
          {#each dirShortcuts as d (d)}
            <button on:click={() => (dir = d)} class="px-2 py-1 rounded-lg text-xs transition-colors {dir === d ? 'bg-blue-600/30 text-blue-300' : 'bg-slate-800 text-slate-500 hover:text-white'}">
              {d.replace('~/', '')}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="mb-4">
      <span class="block text-xs text-slate-500 mb-1.5">Format</span>
      <div class="grid grid-cols-3 gap-1.5">
        {#each FORMATS as f (f.id)}
          <button on:click={() => (format = f.id)} class="px-2 py-2 rounded-xl text-xs text-left transition-colors {format === f.id ? 'bg-blue-600/25 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white'}">
            <div class="font-mono font-medium">{f.label}</div>
            <div class="text-slate-600 text-[10px] mt-0.5">{f.note}</div>
          </button>
        {/each}
      </div>
    </div>

    <div class="mb-4">
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-xs text-slate-500">Files to compress ({files.length})</span>
        <div class="flex gap-1.5">
          <button on:click={addFiles} class="flex items-center gap-1 px-2 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs text-slate-300 transition-colors"><Plus size={11} /> Files</button>
          <button on:click={addFolder} class="flex items-center gap-1 px-2 py-1 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs text-slate-300 transition-colors"><FolderOpen size={11} /> Folder</button>
        </div>
      </div>
      {#if files.length === 0}
        <div class="border border-dashed border-white/10 rounded-xl p-4 text-center text-slate-600 text-xs">No files added yet</div>
      {:else}
        <div class="bg-slate-800/60 rounded-xl border border-white/5 max-h-32 overflow-y-auto">
          {#each files as f (f)}
            <div class="flex items-center gap-2 px-3 py-1.5 border-b border-white/5 last:border-0">
              <span class="flex-1 text-xs font-mono text-slate-400 truncate">{f}</span>
              <button on:click={() => (files = files.filter((p) => p !== f))} class="text-slate-700 hover:text-red-400 shrink-0"><X size={11} /></button>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="mb-4 px-3 py-2 bg-slate-950 rounded-lg border border-white/5">
      <span class="text-xs text-slate-600">Output: </span>
      <span class="text-xs text-slate-400 font-mono">{outputPath}</span>
    </div>

    {#if log}
      <pre class="mb-4 p-3 bg-slate-950 rounded-xl text-[11px] text-slate-400 font-mono max-h-24 overflow-y-auto border border-white/5">{log}</pre>
    {/if}
    {#if error}<div class="mb-4 text-sm text-red-400">{error}</div>{/if}

    <div class="flex gap-2 justify-end">
      <button on:click={() => dispatch('close')} class="px-4 py-2 rounded-lg text-sm text-slate-400 hover:text-white hover:bg-slate-700 transition-colors">Cancel</button>
      <button on:click={create} disabled={busy || !files.length} class="px-5 py-2 bg-orange-600 hover:bg-orange-500 disabled:opacity-50 rounded-lg text-sm text-white font-medium flex items-center gap-2">
        {#if busy}<Loader2 size={14} class="animate-spin" />Creating…{:else}<Save size={14} />Create{/if}
      </button>
    </div>
  </div>
</div>
