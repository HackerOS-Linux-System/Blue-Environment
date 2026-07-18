<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import {
    Save, FolderOpen, Plus, X, FileText, Download,
    Bold, Italic, Hash, Search,
  } from 'lucide-svelte';
  import SaveAsDialog from './SaveAsDialog.svelte';

  interface NoteTab { id: string; title: string; content: string; path?: string; modified: boolean; }
  interface FindState { query: string; replace: string; show: boolean; matchCase: boolean; count: number; }

  let tabs: NoteTab[] = [];
  let activeId: string | null = null;
  let find: FindState = { query: '', replace: '', show: false, matchCase: false, count: 0 };
  let wordCount = 0;
  let lineCount = 0;
  let cursorPos = { line: 1, col: 1 };
  let saveAsTarget: NoteTab | null = null;
  let textareaEl: HTMLTextAreaElement;
  let autosaveTimer: ReturnType<typeof setInterval>;
  let findInputEl: HTMLInputElement;

  $: activeTab = tabs.find((t) => t.id === activeId) ?? null;

  onMount(() => {
    const id = `note-${Date.now()}`;
    tabs = [{ id, title: 'Untitled', content: '', modified: false }];
    activeId = id;

    SystemBridge.executeCommand('cat ~/.cache/Blue-Environment/notepad-autosave.json 2>/dev/null').then((r) => {
      const out = typeof r === 'string' ? r : (r as any)?.stdout || '';
      try {
        const saved = JSON.parse(out);
        if (saved.content) {
          tabs = tabs.map((t) => (t.id === activeId ? { ...t, content: saved.content, title: saved.title || 'Untitled', modified: false } : t));
        }
      } catch {}
    });

    autosaveTimer = setInterval(() => {
      tabs.forEach((tab) => {
        if (tab.modified && !tab.path) saveToCacheNote(tab);
        else if (tab.modified && tab.path) writeToPath(tab);
      });
    }, 30000);
  });
  onDestroy(() => clearInterval(autosaveTimer));

  async function saveToCacheNote(tab: NoteTab) {
    const data = JSON.stringify({ content: tab.content, title: tab.title });
    await SystemBridge.executeCommand(`mkdir -p ~/.cache/Blue-Environment && printf '%s' ${JSON.stringify(data)} > ~/.cache/Blue-Environment/notepad-autosave.json`).catch(() => {});
  }

  async function writeToPath(tab: NoteTab) {
    if (!tab.path) return false;
    try {
      const expandedPath = tab.path.startsWith('~/') ? tab.path.replace('~', '$HOME') : tab.path;
      await SystemBridge.executeCommand(`mkdir -p "$(dirname "${expandedPath}")" && printf '%s' ${JSON.stringify(tab.content)} > "${expandedPath}"`);
      return true;
    } catch { return false; }
  }

  function updateContent(id: string, content: string) {
    tabs = tabs.map((t) => (t.id === id ? { ...t, content, modified: true } : t));
    const lines = content.split('\n');
    lineCount = lines.length;
    wordCount = content.trim() ? content.trim().split(/\s+/).length : 0;
  }

  function newTab() {
    const id = `note-${Date.now()}`;
    tabs = [...tabs, { id, title: 'Untitled', content: '', modified: false }];
    activeId = id;
  }

  function closeTab(id: string, e: MouseEvent) {
    e.stopPropagation();
    const next = tabs.filter((t) => t.id !== id);
    if (next.length === 0) {
      const newId = `note-${Date.now()}`;
      activeId = newId;
      tabs = [{ id: newId, title: 'Untitled', content: '', modified: false }];
      return;
    }
    if (activeId === id) activeId = next[next.length - 1].id;
    tabs = next;
  }

  async function openFile() {
    const path = await SystemBridge.pickFile(
      [
        { name: 'Text Files', extensions: ['txt', 'md', 'json', 'csv', 'log', 'toml', 'yaml', 'yml', 'sh', 'py', 'js', 'ts', 'css', 'html', 'xml', 'ini', 'conf'] },
        { name: 'All Files', extensions: ['*'] },
      ],
      'Open File'
    );
    if (!path) return;
    try {
      const text = await SystemBridge.readFile(path);
      const name = path.split('/').pop() || path;
      const id = `note-${Date.now()}`;
      tabs = [...tabs, { id, title: name, content: text || '', path, modified: false }];
      activeId = id;
    } catch (e) { console.error('Failed to open file:', e); }
  }

  async function saveFile(tab?: NoteTab) {
    const target = tab ?? activeTab;
    if (!target) return;
    if (target.path) {
      const ok = await writeToPath(target);
      if (ok) tabs = tabs.map((t) => (t.id === target.id ? { ...t, modified: false } : t));
    } else {
      saveAsTarget = target;
    }
  }

  function saveAs(tab?: NoteTab) {
    const target = tab ?? activeTab;
    if (!target) return;
    saveAsTarget = target;
  }

  async function handleSaveAsConfirm(e: CustomEvent<{ path: string; format: string }>) {
    const target = saveAsTarget;
    if (!target) return;
    saveAsTarget = null;

    const fileName = e.detail.path.split('/').pop() || 'Untitled';
    const updatedTab: NoteTab = { ...target, path: e.detail.path, title: fileName, modified: false };
    const ok = await writeToPath(updatedTab);

    if (ok) {
      tabs = tabs.map((t) => (t.id === updatedTab.id ? updatedTab : t));
    } else {
      const blob = new Blob([target.content], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url; a.download = fileName; a.click();
      URL.revokeObjectURL(url);
      tabs = tabs.map((t) => (t.id === target.id ? { ...t, modified: false } : t));
    }
  }

  function doFind() {
    if (!activeTab || !find.query) { find = { ...find, count: 0 }; return; }
    const flags = find.matchCase ? 'g' : 'gi';
    const matches = [...activeTab.content.matchAll(new RegExp(find.query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), flags))];
    find = { ...find, count: matches.length };
    if (matches.length > 0 && textareaEl) {
      textareaEl.focus();
      textareaEl.setSelectionRange(matches[0].index!, matches[0].index! + find.query.length);
    }
  }

  function doReplace() {
    if (!activeTab || !find.query) return;
    const flags = find.matchCase ? 'g' : 'gi';
    const newContent = activeTab.content.replace(new RegExp(find.query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), flags), find.replace);
    updateContent(activeTab.id, newContent);
  }

  function handleCursorMove(e: Event) {
    const t = e.target as HTMLTextAreaElement;
    const before = t.value.substring(0, t.selectionStart ?? 0);
    const lines = before.split('\n');
    cursorPos = { line: lines.length, col: lines[lines.length - 1].length + 1 };
  }

  function insertText(before: string, after = '') {
    const ta = textareaEl;
    if (!ta || !activeTab) return;
    const start = ta.selectionStart, end = ta.selectionEnd;
    const sel = activeTab.content.substring(start, end);
    const newContent = activeTab.content.substring(0, start) + before + sel + after + activeTab.content.substring(end);
    updateContent(activeTab.id, newContent);
    setTimeout(() => { ta.selectionStart = start + before.length; ta.selectionEnd = start + before.length + sel.length; ta.focus(); }, 0);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Tab') { e.preventDefault(); insertText('    '); }
    if (e.ctrlKey && e.key === 's' && !e.shiftKey) { e.preventDefault(); saveFile(); }
    if (e.ctrlKey && e.key === 's' && e.shiftKey) { e.preventDefault(); saveAs(); }
    if (e.ctrlKey && e.key === 'f') { e.preventDefault(); find = { ...find, show: !find.show }; }
  }

  $: if (find.show) tick().then(() => findInputEl?.focus());
</script>

<div class="flex flex-col h-full bg-slate-900 text-white overflow-hidden">
  {#if saveAsTarget}
    <SaveAsDialog defaultName={saveAsTarget.title} on:confirm={handleSaveAsConfirm} on:cancel={() => (saveAsTarget = null)} />
  {/if}

  <div class="shrink-0 flex items-center gap-1 px-3 py-2 bg-slate-800 border-b border-white/5">
    <button on:click={newTab} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="New Tab (Ctrl+T)"><Plus size={15} /></button>
    <button on:click={openFile} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Open file"><FolderOpen size={15} /></button>
    <button on:click={() => saveFile()} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Save (Ctrl+S)"><Save size={15} /></button>
    <button on:click={() => saveAs()} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Save As (Ctrl+Shift+S)"><Download size={15} /></button>
    <div class="w-px h-5 bg-white/10 mx-1" />
    <button on:click={() => insertText('**', '**')} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Bold"><Bold size={15} /></button>
    <button on:click={() => insertText('*', '*')} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Italic"><Italic size={15} /></button>
    <button on:click={() => insertText('# ')} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-white" title="Heading"><Hash size={15} /></button>
    <div class="w-px h-5 bg-white/10 mx-1" />
    <button on:click={() => (find = { ...find, show: !find.show })} class="p-1.5 rounded-lg {find.show ? 'bg-blue-500/20 text-blue-400' : 'hover:bg-white/10 text-slate-400'}" title="Find (Ctrl+F)">
      <Search size={15} />
    </button>
    {#if activeTab?.path}
      <div class="ml-auto text-xs text-slate-600 truncate max-w-[200px] font-mono" title={activeTab.path}>{activeTab.path}</div>
    {/if}
  </div>

  <div class="shrink-0 flex bg-slate-800/50 border-b border-white/5 overflow-x-auto scrollbar-hide">
    {#each tabs as tab (tab.id)}
      <div on:click={() => (activeId = tab.id)}
        class="flex items-center gap-1.5 px-4 py-2 cursor-pointer shrink-0 border-r border-white/5 text-sm group max-w-[150px] {activeId === tab.id ? 'bg-slate-900 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}">
        <FileText size={13} class="shrink-0" />
        <span class="truncate">{tab.title}</span>
        {#if tab.modified}<span class="w-1.5 h-1.5 rounded-full bg-blue-400 shrink-0" />{/if}
        <button on:click={(e) => closeTab(tab.id, e)} class="opacity-0 group-hover:opacity-100 hover:text-red-400 ml-0.5 shrink-0"><X size={11} /></button>
      </div>
    {/each}
    <button on:click={newTab} class="p-2.5 hover:bg-slate-700/50 text-slate-500 hover:text-white shrink-0"><Plus size={13} /></button>
  </div>

  {#if find.show}
    <div class="shrink-0 flex items-center gap-2 px-3 py-2 bg-slate-800/80 border-b border-white/5">
      <div class="relative">
        <Search size={13} class="absolute left-2 top-1/2 -translate-y-1/2 text-slate-500" />
        <input bind:this={findInputEl} bind:value={find.query} on:keydown={(e) => e.key === 'Enter' && doFind()} placeholder="Find..."
          class="bg-slate-900 border border-white/10 rounded-lg pl-7 pr-3 py-1.5 text-sm text-white w-44 focus:outline-none focus:border-blue-500/50" />
      </div>
      <input bind:value={find.replace} placeholder="Replace with..." class="bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white w-40 focus:outline-none" />
      <button on:click={doFind} class="px-2.5 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs transition-colors">Find</button>
      <button on:click={doReplace} class="px-2.5 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs transition-colors">Replace All</button>
      <label class="flex items-center gap-1 text-xs text-slate-400 cursor-pointer">
        <input type="checkbox" bind:checked={find.matchCase} class="accent-blue-500" /> Aa
      </label>
      {#if find.count > 0}<span class="text-xs text-slate-500">{find.count} matches</span>{/if}
      <button on:click={() => (find = { ...find, show: false })} class="ml-auto text-slate-500 hover:text-white"><X size={14} /></button>
    </div>
  {/if}

  <div class="flex-1 overflow-hidden">
    {#if activeTab}
      <textarea
        bind:this={textareaEl}
        value={activeTab.content}
        on:input={(e) => updateContent(activeTab.id, e.currentTarget.value)}
        on:keydown={handleKeyDown}
        on:keyup={handleCursorMove}
        on:click={handleCursorMove}
        spellcheck="false"
        class="w-full h-full bg-slate-950 text-slate-100 font-mono text-sm p-4 resize-none focus:outline-none leading-relaxed"
        placeholder="Start typing…  •  Ctrl+S to save  •  Ctrl+Shift+S for Save As"
        style="font-family:'JetBrains Mono','Fira Code',monospace; tab-size:4;"
      />
    {/if}
  </div>

  <div class="shrink-0 flex items-center justify-between px-4 py-1 bg-slate-800 border-t border-white/5 text-xs text-slate-500">
    <div class="flex gap-4">
      <span>Ln {cursorPos.line}, Col {cursorPos.col}</span>
      <span>{lineCount} lines</span>
      <span>{wordCount} words</span>
      {#if activeTab?.content.length}<span>{activeTab.content.length} chars</span>{/if}
    </div>
    <div class="flex gap-3">
      {#if activeTab?.modified}
        <span class="text-yellow-500 cursor-pointer hover:text-yellow-300" on:click={() => saveFile()} title="Click to save">● Unsaved</span>
      {/if}
      {#if activeTab?.path}
        <span class="text-green-600" title={activeTab.path}>Saved</span>
      {:else}
        <span class="text-slate-600">Not saved to disk</span>
      {/if}
      <span>UTF-8</span>
    </div>
  </div>
</div>
