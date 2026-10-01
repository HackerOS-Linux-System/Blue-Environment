<script lang="ts">
  import { Folder, ChevronRight, ChevronDown, FileCode, Trash2, Edit2 } from 'lucide-svelte';
  import type { FileNode } from './types';
  import { createEventDispatcher } from 'svelte';
  import { blueCodeSettings } from './blueCodeSettings';
  import { showContextMenu } from '../../../stores/contextMenu';
  import { FilePlus as FilePlusI, FolderPlus as FolderPlusI, Edit2 as EditI, Trash2 as TrashI, Link2 as LinkI, FileCode as FileCodeI, RefreshCw as RefreshI } from 'lucide-svelte';

  function nodeMenu(e: MouseEvent, node: FileNode) {
    e.stopPropagation();
    const copy = (t: string) => navigator.clipboard.writeText(t).catch(() => {});
    const rel = rootPath && node.path.startsWith(rootPath) ? node.path.slice(rootPath.length).replace(/^\//, '') : node.path;
    showContextMenu(e, [
      ...(node.type === 'file' ? [{ label: 'Open', icon: FileCodeI, action: () => dispatch('openFile', node.path) }] : []),
      { label: 'New file…', icon: FilePlusI, action: () => dispatch('newFile') },
      { label: 'New folder…', icon: FolderPlusI, action: () => dispatch('newFolder') },
      { separator: true },
      { label: 'Rename…', icon: EditI, shortcut: 'F2', action: () => dispatch('rename', node) },
      { label: 'Delete', icon: TrashI, danger: true, action: () => dispatch('delete', node) },
      { separator: true },
      { label: 'Copy path', icon: LinkI, action: () => copy(node.path) },
      { label: 'Copy relative path', icon: LinkI, action: () => copy(rel) },
    ]);
  }

  export let nodes: FileNode[];
  export let level = 0;
  export let selectedDir: string;
  /** relative-path (from the workspace root) → single-letter git status
   * ('M'/'A'/'D'/'U'/'?') — see Sidebar.svelte's `gitStatusMap`, built
   * from `git_repo_status` (BlueCodeApp/git.rs). Empty object when the
   * workspace isn't a git repo or hasn't loaded yet; every lookup below
   * is `?? undefined`-safe for that case. */
  export let gitStatus: Record<string, string> = {};
  export let rootPath = '';

  const dispatch = createEventDispatcher<{ openFile: string; toggleDir: FileNode; rename: FileNode; delete: FileNode; newFile: void; newFolder: void; refresh: void }>();

  const STATUS_COLOR: Record<string, string> = {
    M: 'text-yellow-400', A: 'text-green-400', D: 'text-red-400', U: 'text-orange-400', '?': 'text-slate-500',
  };
  const STATUS_TITLE: Record<string, string> = {
    M: 'Modified', A: 'Added', D: 'Deleted', U: 'Conflicted', '?': 'Untracked',
  };

  // Directories don't have their own git status line, but VS Code-style
  // tools still tint a folder's name when *something* inside it changed
  // — helps spot which subtree to look in without expanding every
  // folder. Cheap enough as a plain prefix scan given typical project
  // sizes; not memoized since Svelte only reruns this on `gitStatus`/
  // `nodes` changes anyway.
  function relativePath(path: string): string {
    if (!rootPath) return path;
    return path.startsWith(rootPath) ? path.slice(rootPath.length).replace(/^\/+/, '') : path;
  }
  function dirHasChanges(dirRelPath: string): boolean {
    const prefix = dirRelPath ? `${dirRelPath}/` : '';
    return Object.keys(gitStatus).some((p) => p.startsWith(prefix));
  }
</script>

{#each nodes as node (node.path)}
  <div>
    <div
      class="flex items-center gap-1 py-0.5 px-1 rounded cursor-pointer hover:bg-white/5 group text-sm {node.type === 'directory' && node.path === selectedDir ? 'bg-blue-600/10' : ''}"
      style="padding-left:{level * 12 + 4}px;"
      on:contextmenu={(e) => nodeMenu(e, node)}
      on:dblclick={() => node.type === 'file' && dispatch('openFile', node.path)}
      on:click={() => {
        if (node.type === 'directory') dispatch('toggleDir', node);
        // Default: one click opens the file. A double-click then re-fires
        // openFile for an already-open path, which is a harmless tab switch.
        else if ($blueCodeSettings.openOnSingleClick) dispatch('openFile', node.path);
      }}
      role="button" tabindex="0"
      on:keydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          if (node.type === 'directory') dispatch('toggleDir', node); else dispatch('openFile', node.path);
        }
      }}>
      {#if node.type === 'directory'}
        <span class="text-slate-500 w-4 shrink-0">{#if node.expanded}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}</span>
      {/if}
      {#if node.type === 'directory'}
        <Folder size={14} class="shrink-0 {dirHasChanges(relativePath(node.path)) ? 'text-yellow-400' : 'text-blue-400'}" />
      {:else}<FileCode size={14} class="text-yellow-400 shrink-0" />{/if}
      <span class="truncate flex-1 {node.type === 'file' && gitStatus[relativePath(node.path)] ? STATUS_COLOR[gitStatus[relativePath(node.path)]] : ''}">{node.name}</span>
      {#if node.type === 'file' && gitStatus[relativePath(node.path)]}
        <span class="text-[10px] font-bold w-3 text-center shrink-0 {STATUS_COLOR[gitStatus[relativePath(node.path)]]}" title={STATUS_TITLE[gitStatus[relativePath(node.path)]]}>
          {gitStatus[relativePath(node.path)]}
        </span>
      {/if}
      <div class="flex gap-0.5 opacity-0 group-hover:opacity-100 ml-auto shrink-0">
        {#if node.type === 'file'}
          <button on:click|stopPropagation={() => dispatch('openFile', node.path)} class="p-0.5 hover:bg-white/10 rounded text-slate-500" title="Open"><FileCode size={11} /></button>
        {/if}
        <button on:click|stopPropagation={() => dispatch('rename', node)} class="p-0.5 hover:bg-white/10 rounded text-slate-500 hover:text-blue-400" title="Rename"><Edit2 size={11} /></button>
        <button on:click|stopPropagation={() => dispatch('delete', node)} class="p-0.5 hover:bg-white/10 rounded text-slate-500 hover:text-red-400" title="Delete"><Trash2 size={11} /></button>
      </div>
    </div>
    {#if node.type === 'directory' && node.expanded && node.children}
      <svelte:self nodes={node.children} level={level + 1} {selectedDir} {gitStatus} {rootPath}
        on:openFile on:toggleDir on:rename on:delete on:newFile on:newFolder on:refresh />
    {/if}
  </div>
{/each}
