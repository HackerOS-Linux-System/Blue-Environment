<script lang="ts">
  import {
    Bold, Italic, Underline, Strikethrough, AlignLeft, AlignCenter,
    AlignRight, AlignJustify, List, ListOrdered, Link, Image,
    Undo2, Redo2, Save, FolderOpen, FilePlus, Download,
    Heading1, Heading2, Heading3, Minus, Table, Code, Quote,
    Type, Highlighter,
  } from 'lucide-svelte';
  import type { DocFormat } from './types';
  import { createEventDispatcher } from 'svelte';

  export let format: DocFormat;
  export let canUndo: boolean;
  export let canRedo: boolean;
  export let fontSize: number;
  export let fontFamily: string;

  const dispatch = createEventDispatcher<{
    undo: void; redo: void; new: DocFormat; open: void; save: void; saveAs: void;
    format: { cmd: string; val?: string };
  }>();

  const FONTS = ['System UI', 'Serif', 'Monospace', 'DM Sans', 'Oxanium'];
  const FONT_SIZES = [10, 11, 12, 13, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64];
  const DOC_FORMATS: DocFormat[] = ['rich', 'markdown', 'spreadsheet', 'presentation'];

  $: isRich = format === 'rich';
  $: isMd = format === 'markdown';
  let showNewMenu = false;
</script>

<div class="shrink-0 border-b border-white/5" style="background:#0f1629;">
  <div class="flex items-center gap-0.5 px-3 py-1.5 border-b border-white/5 overflow-x-auto scrollbar-hide">
    <div class="relative" on:mouseenter={() => (showNewMenu = true)} on:mouseleave={() => (showNewMenu = false)}>
      <button on:click={() => dispatch('new', 'rich')} title="New Document" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><FilePlus size={14} /></button>
      {#if showNewMenu}
        <div class="absolute left-0 top-full mt-1 w-44 bg-slate-900 border border-white/10 rounded-xl overflow-hidden z-50">
          {#each DOC_FORMATS as f (f)}
            <button on:click={() => dispatch('new', f)} class="w-full px-3 py-2 text-left text-sm text-slate-300 hover:bg-white/5 capitalize">
              {f === 'rich' ? 'Rich Text Document' : f.charAt(0).toUpperCase() + f.slice(1)}
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <button on:click={() => dispatch('open')} title="Open file" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><FolderOpen size={14} /></button>
    <button on:click={() => dispatch('save')} title="Save (Ctrl+S)" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><Save size={14} /></button>
    <button on:click={() => dispatch('saveAs')} title="Save As…" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><Download size={14} /></button>
    <div class="w-px h-5 bg-white/10 mx-1 shrink-0" />
    <button on:click={() => dispatch('undo')} title="Undo (Ctrl+Z)" disabled={!canUndo} class="p-1.5 rounded-lg transition-colors {!canUndo ? 'opacity-30' : 'hover:bg-white/10 text-slate-400 hover:text-white'}"><Undo2 size={14} /></button>
    <button on:click={() => dispatch('redo')} title="Redo (Ctrl+Y)" disabled={!canRedo} class="p-1.5 rounded-lg transition-colors {!canRedo ? 'opacity-30' : 'hover:bg-white/10 text-slate-400 hover:text-white'}"><Redo2 size={14} /></button>
  </div>

  {#if isRich || isMd}
    <div class="flex items-center gap-0.5 px-3 py-1.5 overflow-x-auto scrollbar-hide">
      <select bind:value={fontFamily} class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1 text-xs text-slate-300 focus:outline-none mr-1">
        {#each FONTS as f (f)}<option value={f}>{f}</option>{/each}
      </select>
      <select bind:value={fontSize} class="bg-slate-800 border border-white/10 rounded-lg px-2 py-1 text-xs text-slate-300 focus:outline-none w-16 mr-1">
        {#each FONT_SIZES as s (s)}<option value={s}>{s}</option>{/each}
      </select>
      <div class="w-px h-5 bg-white/10 mx-1 shrink-0" />
      <button on:click={() => dispatch('format', { cmd: 'heading', val: '1' })} title="Heading 1" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Heading1 size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'heading', val: '2' })} title="Heading 2" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Heading2 size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'heading', val: '3' })} title="Heading 3" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Heading3 size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'paragraph' })} title="Paragraph" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Type size={14} /></button>
      <div class="w-px h-5 bg-white/10 mx-1 shrink-0" />
      <button on:click={() => dispatch('format', { cmd: 'bold' })} title="Bold (Ctrl+B)" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Bold size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'italic' })} title="Italic (Ctrl+I)" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Italic size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'underline' })} title="Underline (Ctrl+U)" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Underline size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'strikethrough' })} title="Strikethrough" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Strikethrough size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'code' })} title="Inline code" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Code size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'highlight' })} title="Highlight" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Highlighter size={14} /></button>
      <div class="w-px h-5 bg-white/10 mx-1 shrink-0" />
      <button on:click={() => dispatch('format', { cmd: 'align', val: 'left' })} title="Align left" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><AlignLeft size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'align', val: 'center' })} title="Center" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><AlignCenter size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'align', val: 'right' })} title="Align right" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><AlignRight size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'align', val: 'justify' })} title="Justify" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><AlignJustify size={14} /></button>
      <div class="w-px h-5 bg-white/10 mx-1 shrink-0" />
      <button on:click={() => dispatch('format', { cmd: 'bullet' })} title="Bullet list" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><List size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'ordered' })} title="Ordered list" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><ListOrdered size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'quote' })} title="Blockquote" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Quote size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'hr' })} title="Horizontal rule" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Minus size={14} /></button>
      <div class="w-px h-5 bg-white/10 mx-1 shrink-0" />
      <button on:click={() => dispatch('format', { cmd: 'link' })} title="Insert link" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Link size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'image' })} title="Insert image" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Image size={14} /></button>
      <button on:click={() => dispatch('format', { cmd: 'table' })} title="Insert table" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white"><Table size={14} /></button>
    </div>
  {/if}
</div>
