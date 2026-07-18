<script lang="ts">
  import { Save } from 'lucide-svelte';
  import type { DocFormat } from './types';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { createEventDispatcher } from 'svelte';

  export let defaultName: string;
  export let format: DocFormat;

  const dispatch = createEventDispatcher<{ confirm: string; cancel: void }>();

  const EXPORT_FORMATS: Record<DocFormat, { ext: string; label: string }[]> = {
    rich: [
      { ext: 'docx', label: 'Word Document (.docx)' }, { ext: 'html', label: 'Web Page (.html)' },
      { ext: 'txt', label: 'Plain Text (.txt)' }, { ext: 'md', label: 'Markdown (.md)' },
      { ext: 'pdf', label: 'PDF (.pdf) — via print' },
    ],
    markdown: [
      { ext: 'md', label: 'Markdown (.md)' }, { ext: 'docx', label: 'Word Document (.docx)' },
      { ext: 'txt', label: 'Plain Text (.txt)' }, { ext: 'html', label: 'HTML (.html)' },
    ],
    spreadsheet: [{ ext: 'csv', label: 'CSV (.csv)' }, { ext: 'json', label: 'JSON (.json)' }],
    presentation: [{ ext: 'html', label: 'HTML Slides (.html)' }, { ext: 'json', label: 'JSON (.json)' }],
  };

  let dir = '~/Documents';
  let name = defaultName;
  let ext = EXPORT_FORMATS[format][0].ext;
  $: formats = EXPORT_FORMATS[format];

  async function pickDir() {
    try { const picked = await SystemBridge.pickDirectory?.(); if (picked) dir = picked; } catch {}
  }
  function handleSave() {
    const base = name.replace(/\.[^.]+$/, '');
    dispatch('confirm', `${dir}/${base}.${ext}`);
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
  <div class="bg-slate-900 border border-white/10 rounded-2xl p-6 w-[440px] shadow-2xl">
    <div class="flex items-center gap-3 mb-5">
      <div class="w-9 h-9 rounded-xl bg-blue-600/20 flex items-center justify-center"><Save size={18} class="text-blue-400" /></div>
      <div>
        <h2 class="text-white font-semibold">Save Document</h2>
        <p class="text-slate-500 text-xs">Choose location and format</p>
      </div>
    </div>

    <div class="mb-4">
      <label class="block text-xs text-slate-500 mb-1.5">Location</label>
      <div class="flex gap-2">
        <div class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-slate-300 font-mono truncate">{dir}</div>
        <button on:click={pickDir} class="px-3 py-2 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs text-slate-300 transition-colors">Browse</button>
      </div>
    </div>

    <div class="flex gap-1.5 mb-4 flex-wrap">
      {#each ['~/Documents', '~/Desktop', '~/Downloads'] as d (d)}
        <button on:click={() => (dir = d)} class="px-2.5 py-1 rounded-lg text-xs transition-colors {dir === d ? 'bg-blue-600/30 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700'}">{d.replace('~/', '')}</button>
      {/each}
    </div>

    <div class="mb-4">
      <label class="block text-xs text-slate-500 mb-1.5">File name</label>
      <input bind:value={name} class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2.5 text-sm text-white focus:outline-none focus:border-blue-500/50" autofocus on:keydown={(e) => e.key === 'Enter' && handleSave()} />
    </div>

    <div class="mb-5">
      <label class="block text-xs text-slate-500 mb-1.5">Format</label>
      <div class="flex flex-wrap gap-1.5">
        {#each formats as f (f.ext)}
          <button on:click={() => (ext = f.ext)} class="px-3 py-1.5 rounded-lg text-xs transition-colors {ext === f.ext ? 'bg-blue-600/25 text-blue-300 border border-blue-500/30' : 'bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700'}">{f.label}</button>
        {/each}
      </div>
    </div>

    <div class="mb-5 px-3 py-2 bg-slate-950 rounded-lg border border-white/5">
      <span class="text-xs text-slate-600">Path: </span>
      <span class="text-xs text-slate-400 font-mono">{dir}/{name.replace(/\.[^.]+$/, '')}.{ext}</span>
    </div>

    <div class="flex gap-2 justify-end">
      <button on:click={() => dispatch('cancel')} class="px-4 py-2 rounded-lg text-sm text-slate-400 hover:text-white hover:bg-slate-700 transition-colors">Cancel</button>
      <button on:click={handleSave} class="px-5 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm text-white font-medium flex items-center gap-2"><Save size={14} /> Save</button>
    </div>
  </div>
</div>
