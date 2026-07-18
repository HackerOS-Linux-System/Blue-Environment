<script lang="ts">
  import { Folder, ChevronRight, ChevronDown, FileCode, Trash2, Edit2 } from 'lucide-svelte';
  import type { FileNode } from './types';
  import { createEventDispatcher } from 'svelte';

  export let nodes: FileNode[];
  export let level = 0;
  export let selectedDir: string;

  const dispatch = createEventDispatcher<{ openFile: string; toggleDir: FileNode; rename: FileNode; delete: FileNode }>();
</script>

{#each nodes as node (node.path)}
  <div>
    <div
      class="flex items-center gap-1 py-0.5 px-1 rounded cursor-pointer hover:bg-white/5 group text-sm {node.type === 'directory' && node.path === selectedDir ? 'bg-blue-600/10' : ''}"
      style="padding-left:{level * 12 + 4}px;"
      on:dblclick={() => node.type === 'file' && dispatch('openFile', node.path)}
      on:click={() => node.type === 'directory' && dispatch('toggleDir', node)}>
      {#if node.type === 'directory'}
        <span class="text-slate-500 w-4 shrink-0">{#if node.expanded}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}</span>
      {/if}
      {#if node.type === 'directory'}<Folder size={14} class="text-blue-400 shrink-0" />{:else}<FileCode size={14} class="text-yellow-400 shrink-0" />{/if}
      <span class="truncate flex-1">{node.name}</span>
      <div class="flex gap-0.5 opacity-0 group-hover:opacity-100 ml-auto shrink-0">
        {#if node.type === 'file'}
          <button on:click|stopPropagation={() => dispatch('openFile', node.path)} class="p-0.5 hover:bg-white/10 rounded text-slate-500" title="Open"><FileCode size={11} /></button>
        {/if}
        <button on:click|stopPropagation={() => dispatch('rename', node)} class="p-0.5 hover:bg-white/10 rounded text-slate-500 hover:text-blue-400" title="Rename"><Edit2 size={11} /></button>
        <button on:click|stopPropagation={() => dispatch('delete', node)} class="p-0.5 hover:bg-white/10 rounded text-slate-500 hover:text-red-400" title="Delete"><Trash2 size={11} /></button>
      </div>
    </div>
    {#if node.type === 'directory' && node.expanded && node.children}
      <svelte:self nodes={node.children} level={level + 1} {selectedDir}
        on:openFile on:toggleDir on:rename on:delete />
    {/if}
  </div>
{/each}
