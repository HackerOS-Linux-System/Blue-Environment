<script lang="ts">
  import { onMount } from 'svelte';
  import { Archive, FolderOpen, Download, File, Folder, AlertCircle, Loader2, Info, Plus } from 'lucide-svelte';
  import { createArchive } from './useArchive';
  import CreateArchiveDialog from './CreateArchiveDialog.svelte';

  import { showContextMenu } from '../../../stores/contextMenu';
  export let openPath: string | undefined = undefined;
  const copyText = (t: string) => navigator.clipboard.writeText(t).catch(() => {});
  function entryMenu(e: MouseEvent, entry: { path: string; is_dir: boolean }) {
    e.stopPropagation();
    showContextMenu(e, [
      { label: 'Copy path inside archive', action: () => copyText(entry.path) },
      { label: 'Copy name', action: () => copyText(entry.path.replace(/\/$/, '').split('/').pop() ?? entry.path) },
      { separator: true },
      { label: 'Extract archive…', disabled: !$archive, action: () => extract($archive) },
    ]);
  }
  function archiveMenu(e: MouseEvent) {
    showContextMenu(e, [
      { label: 'Extract all…', disabled: !$archive || $loading, action: () => extract($archive) },
      { label: 'Copy archive path', disabled: !$archive, action: () => $archive && copyText($archive.path) },
      { separator: true },
      { label: 'Open another archive…', action: openFile },
    ]);
  }
  const { archive, loading, status, error, openFile, extract, openPath: loadPath } = createArchive();
  onMount(() => { if (openPath) loadPath(openPath); });
  let showCreate = false;
  let search = '';
  let sortBy: 'name' | 'size' = 'name';

  $: entries = ($archive?.info.entries ?? [])
    .filter((e) => !search || e.name.toLowerCase().includes(search.toLowerCase()))
    .sort((a, b) => (sortBy === 'size' ? b.size - a.size : a.name.localeCompare(b.name)));

  $: dirs = entries.filter((e) => e.is_dir);
  $: files = entries.filter((e) => !e.is_dir);
</script>

<div class="flex flex-col h-full bg-slate-900 text-white">
  {#if showCreate}
    <CreateArchiveDialog on:close={() => (showCreate = false)} on:created={() => (showCreate = false)} />
  {/if}

  <div class="flex items-center gap-2 px-4 py-2.5 border-b border-white/5 shrink-0 bg-slate-800/50">
    <Archive size={18} class="text-blue-400" />
    <span class="font-semibold text-sm">{$archive?.name || 'Blue Archive'}</span>
    <div class="flex-1" />
    <button on:click={() => (showCreate = true)} class="flex items-center gap-1.5 px-3 py-1.5 bg-orange-600 hover:bg-orange-500 rounded-lg text-xs">
      <Plus size={12} /> New Archive
    </button>
    {#if $archive}
      <button on:click={() => extract($archive)} disabled={$loading} class="flex items-center gap-1.5 px-3 py-1.5 bg-green-600 hover:bg-green-500 rounded-lg text-xs disabled:opacity-50">
        {#if $loading}<Loader2 size={12} class="animate-spin" />{:else}<Download size={12} />{/if} Extract All
      </button>
    {/if}
    <button on:click={openFile} disabled={$loading} class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs disabled:opacity-50">
      <FolderOpen size={12} /> {$archive ? 'Open Another' : 'Open Archive'}
    </button>
  </div>

  {#if $status}<div class="px-4 py-2 text-xs text-green-400 bg-green-500/5 border-b border-green-500/10">{$status}</div>{/if}
  {#if $error}<div class="px-4 py-2 text-xs text-red-400 bg-red-500/5 border-b border-red-500/10 flex gap-2 items-start"><AlertCircle size={12} class="mt-0.5 shrink-0" />{$error}</div>{/if}

  {#if !$archive && !$loading}
    <div class="flex-1 flex flex-col items-center justify-center gap-4 text-center">
      <Archive size={48} class="text-slate-600" />
      <p class="text-slate-400 text-sm">Open a .zip, .tar.gz, .7z or similar archive to inspect its contents.</p>
      <button on:click={openFile} class="px-5 py-2.5 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">Open Archive</button>
      <p class="text-xs text-slate-600">Supports tar, zip, 7z and more via system tools</p>
    </div>
  {/if}

  {#if $loading}
    <div class="flex-1 flex items-center justify-center"><Loader2 size={24} class="animate-spin text-blue-400" /></div>
  {/if}

  {#if $archive && !$loading}
    <div class="flex-1 flex flex-col overflow-hidden">
      <div class="flex items-center gap-3 px-4 py-2 border-b border-white/5 shrink-0">
        <div class="flex items-center gap-1.5 text-xs text-slate-400"><Info size={12} />{$archive.info.total_files} items</div>
        <input bind:value={search} placeholder="Filter files…" class="flex-1 bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-white focus:outline-none" />
        <select bind:value={sortBy} class="bg-slate-800 border border-white/10 rounded px-2 py-1 text-xs text-slate-300 focus:outline-none">
          <option value="name">Sort: Name</option>
          <option value="size">Sort: Size</option>
        </select>
      </div>

      <div class="flex-1 overflow-y-auto p-2 space-y-0.5" on:contextmenu={archiveMenu} role="presentation">
        {#each dirs as e, i (i)}
          <div on:contextmenu={(ev) => entryMenu(ev, e)} role="presentation" class="flex items-center gap-2 px-2 py-1.5 hover:bg-white/5 rounded">
            <Folder size={14} class="text-blue-400 shrink-0" />
            <span class="text-sm text-slate-300 truncate flex-1">{e.path}</span>
          </div>
        {/each}
        {#each files as e, i (i)}
          <div on:contextmenu={(ev) => entryMenu(ev, e)} role="presentation" class="flex items-center gap-2 px-2 py-1.5 hover:bg-white/5 rounded">
            <File size={14} class="text-yellow-400 shrink-0" />
            <span class="text-sm text-slate-300 truncate flex-1">{e.path}</span>
            <span class="text-xs text-slate-600 shrink-0">{e.size > 0 ? `${(e.size / 1024).toFixed(1)} KB` : ''}</span>
          </div>
        {/each}
        {#if entries.length === 0}
          <div class="text-center py-8 text-slate-600 text-sm">No matching files</div>
        {/if}
      </div>
    </div>
  {/if}
</div>
