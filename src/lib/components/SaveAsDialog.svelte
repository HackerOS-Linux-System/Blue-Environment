<script lang="ts">
  import { Save, FolderOpen } from 'lucide-svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import { createEventDispatcher } from 'svelte';

  export let defaultName: string;

  const dispatch = createEventDispatcher<{ confirm: { path: string; format: string }; cancel: void }>();

  let name = defaultName.endsWith('.txt') ? defaultName : `${defaultName}.txt`;
  let format = 'txt';
  let dir = '~/Documents';

  const formats = [
    { ext: 'txt', label: 'Plain Text (.txt)' },
    { ext: 'md', label: 'Markdown (.md)' },
    { ext: 'html', label: 'HTML (.html)' },
    { ext: 'log', label: 'Log (.log)' },
    { ext: 'json', label: 'JSON (.json)' },
    { ext: 'csv', label: 'CSV (.csv)' },
  ];

  function handleFormatChange(ext: string) {
    format = ext;
    const base = name.replace(/\.[^.]+$/, '');
    name = `${base}.${ext}`;
  }

  function handleSave() {
    const fullPath = `${dir}/${name}`.replace(/\/+/g, '/');
    dispatch('confirm', { path: fullPath, format });
  }

  async function pickDir() {
    try {
      const picked = await SystemBridge.pickDirectory?.();
      if (picked) dir = picked;
    } catch {}
  }

  const shortcuts = ['~/Documents', '~/Desktop', '~/Downloads', '~'];
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
  <div class="bg-slate-900 border border-white/10 rounded-2xl p-6 w-[440px] shadow-2xl" style="box-shadow:0 0 60px rgba(59,130,246,0.15);">
    <div class="flex items-center gap-3 mb-5">
      <div class="w-9 h-9 rounded-xl bg-blue-600/20 flex items-center justify-center"><Save size={18} class="text-blue-400" /></div>
      <div>
        <h2 class="text-white font-semibold">Save File</h2>
        <p class="text-slate-500 text-xs">Choose where to save your document</p>
      </div>
    </div>

    <div class="mb-4">
      <label class="block text-xs text-slate-500 mb-1.5">Save location</label>
      <div class="flex gap-2">
        <div class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-slate-300 font-mono truncate">{dir}</div>
        <button on:click={pickDir} class="px-3 py-2 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs text-slate-300 transition-colors flex items-center gap-1.5">
          <FolderOpen size={13} /> Browse
        </button>
      </div>
    </div>

    <div class="flex gap-1.5 mb-4 flex-wrap">
      {#each shortcuts as d (d)}
        <button on:click={() => (dir = d)} class="px-2.5 py-1 rounded-lg text-xs transition-colors {dir === d ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700'}">
          {d.replace('~/', '').replace('~', 'Home') || 'Home'}
        </button>
      {/each}
    </div>

    <div class="mb-4">
      <label class="block text-xs text-slate-500 mb-1.5">File name</label>
      <input bind:value={name} class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2.5 text-sm text-white focus:outline-none focus:border-blue-500/50"
        autofocus on:keydown={(e) => e.key === 'Enter' && handleSave()} />
    </div>

    <div class="mb-6">
      <label class="block text-xs text-slate-500 mb-1.5">Format</label>
      <div class="grid grid-cols-3 gap-1.5">
        {#each formats as f (f.ext)}
          <button on:click={() => handleFormatChange(f.ext)} class="px-2 py-1.5 rounded-lg text-xs transition-colors text-left {format === f.ext ? 'bg-blue-600/25 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700'}">
            {f.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="mb-5 px-3 py-2 bg-slate-950 rounded-lg border border-white/5">
      <span class="text-xs text-slate-600">Full path: </span>
      <span class="text-xs text-slate-400 font-mono">{dir}/{name}</span>
    </div>

    <div class="flex gap-2 justify-end">
      <button on:click={() => dispatch('cancel')} class="px-4 py-2 rounded-lg text-sm text-slate-400 hover:text-white hover:bg-slate-700 transition-colors">Cancel</button>
      <button on:click={handleSave} class="px-5 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm text-white font-medium transition-colors flex items-center gap-2">
        <Save size={14} /> Save
      </button>
    </div>
  </div>
</div>
