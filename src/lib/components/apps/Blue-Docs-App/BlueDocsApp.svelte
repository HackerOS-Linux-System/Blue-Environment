<script lang="ts">
  import { stopPropagation } from 'svelte/legacy';

  import { onMount, onDestroy } from 'svelte';
  import {
    X, FileText, FilePlus, Sidebar, Printer, Maximize2, Minimize2,
  } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { createDocumentState } from './document';
  import type { DocFormat } from './types';
  import Toolbar from './Toolbar.svelte';
  import RichEditor from './RichEditor.svelte';
  import MarkdownEditor from './MarkdownEditor.svelte';
  import SpreadsheetEditor from './SpreadsheetEditor.svelte';
  import PresentationEditor from './PresentationEditor.svelte';
  import OutlineSidebar from './OutlineSidebar.svelte';
  import ReviewPanel from './ReviewPanel.svelte';
  import { profileState, activeAuthor, loadProfiles } from './docsProfile';
  import SaveAsDialog from './SaveAsDialog.svelte';

  const doc = createDocumentState();
  const { docs, activeId, history } = doc;

  let fontSize = $state(14);
  let fontFamily = $state('System UI');
  let showSidebar = $state(true);
  let saveAsOpen = $state(false);
  let distraction = $state(false);
  let richEditorRef = $state() as RichEditor;
  let reviewOpen = $state(false);
  let trackChanges = $state(false);
  let commentDraft = $state('');
  let author = $derived(activeAuthor($profileState));

  let activeDoc = $derived($docs.find((d) => d.id === $activeId) ?? $docs[0]);
  let hist = $derived($history[$activeId]);
  let canUndo = $derived((hist?.past.length ?? 0) > 0);
  let canRedo = $derived((hist?.future.length ?? 0) > 0);

  function handleFormat(e: CustomEvent<{ cmd: string; val?: string }>) {
    richEditorRef?.applyFormat(e.detail.cmd, e.detail.val);
  }

  async function handleOpen() {
    const path = await SystemBridge.pickFile(
      [
        { name: 'Documents', extensions: ['html', 'htm', 'md', 'txt', 'csv', 'json', 'docx', 'pdf', 'odt', 'rtf', 'xlsx', 'xlsm', 'xls', 'ods'] },
        { name: 'Spreadsheets', extensions: ['xlsx', 'xlsm', 'xlsb', 'xls', 'ods', 'csv'] },
        { name: 'Word Documents', extensions: ['docx', 'doc'] },
        { name: 'PDF Files', extensions: ['pdf'] },
        { name: 'All Files', extensions: ['*'] },
      ],
      'Open Document'
    );
    if (path) doc.openDocFromPath(path);
  }

  async function handleSave() {
    if (!activeDoc.path) saveAsOpen = true;
    else await doc.saveDoc();
  }

  function handleExport(fmt: string) { if (fmt === 'pdf') window.print(); }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey) {
      if (e.key === 's') { e.preventDefault(); handleSave(); }
      if (e.key === 'z') { e.preventDefault(); doc.undo(); }
      if (e.key === 'y') { e.preventDefault(); doc.redo(); }
      if (e.key === 'p') { e.preventDefault(); window.print(); }
      if (e.key === 't') { e.preventDefault(); doc.newDoc('rich'); }
    }
  }
  onMount(() => window.addEventListener('keydown', handleKeyDown));
  onDestroy(() => window.removeEventListener('keydown', handleKeyDown));
</script>

<div class="flex flex-col h-full bg-slate-950 text-white overflow-hidden">
  {#if saveAsOpen}
    <SaveAsDialog defaultName={activeDoc.name} format={activeDoc.format}
      on:confirm={(e) => { saveAsOpen = false; doc.saveDocAs(e.detail); }} on:cancel={() => (saveAsOpen = false)} />
  {/if}

  {#if !distraction}
    <div class="shrink-0 flex bg-slate-900 border-b border-white/5 overflow-x-auto scrollbar-hide">
      {#each $docs as d (d.id)}
        <div onclick={() => activeId.set(d.id)} role="button" tabindex="0" onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => activeId.set(d.id))(); } }}
          class="flex items-center gap-1.5 px-4 py-2.5 cursor-pointer shrink-0 border-r border-white/5 text-sm group max-w-[180px] transition-colors {$activeId === d.id ? 'bg-slate-950 text-white' : 'text-slate-500 hover:text-white hover:bg-slate-800/50'}">
          <FileText size={13} class="shrink-0" />
          <span class="truncate">{d.name}</span>
          {#if d.modified}<span class="w-1.5 h-1.5 rounded-full bg-blue-400 shrink-0"></span>{/if}
          <button onclick={stopPropagation(() => doc.closeDoc(d.id))} class="opacity-0 group-hover:opacity-100 hover:text-red-400 ml-0.5 shrink-0 transition-opacity"><X size={11} /></button>
        </div>
      {/each}
      <button onclick={() => doc.newDoc('rich')} class="p-2.5 hover:bg-slate-800/50 text-slate-600 hover:text-white shrink-0 transition-colors"><FilePlus size={13} /></button>
      <button onclick={() => (distraction = true)} class="ml-auto p-2.5 hover:bg-slate-800/50 text-slate-600 hover:text-white shrink-0 transition-colors" title="Distraction-free mode"><Maximize2 size={13} /></button>
    </div>
  {/if}

  {#if !distraction}
    <Toolbar format={activeDoc.format} {canUndo} {canRedo} bind:fontSize bind:fontFamily {reviewOpen} {trackChanges}
      on:toggleReview={() => { reviewOpen = !reviewOpen; if (reviewOpen) loadProfiles(); }} on:toggleTracking={() => { trackChanges = !trackChanges; if (trackChanges) { reviewOpen = true; loadProfiles(); } }}
      on:undo={doc.undo} on:redo={doc.redo}
      on:new={(e) => doc.newDoc(e.detail)} on:open={handleOpen}
      on:save={handleSave} on:saveAs={() => (saveAsOpen = true)}
      on:format={handleFormat} />
  {/if}

  <div class="flex-1 flex overflow-hidden relative">
    {#if distraction}
      <button onclick={() => (distraction = false)} class="absolute top-4 right-4 z-10 p-2 rounded-xl bg-white/5 hover:bg-white/10 text-slate-500 hover:text-white transition-colors" title="Exit distraction-free">
        <Minimize2 size={15} />
      </button>
    {/if}

    <div class="flex-1 flex overflow-hidden {distraction ? 'docs-distraction-surface' : ''}">
      {#if activeDoc.format === 'rich'}
        <RichEditor bind:this={richEditorRef} content={activeDoc.content} {fontFamily} {fontSize} {trackChanges} {author} on:change={(e) => doc.updateContent(e.detail)} />
      {:else if activeDoc.format === 'markdown'}
        <MarkdownEditor content={activeDoc.content} on:change={(e) => doc.updateContent(e.detail)} />
      {:else if activeDoc.format === 'spreadsheet'}
        <SpreadsheetEditor content={activeDoc.content} on:change={(e) => doc.updateContent(e.detail)} />
      {:else if activeDoc.format === 'presentation'}
        <PresentationEditor content={activeDoc.content} on:change={(e) => doc.updateContent(e.detail)} />
      {/if}
    </div>

    {#if reviewOpen && !distraction && activeDoc.format === 'rich'}
      <ReviewPanel
        content={activeDoc.content} {trackChanges} bind:commentDraft
        onToggleTracking={() => (trackChanges = !trackChanges)}
        onAddComment={(t) => richEditorRef?.addComment(t)}
        onReply={(id, t) => richEditorRef?.replyComment(id, t)}
        onResolve={(id, r) => richEditorRef?.resolveComment(id, r)}
        onDeleteComment={(id) => richEditorRef?.removeComment(id)}
        onChange={(a, id) => richEditorRef?.changeAction(a, id)}
        onFocusComment={(id) => richEditorRef?.focusComment(id)} />
    {/if}

    {#if showSidebar && !distraction && (activeDoc.format === 'rich' || activeDoc.format === 'markdown')}
      <OutlineSidebar content={activeDoc.content} format={activeDoc.format} />
    {/if}
  </div>

  {#if !distraction}
    <div class="shrink-0 flex items-center justify-between px-4 py-1 bg-slate-900 border-t border-white/5 text-xs text-slate-600">
      <div class="flex gap-4">
        <span class="capitalize text-slate-500">{activeDoc.format}</span>
        {#if activeDoc.path}<span class="font-mono truncate max-w-xs text-slate-700" title={activeDoc.path}>{activeDoc.path}</span>{/if}
      </div>
      <div class="flex items-center gap-3">
        {#if activeDoc.modified}<span class="text-yellow-600 cursor-pointer hover:text-yellow-400" onclick={handleSave}>● Unsaved</span>{/if}
        <button onclick={() => (showSidebar = !showSidebar)} class="flex items-center gap-1 transition-colors {showSidebar ? 'text-blue-500' : 'hover:text-slate-400'}" title="Toggle outline sidebar">
          <Sidebar size={12} /><span>Outline</span>
        </button>
        <button onclick={() => window.print()} class="hover:text-slate-400" title="Print (Ctrl+P)"><Printer size={12} /></button>
      </div>
    </div>
  {/if}
</div>
