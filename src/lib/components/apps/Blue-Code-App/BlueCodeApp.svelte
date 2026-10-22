<script lang="ts">
  import { run, stopPropagation } from 'svelte/legacy';

  import { onMount, onDestroy } from 'svelte';
  import { Code2, Plus, AlertCircle, FileCode, X } from 'lucide-svelte';
  import { createFileTree } from './fileTree';
  import { createEditorFiles } from './editorFiles';
  import type { SidebarTab, CommandEntry } from './types';

  import Toolbar from './Toolbar.svelte';
  import Sidebar from './Sidebar.svelte';
  import StatusBar from './StatusBar.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import QuickOpen from './QuickOpen.svelte';
  import TerminalPane from './TerminalPane.svelte';
  import CodeEditor from './CodeEditor.svelte';
  import type { EditorHandle } from './editorHandle';
  import { languageLabel } from './cmLanguages';
  import SettingsPanel from './SettingsPanel.svelte';
  import { blueCodeSettings } from './blueCodeSettings';

  
  interface Props {
    windowId: string;
    /** Set when launched from Explorer with "open this file" — see
   * ExplorerApp.svelte's handleOpen and windowManager.ts's launchArgs. */
    openPath?: string | undefined;
  }

  let { windowId, openPath = undefined }: Props = $props();

  const tree = createFileTree();
  const editor = createEditorFiles(tree.rootPath);
  const { rootPath, fileTree } = tree;
  const { openFiles, activeIdx, activeFile, fileDiagnostics, errors, warnings, lspStatus, editorTheme, fontSize, cursorPos, revealLine } = editor;

  onMount(() => {
    if (openPath) editor.openFile(openPath);

    // Window-level fallback for Ctrl+P/Ctrl+Shift+P: the editor's own
    // keymap only exists once a file is open (the editor isn't mounted
    // at all on the empty-workspace
    // screen) — without this, Quick Open would be unreachable by
    // keyboard until the person opens a file some other way first.
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === ',') { e.preventDefault(); showSettings = true; return; }
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'p') return;
      e.preventDefault(); // always: also stops the browser's own print dialog on Ctrl+P
      if (e.shiftKey) showCommandPalette = true; else showQuickOpen = true;
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  });

  let sidebarTab = $state<SidebarTab>('files');
  let sidebarCollapsed = $state(false);
  let showTerminal = $state($blueCodeSettings.showTerminalOnStart);
  let showSettings = $state(false);
  let showCommandPalette = $state(false);
  let showQuickOpen = $state(false);
  let commandInput = $state('');
  let editorRef = $state<EditorHandle | undefined>();

  async function newFile() {
    const path = await tree.createFile();
    if (path) await editor.openFile(path);
  }

  let commands = $derived([
    { id: 'settings', label: 'Blue Code Settings…', shortcut: 'Ctrl+,', action: () => (showSettings = true) },
    { id: 'single-click', label: 'Toggle: Open files with single click', shortcut: '', action: () => blueCodeSettings.patch({ openOnSingleClick: !$blueCodeSettings.openOnSingleClick }) },
    { id: 'save', label: 'Save File', shortcut: 'Ctrl+S', action: () => editor.saveFile($activeIdx) },
    { id: 'saveAll', label: 'Save All', shortcut: 'Ctrl+Shift+S', action: editor.saveAll },
    { id: 'newFile', label: 'New File', shortcut: 'Ctrl+N', action: newFile },
    { id: 'newFolder', label: 'New Folder', shortcut: '', action: () => tree.createFolder() },
    { id: 'openFolder', label: 'Open Folder…', shortcut: 'Ctrl+K Ctrl+O', action: tree.openWorkspace },
    { id: 'terminal', label: 'Toggle Terminal', shortcut: 'Ctrl+`', action: () => (showTerminal = !showTerminal) },
    { id: 'sidebar', label: 'Toggle Sidebar', shortcut: 'Ctrl+B', action: () => (sidebarCollapsed = !sidebarCollapsed) },
    { id: 'refresh', label: 'Refresh Explorer', shortcut: '', action: () => tree.loadTree($rootPath) },
    { id: 'lint', label: 'Run Linter', shortcut: '', action: () => editor.runLint($activeIdx) },
    { id: 'search', label: 'Search in Files', shortcut: 'Ctrl+F', action: () => (sidebarTab = 'search') },
    { id: 'dev', label: 'Active Dev Mode', shortcut: '', action: () => (sidebarTab = 'dev') },
    { id: 'theme-dark', label: 'Theme: Blue Dark', shortcut: '', action: () => editorTheme.set('blue-dark') },
    { id: 'theme-light', label: 'Theme: Blue Light', shortcut: '', action: () => editorTheme.set('blue-light') },
    { id: 'theme-ocean', label: 'Theme: Blue Ocean', shortcut: '', action: () => editorTheme.set('blue-ocean') },
    { id: 'font-inc', label: 'Increase Font Size', shortcut: 'Ctrl++', action: () => fontSize.update((s) => Math.min(s + 1, 28)) },
    { id: 'font-dec', label: 'Decrease Font Size', shortcut: 'Ctrl+-', action: () => fontSize.update((s) => Math.max(s - 1, 10)) },
    { id: 'format', label: 'Format Document', shortcut: 'Shift+Alt+F', action: () => { editorRef?.format().then((ok) => { if (ok === false) toastNoFormatter(); }).catch(() => toastNoFormatter()); } },
    { id: 'go-line', label: 'Go to Line…', shortcut: 'Ctrl+G', action: () => editorRef?.goToLine() },
    { id: 'fold-all', label: 'Fold All', shortcut: '', action: () => editorRef?.foldAll() },
    { id: 'unfold-all', label: 'Unfold All', shortcut: '', action: () => editorRef?.unfoldAll() },
    { id: 'select-next', label: 'Select Next Occurrence', shortcut: 'Ctrl+D', action: () => editorRef?.selectNextOccurrence() },
    { id: 'find-file', label: 'Search in Files', shortcut: 'Ctrl+Shift+F', action: () => { sidebarTab = 'search'; sidebarCollapsed = false; } },
    { id: 'quick-open', label: 'Go to File…', shortcut: 'Ctrl+P', action: () => (showQuickOpen = true) },
    { id: 'close-tab', label: 'Close Active Tab', shortcut: 'Ctrl+W', action: () => editor.closeFile($activeIdx) },
  ] as CommandEntry[]);

  function toastNoFormatter() {
    // No formatter for this language (or the document has a syntax error) — silently ignored,
    // the status bar / Problems panel already tell the person what is wrong.
  }

  function handleEditorMount(handle: EditorHandle) {
    editorRef = handle;
    editor.setEditorHandle(handle);

    // Problems panel jump-to-line: revealLine is set by editor.
    // openFileAtLine() and may arrive either before or after this mount
    // (the target file's {#key $activeIdx} block and this handler race
    // against each other) — consuming it here, right after mount,
    // covers the "file wasn't already open" case; the reactive block
    // below covers the "file was already open, just switching tabs" case.
    if ($revealLine !== null) {
      handle.revealLine($revealLine);
      revealLine.set(null);
    }
  }

  // Covers switching to an *already-open* file via a Problems click,
  // where CodeEditor doesn't remount (no {#key} change) and so
  // handleEditorMount above never re-fires.
  run(() => {
    if ($revealLine !== null && editorRef && $activeFile) {
      editorRef.revealLine($revealLine);
      revealLine.set(null);
    }
  });
</script>

<div class="flex flex-col h-full bg-slate-900 text-white text-sm relative">
  <Toolbar
    {sidebarCollapsed} canSave={!!$activeFile?.modified} {showTerminal} {sidebarTab}
    activeFile={$activeFile} lspStatus={$lspStatus}
    on:toggleSidebar={() => (sidebarCollapsed = !sidebarCollapsed)}
    on:newFile={newFile}
    on:save={() => editor.saveFile($activeIdx)}
    on:toggleTerminal={() => (showTerminal = !showTerminal)}
    on:setTab={(e) => (sidebarTab = e.detail)}
    on:commandPalette={() => (showCommandPalette = true)}
    on:refresh={() => tree.loadTree($rootPath)}
  />

  <div class="flex flex-1 overflow-hidden">
    {#if !sidebarCollapsed}
      <Sidebar {tree} {editor} bind:sidebarTab />
    {/if}

    <div class="flex-1 flex flex-col overflow-hidden">
      <div class="flex bg-slate-800 border-b border-white/5 overflow-x-auto shrink-0">
        {#each $openFiles as file, idx (idx)}
          <div onclick={() => activeIdx.set(idx)} role="button" tabindex="0" onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => activeIdx.set(idx))(); } }}
            class="flex items-center gap-1.5 px-3 py-2 cursor-pointer border-b-2 shrink-0 group max-w-[160px] {$activeIdx === idx ? 'border-blue-500 text-white bg-slate-900' : 'border-transparent text-slate-400 hover:text-white'}">
            <FileCode size={13} />
            <span class="text-xs truncate">{file.path.split('/').pop()}</span>
            {#if file.modified}<span class="w-1.5 h-1.5 bg-yellow-400 rounded-full shrink-0"></span>{/if}
            <button onclick={stopPropagation(() => editor.closeFile(idx))} class="p-0.5 hover:bg-white/10 rounded opacity-0 group-hover:opacity-100 shrink-0"><X size={11} /></button>
          </div>
        {/each}
      </div>

      {#if $fileDiagnostics.length > 0}
        <div class="shrink-0 flex items-center gap-3 px-3 py-1 bg-red-500/5 border-b border-red-500/10 text-xs">
          {#if $errors > 0}<span class="flex items-center gap-1 text-red-400"><AlertCircle size={12} /> {$errors} error{$errors > 1 ? 's' : ''}</span>{/if}
          {#if $warnings > 0}<span class="flex items-center gap-1 text-yellow-400"><AlertCircle size={12} /> {$warnings} warning{$warnings > 1 ? 's' : ''}</span>{/if}
          <span class="text-slate-600 text-[10px] ml-auto">{$fileDiagnostics[0].message.slice(0, 60)}</span>
        </div>
      {/if}

      <div class="flex-1 overflow-hidden">
        {#if $activeFile}
          {#key $activeIdx}
            <CodeEditor
              language={$activeFile.language} value={$activeFile.content}
              theme={$editorTheme} fontSize={$fontSize}
              lsp={editor.lsp} lspVersion={$lspStatus}
              onchange={(v) => editor.handleEditorChange(v)}
              onready={handleEditorMount}
              oncursor={(p) => editor.setCursorPos(p)}
              onsave={() => editor.saveFile($activeIdx)}
              onsaveall={editor.saveAll}
              oncommandpalette={() => (showCommandPalette = true)}
              onquickopen={() => (showQuickOpen = true)}
            />
          {/key}
        {:else}
          <div class="flex flex-col items-center justify-center h-full text-slate-600 gap-4">
            <Code2 size={40} />
            <div class="text-sm">Open a file or create a new one</div>
            <div class="flex gap-2">
              <button onclick={newFile} class="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm text-white"><Plus size={14} /> New File</button>
              <button onclick={tree.openWorkspace} class="flex items-center gap-2 px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded-xl text-sm text-white">Open Folder</button>
            </div>
          </div>
        {/if}
      </div>

      {#if showTerminal}
        <div class="h-44 bg-slate-900 border-t border-white/5 flex flex-col shrink-0">
          <div class="flex items-center justify-between px-3 py-1 border-b border-white/5 bg-slate-800/50">
            <span class="text-xs text-slate-400">Terminal</span>
            <button onclick={() => (showTerminal = false)} class="p-0.5 hover:bg-white/10 rounded text-slate-500"><X size={12} /></button>
          </div>
          <div class="flex-1 overflow-hidden"><TerminalPane {windowId} /></div>
        </div>
      {/if}
    </div>
  </div>

  <StatusBar
    languageLabel={$activeFile ? languageLabel($activeFile.language) : '—'}
    line={$cursorPos.line} col={$cursorPos.col} errors={$errors} warnings={$warnings}
    editorTheme={$editorTheme} fontSize={$fontSize}
    on:toggleTheme={() => editorTheme.update((t) => (t === 'blue-dark' ? 'blue-light' : 'blue-dark'))}
    on:fontSize={(e) => fontSize.set(e.detail)}
    on:settings={() => (showSettings = true)}
  />

  <SettingsPanel visible={showSettings} editorTheme={$editorTheme} fontSize={$fontSize}
    on:close={() => (showSettings = false)}
    on:theme={(e) => editorTheme.set(e.detail)}
    on:fontSize={(e) => fontSize.set(e.detail)} />

  <CommandPalette visible={showCommandPalette} input={commandInput} {commands}
    on:close={() => { showCommandPalette = false; commandInput = ''; }}
    on:input={(e) => (commandInput = e.detail)} />

  <QuickOpen visible={showQuickOpen} fileTree={$fileTree}
    on:open={(e) => { editor.openFile(e.detail); showQuickOpen = false; }}
    on:close={() => (showQuickOpen = false)} />
</div>
