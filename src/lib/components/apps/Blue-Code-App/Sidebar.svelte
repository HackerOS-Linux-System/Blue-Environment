<script lang="ts">
  import { RefreshCw, FolderOpen, FolderPlus, FilePlus, AlertCircle, AlertTriangle } from 'lucide-svelte';
  import { onDestroy } from 'svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { showContextMenu } from '../../../stores/contextMenu';

  function treeBackgroundMenu(e: MouseEvent) {
    showContextMenu(e, [
      { label: 'New file…', icon: FilePlus, action: () => tree.createFile() },
      { label: 'New folder…', icon: FolderPlus, action: () => tree.createFolder() },
      { label: 'Open folder…', icon: FolderOpen, action: () => tree.openWorkspace() },
      { separator: true },
      { label: 'Refresh', icon: RefreshCw, action: () => tree.loadTree($rootPath) },
    ]);
  }
  import GitPanel from '../../GitPanel.svelte';
  import type { FileTreeState } from './fileTree';
  import type { EditorFilesState } from './editorFiles';
  import type { SidebarTab } from './types';
  import FileTreeView from './FileTreeView.svelte';
  import DevServerPanel from './DevServerPanel.svelte';

  export let tree: FileTreeState;
  export let editor: EditorFilesState;
  export let sidebarTab: SidebarTab;

  const { rootPath, fileTree, isLoading, selectedDir } = tree;
  const { openFiles, diagnostics, totalErrors, totalWarnings } = editor;

  // Git-changed-file indicators in the file tree (colored letter next to
  // each modified/added/untracked file — see FileTreeView.svelte's
  // `gitStatus` prop) — uses the structured `git_repo_status` command
  // (BlueCodeApp/git.rs) rather than GitPanel's own shell-based
  // `git status --porcelain` parsing, since this needs to run on every
  // tree refresh/file save, not just when the Git sidebar tab is open.
  let gitStatusMap: Record<string, string> = {};
  async function refreshGitStatus(root: string) {
    if (!root) { gitStatusMap = {}; return; }
    try {
      const status = await SystemBridge.invokeCommand<{ is_repo: boolean; files: { path: string; state: string; staged: boolean }[] }>('git_repo_status', { path: root });
      if (!status.is_repo) { gitStatusMap = {}; return; }
      const STATE_LETTER: Record<string, string> = { Modified: 'M', Added: 'A', Deleted: 'D', Renamed: 'M', Untracked: '?', Conflicted: 'U' };
      const map: Record<string, string> = {};
      for (const f of status.files) map[f.path] = STATE_LETTER[f.state] ?? 'M';
      gitStatusMap = map;
    } catch { gitStatusMap = {}; }
  }
  $: refreshGitStatus($rootPath);
  // Re-check after every save, not just on tree refresh — a save is the
  // single most common moment a file's git status actually changes.
  const unsubOpenFilesForGit = openFiles.subscribe(() => refreshGitStatus($rootPath));
  onDestroy(unsubOpenFilesForGit);

  const TABS: SidebarTab[] = ['files', 'search', 'git', 'problems', 'dev'];

  let searchTerm = '';
  let searchResults: { file: string; line: number; content: string }[] = [];
  let replaceTerm = '';
  let replaceStatus: string | null = null;
  let replacing = false;
  let activeSearch: ReturnType<typeof import('./search').createSearch> | null = null;

  async function runSearch() {
    // Local mini-search kept separate to avoid a circular import with
    // search.ts's own rootPath-store dependency; this mirrors
    // createSearch() exactly for the sidebar's own input field.
    const { createSearch } = await import('./search');
    const s = createSearch(rootPath);
    activeSearch = s; // kept so runReplace (below) can reuse the same result set
    replaceStatus = null;
    s.searchTerm.set(searchTerm);
    await s.searchFiles();
    const unsub = s.searchResults.subscribe((r) => (searchResults = r));
    unsub();
  }

  async function runReplace() {
    if (!activeSearch || !searchTerm || replacing) return;
    replacing = true;
    replaceStatus = null;
    try {
      const { files, occurrences } = await activeSearch.replaceAll(replaceTerm);
      replaceStatus = occurrences > 0
        ? `Replaced ${occurrences} occurrence${occurrences === 1 ? '' : 's'} in ${files} file${files === 1 ? '' : 's'}.`
        : 'Nothing to replace.';
      const unsub = activeSearch.searchResults.subscribe((r) => (searchResults = r));
      unsub();
    } finally {
      replacing = false;
    }
  }

  async function handleRename(node: any) {
    const newPath = await tree.renameNode(node);
    if (newPath) editor.renameOpenFile(node.path, newPath);
  }
  async function handleDelete(node: any) {
    const ok = await tree.deleteNode(node);
    if (ok) {
      const idx = $openFiles.findIndex((f) => f.path === node.path);
      if (idx >= 0) editor.closeFile(idx);
    }
  }

  // Grouped for the Problems tab — computed here rather than inline in
  // the template so TypeScript can infer the reduce's accumulator type
  // properly instead of defaulting to `{}`/`any`.
  $: problemsByFile = $diagnostics.reduce<Record<string, typeof $diagnostics>>((acc, d) => {
    (acc[d.file] ??= []).push(d);
    return acc;
  }, {});
</script>

<div class="w-56 bg-slate-800/50 border-r border-white/5 flex flex-col overflow-hidden">
  <div class="flex border-b border-white/5 shrink-0">
    {#each TABS as tab (tab)}
      <button on:click={() => (sidebarTab = tab)} class="flex-1 py-1.5 text-xs capitalize transition-colors relative {sidebarTab === tab ? 'bg-slate-900 text-white border-b-2 border-blue-500' : 'text-slate-500 hover:text-white'}">
        {tab}
        {#if tab === 'problems' && ($totalErrors + $totalWarnings) > 0}
          <span class="absolute top-0.5 right-1 min-w-[14px] h-[14px] px-0.5 rounded-full text-[9px] leading-[14px] {$totalErrors > 0 ? 'bg-red-500' : 'bg-yellow-500'} text-white">{$totalErrors + $totalWarnings}</span>
        {/if}
      </button>
    {/each}
  </div>

  {#if sidebarTab === 'files'}
    <div class="flex-1 overflow-y-auto p-1" on:contextmenu={treeBackgroundMenu} role="presentation">
      <div class="flex items-center justify-between px-2 py-1 mb-1">
        <span class="text-[10px] font-semibold text-slate-500 uppercase tracking-wider truncate" title={$rootPath}>{$rootPath.split('/').pop() || $rootPath || 'Explorer'}</span>
        <div class="flex gap-0.5 shrink-0">
          <button on:click={() => tree.createFile()} class="p-0.5 hover:bg-white/10 rounded text-slate-500" title="New File"><FilePlus size={12} /></button>
          <button on:click={() => tree.createFolder()} class="p-0.5 hover:bg-white/10 rounded text-slate-500" title="New Folder"><FolderPlus size={12} /></button>
          <button on:click={tree.openWorkspace} class="p-0.5 hover:bg-white/10 rounded text-slate-500" title="Open Folder"><FolderOpen size={12} /></button>
          <button on:click={() => tree.loadTree($rootPath)} class="p-0.5 hover:bg-white/10 rounded text-slate-500" title="Refresh"><RefreshCw size={11} /></button>
        </div>
      </div>
      {#if $isLoading}
        <div class="text-center py-4 text-slate-500 text-xs">Loading…</div>
      {:else if $fileTree.length === 0}
        <div class="text-center py-6 px-2 text-slate-600 text-xs">Empty workspace.<br />Use the icons above to create a file or folder.</div>
      {:else}
        <FileTreeView nodes={$fileTree} selectedDir={$selectedDir} gitStatus={gitStatusMap} rootPath={$rootPath}
          on:openFile={(e) => editor.openFile(e.detail)}
          on:toggleDir={(e) => tree.toggleDir(e.detail)}
          on:rename={(e) => handleRename(e.detail)}
          on:delete={(e) => handleDelete(e.detail)}
          on:newFile={() => tree.createFile()}
          on:newFolder={() => tree.createFolder()}
          on:refresh={() => tree.loadTree($rootPath)} />
      {/if}
    </div>
  {/if}

  {#if sidebarTab === 'search'}
    <div class="flex-1 overflow-y-auto p-2">
      <div class="flex gap-1 mb-1.5">
        <input type="text" bind:value={searchTerm} on:keydown={(e) => e.key === 'Enter' && runSearch()} placeholder="Search…"
          class="flex-1 bg-slate-900 border border-white/10 rounded px-2 py-1 text-xs focus:outline-none focus:border-blue-500/50" />
        <button on:click={runSearch} class="px-2 py-1 bg-blue-600 hover:bg-blue-500 rounded text-xs">Go</button>
      </div>
      <div class="flex gap-1 mb-2">
        <input type="text" bind:value={replaceTerm} on:keydown={(e) => e.key === 'Enter' && runReplace()} placeholder="Replace with…"
          class="flex-1 bg-slate-900 border border-white/10 rounded px-2 py-1 text-xs focus:outline-none focus:border-blue-500/50" />
        <button on:click={runReplace} disabled={!activeSearch || !searchTerm || searchResults.length === 0 || replacing}
          class="px-2 py-1 bg-slate-700 hover:bg-slate-600 disabled:opacity-30 rounded text-xs whitespace-nowrap">
          {replacing ? '…' : 'Replace All'}
        </button>
      </div>
      {#if replaceStatus}<div class="text-[10px] text-green-400 mb-2">{replaceStatus}</div>{/if}
      <div class="space-y-1">
        {#each searchResults as r, i (i)}
          <div on:click={() => editor.openFile(r.file)} class="cursor-pointer hover:bg-white/5 rounded p-1">
            <div class="text-[10px] text-blue-400 truncate">{r.file.split('/').pop()}:{r.line}</div>
            <div class="text-[10px] text-slate-400 truncate">{r.content}</div>
          </div>
        {/each}
        {#if searchResults.length === 0 && searchTerm}<div class="text-xs text-slate-600 text-center py-4">No results</div>{/if}
      </div>
    </div>
  {/if}

  {#if sidebarTab === 'git'}
    <div class="flex-1 overflow-hidden"><GitPanel cwd={$rootPath} /></div>
  {/if}

  {#if sidebarTab === 'problems'}
    <div class="flex-1 overflow-y-auto p-1">
      {#if $diagnostics.length === 0}
        <div class="text-center py-6 px-2 text-slate-600 text-xs">No problems detected across open files.</div>
      {:else}
        {#each Object.entries(problemsByFile) as [file, diags] (file)}
          <div class="mb-1">
            <div class="px-2 py-1 text-[10px] font-semibold text-slate-500 truncate" title={file}>{file.split('/').pop()}</div>
            {#each diags as d, i (i)}
              <div on:click={() => editor.openFileAtLine(file, d.line)}
                class="flex items-start gap-1.5 px-3 py-1 hover:bg-white/5 cursor-pointer rounded">
                {#if d.severity === 'error'}
                  <AlertCircle size={11} class="text-red-400 shrink-0 mt-0.5" />
                {:else}
                  <AlertTriangle size={11} class="text-yellow-400 shrink-0 mt-0.5" />
                {/if}
                <div class="min-w-0">
                  <div class="text-[11px] text-slate-300 truncate">{d.message}</div>
                  <div class="text-[10px] text-slate-600">Line {d.line}, Col {d.col}</div>
                </div>
              </div>
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  {/if}

  {#if sidebarTab === 'dev'}
    <DevServerPanel {rootPath} />
  {/if}
</div>
