<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import {
    Folder, HardDrive, ArrowLeft, ArrowRight, RefreshCw,
    Plus, Trash2, Copy, Clipboard, Scissors, Home, Image,
    Music, Video, FileText, Download, ChevronRight,
    Grid, List, Eye, X, Edit, Search,
    SortAsc, SortDesc, Columns, Info, Check, AlertCircle, Loader2,
    Star, StarOff, Archive as ArchiveIcon, FileBox,
  } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { dialogPrompt, dialogConfirm } from '../../../stores/dialog';
  import { configStore } from '../../../utils/configStore';
  import { openApp } from '../../../stores/windowManager';
  import { AppId } from '../../../types';
  import type { FileEntry, Tab, Notif, SortKey } from './types';
  import { BOOKMARKS } from './types';
  import FileIcon from './FileIcon.svelte';
  import RightPane from './RightPane.svelte';

  const BM_ICONS: Record<string, any> = { Home, Folder, FileText, Download, Image, Music, Video };
  const BOOKMARKS_WITH_ICONS = BOOKMARKS.map((bm) => ({ ...bm, icon: BM_ICONS[bm.iconName] ?? Folder }));

  // --- User-defined bookmarks (zakładki użytkownika) -------------------------------------
  let customBookmarks: string[] = configStore.get().customBookmarks ?? [];
  configStore.subscribe((cfg) => { customBookmarks = cfg.customBookmarks ?? []; });

  function isCustomBookmark(path: string) { return customBookmarks.includes(path); }

  async function toggleCustomBookmark(path: string) {
    const next = isCustomBookmark(path)
      ? customBookmarks.filter((p) => p !== path)
      : [...customBookmarks, path];
    customBookmarks = next;
    await configStore.save({ customBookmarks: next });
  }

  // --- Archive preview (podgląd zawartości archiwum bez osobnej appki) ------------------
  const ARCHIVE_EXT = ['zip', 'tar', 'gz', 'bz2', 'xz', '7z', 'rar', 'zst', 'lz4', 'tgz'];
  function isArchive(file: FileEntry) {
    const ext = file.name.split('.').pop()?.toLowerCase() ?? '';
    return file.mime_type.includes('zip') || file.mime_type.includes('archive') || file.mime_type.includes('compressed') || ARCHIVE_EXT.includes(ext);
  }
  let archiveEntries: { name: string; size?: string; is_dir?: boolean }[] | null = null;
  let archiveLoading = false;
  let archiveError = '';
  async function loadArchivePreview(file: FileEntry) {
    archiveEntries = null; archiveError = ''; archiveLoading = true;
    try {
      const info = await SystemBridge.invokeCommand<any>('archive_list', { path: file.path });
      archiveEntries = info?.entries ?? info ?? [];
      if (info?.error) archiveError = info.error;
    } catch (e: any) {
      archiveError = e?.message ?? String(e);
    } finally {
      archiveLoading = false;
    }
  }

  // --- Properties panel (właściwości pliku) ----------------------------------------------
  let propertiesFile: FileEntry | null = null;
  function openProperties(file: FileEntry) { propertiesFile = file; }
  function closeProperties() { propertiesFile = null; }

  // --- Right-click context menu -----------------------------------------------------------
  let ctxMenu: { x: number; y: number; file: FileEntry } | null = null;
  function openContextMenu(e: MouseEvent, file: FileEntry) {
    e.preventDefault();
    if (!selected.has(file.path)) selected = new Set([file.path]);
    ctxMenu = { x: e.clientX, y: e.clientY, file };
  }
  function closeContextMenu() { ctxMenu = null; }

  let tabs: Tab[] = [{ id: 'tab-1', path: 'HOME', history: ['HOME'], historyIndex: 0 }];
  let activeTabId = 'tab-1';
  let dualPane = false;
  let rightPath = 'HOME';

  let files: FileEntry[] = [];
  let loading = false;
  let selected = new Set<string>();
  let lastSel: string | null = null;

  let viewMode: 'grid' | 'list' = 'grid';
  let showHidden = false;
  let sortBy: SortKey = 'name';
  let sortAsc = true;
  let searchTerm = '';
  let showSearch = false;

  let clipboard: { action: 'copy' | 'cut'; files: string[] } | null = null;
  let previewFile: FileEntry | null = null;
  let previewContent = '';
  let previewLoading = false;
  let thumbnails: Record<string, string> = {};
  let notifs: Notif[] = [];
  let renaming: string | null = null;
  let renameVal = '';
  let dragOver: string | null = null;

  let gridEl: HTMLDivElement;
  let searchEl: HTMLInputElement;
  let renameEl: HTMLInputElement;

  $: activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0];

  function notify(type: Notif['type'], message: string) {
    const id = Date.now().toString();
    notifs = [...notifs, { id, type, message }];
    setTimeout(() => (notifs = notifs.filter((n) => n.id !== id)), 3500);
  }

  async function loadFiles(path: string) {
    loading = true; selected = new Set(); previewFile = null;
    try {
      const entries = await SystemBridge.getFiles(path);
      files = entries;
      for (const e of entries.filter((e: FileEntry) => e.mime_type.startsWith('image/')).slice(0, 24)) {
        if (!thumbnails[e.path]) {
          SystemBridge.readFileAsDataURL(e.path).then((d: string | null) => { if (d) thumbnails = { ...thumbnails, [e.path]: d }; }).catch(() => {});
        }
      }
    } catch { notify('error', `Cannot open: ${path}`); }
    finally { loading = false; }
  }

  function navigateTo(path: string) {
    tabs = tabs.map((t) => {
      if (t.id !== activeTabId) return t;
      const h = t.history.slice(0, t.historyIndex + 1);
      h.push(path);
      return { ...t, path, history: h, historyIndex: h.length - 1 };
    });
    loadFiles(path);
  }

  onMount(() => loadFiles(activeTab.path));

  function goBack() {
    const t = activeTab;
    if (t.historyIndex <= 0) return;
    const ni = t.historyIndex - 1, np = t.history[ni];
    tabs = tabs.map((tb) => (tb.id === activeTabId ? { ...tb, historyIndex: ni, path: np } : tb));
    loadFiles(np);
  }
  function goForward() {
    const t = activeTab;
    if (t.historyIndex >= t.history.length - 1) return;
    const ni = t.historyIndex + 1, np = t.history[ni];
    tabs = tabs.map((tb) => (tb.id === activeTabId ? { ...tb, historyIndex: ni, path: np } : tb));
    loadFiles(np);
  }
  function goUp() {
    const cur = activeTab.path;
    if (cur === 'HOME' || cur === '/') return;
    const parent = cur.includes('/') ? cur.split('/').slice(0, -1).join('/') || '/' : 'HOME';
    navigateTo(parent);
  }

  function addTab() {
    const id = `tab-${Date.now()}`;
    tabs = [...tabs, { id, path: 'HOME', history: ['HOME'], historyIndex: 0 }];
    activeTabId = id;
    loadFiles('HOME');
  }
  function closeTab(id: string) {
    if (tabs.length === 1) return;
    const next = tabs.filter((t) => t.id !== id);
    tabs = next;
    if (activeTabId === id) {
      const n = next[next.length - 1];
      activeTabId = n.id;
      loadFiles(n.path);
    }
  }

  function handleOpen(file: FileEntry) {
    if (file.is_dir) { navigateTo(file.path); return; }
    if (file.mime_type.startsWith('image/')) { openPreview(file); return; }
    if (file.mime_type.startsWith('text/')) {
      // Previously this also fell into `openPreview` — a read-only pane
      // inside Explorer itself, not a real editor. Now opens the file
      // in whichever app the user picked in Settings → Applications
      // (defaults to Notepad).
      const editorApp = (configStore.get().defaultTextEditor ?? 'notepad') === 'blue_code' ? AppId.BLUE_CODE : AppId.NOTEPAD;
      openApp(editorApp, false, undefined, { openPath: file.path });
      return;
    }
    SystemBridge.launchApp(`xdg-open "${file.path}"`);
  }

  async function createFolder() {
    const name = await dialogPrompt({ title: 'New Folder', placeholder: 'Untitled Folder', defaultValue: 'New Folder', confirmLabel: 'Create' });
    if (!name?.trim()) return;
    try { await SystemBridge.createFolder(activeTab.path, name.trim()); notify('success', `Created: ${name}`); loadFiles(activeTab.path); }
    catch { notify('error', 'Failed to create folder'); }
  }

  async function deleteSelected() {
    if (selected.size === 0) return;
    const ok = await dialogConfirm({ title: 'Delete items', message: `Delete ${selected.size} item(s)? This cannot be undone.`, confirmLabel: 'Delete', danger: true });
    if (!ok) return;
    let errors = 0;
    for (const path of selected) { try { await SystemBridge.deleteFile(path); } catch { errors++; } }
    notify(errors ? 'error' : 'success', errors ? `${errors} item(s) failed` : `Deleted ${selected.size} item(s)`);
    selected = new Set();
    loadFiles(activeTab.path);
  }

  function copySelected() { if (!selected.size) return; clipboard = { action: 'copy', files: [...selected] }; notify('info', `Copied ${selected.size} item(s)`); }
  function cutSelected() { if (!selected.size) return; clipboard = { action: 'cut', files: [...selected] }; notify('info', `Cut ${selected.size} item(s)`); }

  async function paste() {
    if (!clipboard) return;
    let errors = 0;
    for (const src of clipboard.files) {
      const name = src.split('/').pop() ?? '';
      const dst = `${activeTab.path}/${name}`;
      try { if (clipboard.action === 'copy') await SystemBridge.copyFile(src, dst); else await SystemBridge.moveFile(src, dst); }
      catch { errors++; }
    }
    notify(errors ? 'error' : 'success', errors ? `${errors} item(s) failed` : `Pasted ${clipboard.files.length} item(s)`);
    if (clipboard.action === 'cut') clipboard = null;
    loadFiles(activeTab.path);
  }

  async function startRename(file: FileEntry) {
    renaming = file.path; renameVal = file.name;
    await tick();
    renameEl?.select();
  }
  async function commitRename() {
    if (!renaming || !renameVal.trim()) { renaming = null; return; }
    const file = files.find((f) => f.path === renaming);
    if (!file || renameVal === file.name) { renaming = null; return; }
    const newPath = renaming.slice(0, renaming.lastIndexOf('/') + 1) + renameVal.trim();
    try { await SystemBridge.moveFile(renaming, newPath); notify('success', `Renamed to: ${renameVal}`); loadFiles(activeTab.path); }
    catch { notify('error', 'Rename failed'); }
    finally { renaming = null; }
  }

  async function openPreview(file: FileEntry) {
    previewFile = file; previewLoading = true;
    archiveEntries = null; archiveError = '';
    try {
      if (isArchive(file)) {
        await loadArchivePreview(file);
      } else if (file.mime_type.startsWith('image/')) {
        previewContent = (await SystemBridge.readFileAsDataURL(file.path)) ?? '';
      } else {
        previewContent = await SystemBridge.readFile(file.path);
      }
    } catch { previewContent = ''; }
    finally { previewLoading = false; }
  }

  function toggleSelect(path: string, e?: MouseEvent) {
    if (renaming) return;
    if (e?.shiftKey && lastSel) {
      const i1 = sorted.findIndex((f) => f.path === lastSel);
      const i2 = sorted.findIndex((f) => f.path === path);
      const [a, b] = [Math.min(i1, i2), Math.max(i1, i2)];
      selected = new Set(sorted.slice(a, b + 1).map((f) => f.path));
    } else if (e?.ctrlKey || e?.metaKey) {
      const n = new Set(selected);
      n.has(path) ? n.delete(path) : n.add(path);
      selected = n;
    } else {
      selected = new Set([path]);
    }
    lastSel = path;
  }

  $: filtered = files.filter((f) => showHidden || !f.name.startsWith('.')).filter((f) => !searchTerm || f.name.toLowerCase().includes(searchTerm.toLowerCase()));
  $: sorted = [...filtered].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    let cmp = 0;
    if (sortBy === 'name') cmp = a.name.localeCompare(b.name);
    else if (sortBy === 'size') cmp = (parseFloat(a.size) || 0) - (parseFloat(b.size) || 0);
    else if (sortBy === 'modified') cmp = (a.modified ?? '').localeCompare(b.modified ?? '');
    else if (sortBy === 'type') cmp = a.mime_type.localeCompare(b.mime_type);
    return sortAsc ? cmp : -cmp;
  });

  function toggleSort(k: SortKey) { if (sortBy === k) sortAsc = !sortAsc; else { sortBy = k; sortAsc = true; } }
  function toggleSortByKey(k: string) { toggleSort(k as SortKey); }


  function onDragStart(e: DragEvent, file: FileEntry) {
    const paths = selected.has(file.path) ? [...selected] : [file.path];
    e.dataTransfer?.setData('text/plain', JSON.stringify(paths));
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copyMove';
  }
  async function onDrop(e: DragEvent, targetDir: FileEntry | null) {
    e.preventDefault(); dragOver = null;
    const dest = targetDir?.path ?? activeTab.path;
    try {
      const paths: string[] = JSON.parse(e.dataTransfer?.getData('text/plain') ?? '[]');
      const isCopy = e.ctrlKey;
      for (const src of paths) {
        const name = src.split('/').pop() ?? '';
        const dst = `${dest}/${name}`;
        if (isCopy) await SystemBridge.copyFile(src, dst); else await SystemBridge.moveFile(src, dst);
      }
      notify('success', `${e.ctrlKey ? 'Copied' : 'Moved'} ${paths.length} item(s)`);
      loadFiles(activeTab.path);
    } catch { notify('error', 'Drop failed'); }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (renaming) return;
    if (e.ctrlKey) {
      if (e.key === 'a') { e.preventDefault(); selected = new Set(sorted.map((f) => f.path)); }
      if (e.key === 'c') { e.preventDefault(); copySelected(); }
      if (e.key === 'x') { e.preventDefault(); cutSelected(); }
      if (e.key === 'v') { e.preventDefault(); paste(); }
      if (e.key === 'f') { e.preventDefault(); showSearch = !showSearch; setTimeout(() => searchEl?.focus(), 50); }
      if (e.key === 'n') { e.preventDefault(); createFolder(); }
      if (e.key === 't') { e.preventDefault(); addTab(); }
    }
    if (e.key === 'F2' && selected.size === 1) {
      const f = files.find((f) => selected.has(f.path));
      if (f) startRename(f);
    }
    if (e.key === 'Delete') { e.preventDefault(); deleteSelected(); }
    if (e.key === 'Escape') { searchTerm = ''; showSearch = false; selected = new Set(); }
  }
  onMount(() => window.addEventListener('keydown', handleKeyDown));
  onMount(() => {
    const closeCtx = () => (ctxMenu = null);
    window.addEventListener('click', closeCtx);
    return () => window.removeEventListener('click', closeCtx);
  });
  onDestroy(() => window.removeEventListener('keydown', handleKeyDown));

  $: breadcrumbs = activeTab.path.split('/').map((part, i, arr) => ({
    label: part === 'HOME' ? '~' : part,
    path: arr.slice(0, i + 1).join('/') || '/',
  }));
</script>

<div class="flex h-full bg-slate-900 text-white overflow-hidden" on:dragover={(e) => e.preventDefault()} on:drop={(e) => onDrop(e, null)}>
  <div class="fixed top-16 right-4 z-50 flex flex-col gap-2 pointer-events-none">
    {#each notifs as n (n.id)}
      <div class="flex items-center gap-2 px-4 py-2 rounded-xl shadow-lg text-sm {n.type === 'success' ? 'bg-green-600/90' : n.type === 'error' ? 'bg-red-600/90' : 'bg-slate-700/90'}">
        {#if n.type === 'success'}<Check size={13} />{:else if n.type === 'error'}<AlertCircle size={13} />{:else}<Info size={13} />{/if}
        {n.message}
      </div>
    {/each}
  </div>

  <div class="w-44 bg-slate-800/50 border-r border-white/5 flex flex-col shrink-0">
    <div class="p-3 border-b border-white/5 flex items-center justify-between">
      <span class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Places</span>
      <button on:click={() => toggleCustomBookmark(activeTab.path)} title="Bookmark current folder" class="p-0.5 hover:text-yellow-400 text-slate-500">
        {#if isCustomBookmark(activeTab.path)}<Star size={13} class="text-yellow-400 fill-yellow-400" />{:else}<Star size={13} />{/if}
      </button>
    </div>
    <div class="flex-1 overflow-y-auto p-2 space-y-0.5">
      {#each BOOKMARKS_WITH_ICONS as bm (bm.path)}
        <button on:click={() => navigateTo(bm.path)} class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm transition-colors {activeTab.path === bm.path ? 'bg-blue-600/20 text-blue-400' : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
          <svelte:component this={bm.icon} size={14} />{bm.name}
        </button>
      {/each}
      <div class="h-px bg-white/5 my-2" />
      <button on:click={() => navigateTo('/')} class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm text-slate-400 hover:bg-white/5 hover:text-white">
        <HardDrive size={14} /> / (root)
      </button>
      {#if customBookmarks.length}
        <div class="h-px bg-white/5 my-2" />
        <div class="px-2 pb-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">Bookmarks</div>
        {#each customBookmarks as path (path)}
          <div class="group w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm transition-colors {activeTab.path === path ? 'bg-blue-600/20 text-blue-400' : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
            <button on:click={() => navigateTo(path)} class="flex items-center gap-2 flex-1 min-w-0 text-left">
              <Folder size={14} class="shrink-0" /><span class="truncate">{path.split('/').pop()}</span>
            </button>
            <button on:click={() => toggleCustomBookmark(path)} class="opacity-0 group-hover:opacity-100 hover:text-red-400 shrink-0"><X size={11} /></button>
          </div>
        {/each}
      {/if}
    </div>
  </div>

  <div class="flex-1 flex flex-col min-w-0">
    <div class="flex items-center bg-slate-800/80 border-b border-white/5 overflow-x-auto shrink-0">
      {#each tabs as t (t.id)}
        <div on:click={() => { activeTabId = t.id; loadFiles(t.path); }}
          class="flex items-center gap-1.5 px-3 py-2 cursor-pointer border-r border-white/5 shrink-0 group max-w-[140px] {t.id === activeTabId ? 'bg-slate-900 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}">
          <Folder size={12} />
          <span class="text-xs truncate">{t.path === 'HOME' ? '~' : t.path.split('/').pop()}</span>
          {#if tabs.length > 1}
            <button on:click|stopPropagation={() => closeTab(t.id)} class="opacity-0 group-hover:opacity-100 hover:text-red-400 ml-auto shrink-0"><X size={10} /></button>
          {/if}
        </div>
      {/each}
      <button on:click={addTab} class="p-2 text-slate-500 hover:text-white shrink-0"><Plus size={12} /></button>
      <div class="ml-auto pr-2">
        <button on:click={() => (dualPane = !dualPane)} title="Dual pane" class="p-1.5 rounded {dualPane ? 'text-blue-400' : 'text-slate-500 hover:text-white'}"><Columns size={13} /></button>
      </div>
    </div>

    <div class="h-11 bg-slate-800 border-b border-white/5 flex items-center px-2 gap-1 shrink-0">
      <button on:click={goBack} disabled={activeTab.historyIndex === 0} class="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><ArrowLeft size={15} /></button>
      <button on:click={goForward} disabled={activeTab.historyIndex >= activeTab.history.length - 1} class="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><ArrowRight size={15} /></button>
      <button on:click={goUp} class="p-1.5 hover:bg-white/10 rounded"><HardDrive size={15} /></button>
      <button on:click={() => loadFiles(activeTab.path)} class="p-1.5 hover:bg-white/10 rounded"><RefreshCw size={15} class={loading ? 'animate-spin' : ''} /></button>
      <div class="w-px h-5 bg-white/10 mx-1" />
      <button on:click={createFolder} title="New folder (Ctrl+N)" class="p-1.5 hover:bg-white/10 rounded"><Plus size={15} /></button>
      <button on:click={deleteSelected} disabled={!selected.size} title="Delete (Del)" class="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Trash2 size={15} /></button>
      <button on:click={copySelected} disabled={!selected.size} title="Copy (Ctrl+C)" class="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Copy size={15} /></button>
      <button on:click={cutSelected} disabled={!selected.size} title="Cut (Ctrl+X)" class="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Scissors size={15} /></button>
      <button on:click={paste} disabled={!clipboard} title="Paste (Ctrl+V)" class="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Clipboard size={15} class={clipboard ? 'text-blue-400' : ''} /></button>
      <div class="w-px h-5 bg-white/10 mx-1" />
      <div class="flex items-center text-sm flex-1 min-w-0 overflow-hidden">
        {#each breadcrumbs as crumb, i (i)}
          {#if i > 0}<ChevronRight size={11} class="mx-0.5 text-slate-600 shrink-0" />{/if}
          <button on:click={() => navigateTo(crumb.path)} class="hover:text-blue-400 transition-colors shrink-0 truncate max-w-[80px]">{crumb.label}</button>
        {/each}
      </div>
      <div class="flex items-center gap-1 ml-auto shrink-0">
        <button on:click={() => { showSearch = !showSearch; setTimeout(() => searchEl?.focus(), 50); }} class="p-1.5 rounded {showSearch ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}"><Search size={15} /></button>
        <button on:click={() => (showHidden = !showHidden)} class="p-1.5 rounded {showHidden ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}" title="Show hidden"><Eye size={15} /></button>
        <button on:click={() => (viewMode = 'grid')} class="p-1.5 rounded {viewMode === 'grid' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}"><Grid size={15} /></button>
        <button on:click={() => (viewMode = 'list')} class="p-1.5 rounded {viewMode === 'list' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}"><List size={15} /></button>
      </div>
    </div>

    {#if showSearch}
      <div class="flex items-center gap-2 px-3 py-1.5 bg-slate-800/50 border-b border-white/5">
        <Search size={13} class="text-slate-500 shrink-0" />
        <input bind:this={searchEl} type="text" bind:value={searchTerm} placeholder="Search in current folder…" class="flex-1 bg-transparent text-sm focus:outline-none placeholder-slate-600" />
        {#if searchTerm}<span class="text-xs text-slate-500 shrink-0">{sorted.length}</span>{/if}
        <button on:click={() => { searchTerm = ''; showSearch = false; }}><X size={13} class="text-slate-500" /></button>
      </div>
    {/if}

    <div class="flex-1 flex overflow-hidden">
      <div bind:this={gridEl} class="flex-1 overflow-auto p-2" on:click={(e) => { if (e.target === gridEl) selected = new Set(); }}>
        {#if loading}
          <div class="flex items-center justify-center h-full"><Loader2 size={22} class="animate-spin text-blue-400" /></div>
        {:else if sorted.length === 0}
          <div class="flex flex-col items-center justify-center h-full text-slate-600 gap-2"><Folder size={36} /><span class="text-sm">{searchTerm ? 'No results' : 'Empty folder'}</span></div>
        {:else if viewMode === 'grid'}
          <div class="grid gap-1" style="grid-template-columns:repeat(auto-fill, minmax(88px, 1fr));">
            {#each sorted as file (file.path)}
              {@const isSel = selected.has(file.path)}
              {@const isDragTgt = dragOver === file.path && file.is_dir}
              {@const isCut = clipboard?.action === 'cut' && clipboard.files.includes(file.path)}
              <div draggable="true"
                on:dragstart={(e) => onDragStart(e, file)}
                on:dragover={(e) => { if (file.is_dir) { e.preventDefault(); dragOver = file.path; } }}
                on:drop={(e) => file.is_dir && onDrop(e, file)}
                on:dragleave={() => (dragOver = null)}
                class="relative flex flex-col items-center p-2 rounded-xl cursor-pointer transition-colors duration-100 select-none {isSel ? 'bg-blue-600/30 ring-2 ring-blue-500/60' : 'hover:bg-white/5'} {isDragTgt ? 'bg-blue-500/20 ring-2 ring-blue-400' : ''} {isCut ? 'opacity-50' : ''}"
                on:click|stopPropagation={(e) => toggleSelect(file.path, e)}
                on:dblclick={() => !renaming && handleOpen(file)}
                on:contextmenu|stopPropagation={(e) => openContextMenu(e, file)}>
                <div class="mb-1.5">
                  {#if file.mime_type.startsWith('image/') && thumbnails[file.path]}
                    <img src={thumbnails[file.path]} alt={file.name} class="w-12 h-12 object-cover rounded-lg" />
                  {:else}
                    <FileIcon {file} size={40} />
                  {/if}
                </div>
                {#if renaming === file.path}
                  <input bind:this={renameEl} bind:value={renameVal} on:blur={commitRename}
                    on:keydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') renaming = null; e.stopPropagation(); }}
                    class="w-full text-xs text-center bg-slate-800 border border-blue-500 rounded px-1 focus:outline-none" autofocus on:click|stopPropagation />
                {:else}
                  <span class="text-xs text-center break-all line-clamp-2 leading-tight px-1">{file.name}</span>
                {/if}
                <span class="text-[10px] text-slate-600 mt-0.5">{file.size}</span>
              </div>
            {/each}
          </div>
        {:else}
          <table class="w-full text-sm border-collapse">
            <thead class="sticky top-0 bg-slate-800 z-10 text-slate-400 text-xs">
              <tr>
                {#each [['Name', 'name'], ['Size', 'size'], ['Modified', 'modified'], ['Type', 'type']] as [label, sk] (sk)}
                  <th class="p-2 text-left cursor-pointer hover:text-white select-none whitespace-nowrap" on:click={() => toggleSortByKey(sk)}>
                    <span class="flex items-center gap-1">{label} {#if sortBy === sk}{#if sortAsc}<SortAsc size={11} />{:else}<SortDesc size={11} />{/if}{/if}</span>
                  </th>
                {/each}
                <th class="w-14" />
              </tr>
            </thead>
            <tbody>
              {#each sorted as file (file.path)}
                {@const isSel = selected.has(file.path)}
                {@const isDragTgt = dragOver === file.path && file.is_dir}
                {@const isCut = clipboard?.action === 'cut' && clipboard.files.includes(file.path)}
                <tr draggable="true"
                  on:dragstart={(e) => onDragStart(e, file)}
                  on:dragover={(e) => { if (file.is_dir) { e.preventDefault(); dragOver = file.path; } }}
                  on:drop={(e) => file.is_dir && onDrop(e, file)}
                  on:dragleave={() => (dragOver = null)}
                  class="border-b border-white/5 cursor-pointer group {isSel ? 'bg-blue-600/20' : 'hover:bg-white/5'} {isDragTgt ? 'bg-blue-500/15' : ''} {isCut ? 'opacity-50' : ''}"
                  on:click={(e) => toggleSelect(file.path, e)}
                  on:dblclick={() => handleOpen(file)}
                  on:contextmenu|preventDefault={(e) => openContextMenu(e, file)}>
                  <td class="p-2">
                    <div class="flex items-center gap-2 min-w-0">
                      {#if file.mime_type.startsWith('image/') && thumbnails[file.path]}
                        <img src={thumbnails[file.path]} alt="" class="w-5 h-5 object-cover rounded shrink-0" />
                      {:else}
                        <FileIcon {file} size={16} />
                      {/if}
                      {#if renaming === file.path}
                        <input bind:this={renameEl} bind:value={renameVal} on:blur={commitRename}
                          on:keydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') renaming = null; e.stopPropagation(); }}
                          class="flex-1 bg-slate-800 border border-blue-500 rounded px-1 text-sm focus:outline-none" autofocus on:click|stopPropagation />
                      {:else}
                        <span class="text-sm truncate">{file.name}</span>
                      {/if}
                    </div>
                  </td>
                  <td class="p-2 text-sm text-slate-500 whitespace-nowrap">{file.size}</td>
                  <td class="p-2 text-sm text-slate-500 whitespace-nowrap">{file.modified ?? '—'}</td>
                  <td class="p-2 text-xs text-slate-600 max-w-[100px] truncate">{file.mime_type}</td>
                  <td class="p-2">
                    <div class="flex gap-1 opacity-0 group-hover:opacity-100">
                      <button on:click|stopPropagation={() => startRename(file)} class="p-1 hover:bg-white/10 rounded text-slate-400"><Edit size={12} /></button>
                      <button on:click|stopPropagation={() => { selected = new Set([file.path]); setTimeout(deleteSelected, 0); }} class="p-1 hover:bg-red-500/20 rounded text-red-400"><Trash2 size={12} /></button>
                    </div>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>

      {#if previewFile}
        {@const pf = previewFile}
        <div class="w-60 border-l border-white/5 bg-slate-800/50 flex flex-col shrink-0">
          <div class="flex items-center justify-between p-2 border-b border-white/5">
            <span class="text-xs font-medium truncate">{pf.name}</span>
            <button on:click={() => (previewFile = null)} class="p-1 hover:bg-white/10 rounded shrink-0"><X size={12} /></button>
          </div>
          <div class="flex-1 overflow-auto p-2">
            <div class="text-[10px] text-slate-500 mb-2 space-y-0.5">
              <div>Size: {pf.size}</div>
              <div>Modified: {pf.modified ?? '—'}</div>
            </div>
            {#if isArchive(pf)}
              <!-- Blue Archive integration: preview archive contents inline, no separate app needed -->
              <div class="flex items-center gap-1.5 text-[10px] text-blue-400 mb-1.5"><ArchiveIcon size={12} /> Archive contents</div>
              {#if archiveLoading}
                <Loader2 size={16} class="animate-spin text-blue-400" />
              {:else if archiveError}
                <div class="text-[10px] text-red-400">{archiveError}</div>
              {:else if archiveEntries && archiveEntries.length}
                <ul class="space-y-0.5 max-h-64 overflow-auto">
                  {#each archiveEntries as entry (entry.name)}
                    <li class="flex items-center gap-1.5 text-[10px] text-slate-300 truncate">
                      {#if entry.is_dir}<Folder size={11} class="text-slate-500 shrink-0" />{:else}<FileBox size={11} class="text-slate-500 shrink-0" />{/if}
                      <span class="truncate flex-1">{entry.name}</span>
                      {#if entry.size}<span class="text-slate-600 shrink-0">{entry.size}</span>{/if}
                    </li>
                  {/each}
                </ul>
              {:else}
                <div class="text-center text-slate-600 text-xs py-4">Empty or unsupported archive</div>
              {/if}
            {:else if previewLoading}
              <Loader2 size={16} class="animate-spin text-blue-400" />
            {:else if pf.mime_type.startsWith('image/') && previewContent}
              <img src={previewContent} alt="preview" class="max-w-full rounded object-contain" />
            {:else if pf.mime_type.startsWith('text/') && previewContent}
              <pre class="text-[10px] bg-slate-900 p-2 rounded overflow-auto max-h-64 font-mono">{previewContent.slice(0, 3000)}</pre>
            {:else}
              <div class="text-center text-slate-600 text-xs py-4">No preview</div>
            {/if}
          </div>
          <div class="p-2 border-t border-white/5 flex gap-1.5">
            {#if isArchive(pf)}
              <button on:click={() => SystemBridge.invokeCommand('archive_extract', { path: pf.path, destDir: activeTab.path }).then(() => { notify('success', 'Extracted'); loadFiles(activeTab.path); }).catch(() => notify('error', 'Extraction failed'))} class="flex-1 py-1 bg-blue-600 hover:bg-blue-500 rounded text-xs">Extract here</button>
            {:else}
              <button on:click={() => SystemBridge.launchApp(`xdg-open "${pf.path}"`)} class="flex-1 py-1 bg-blue-600 hover:bg-blue-500 rounded text-xs">Open</button>
            {/if}
            <button on:click={() => openProperties(pf)} title="Properties" class="p-1.5 hover:bg-white/10 rounded"><Info size={12} /></button>
            <button on:click={() => startRename(pf)} class="p-1.5 hover:bg-white/10 rounded"><Edit size={12} /></button>
            <button on:click={() => { selected = new Set([pf.path]); deleteSelected(); previewFile = null; }} class="p-1.5 hover:bg-red-500/20 rounded text-red-400"><Trash2 size={12} /></button>
          </div>
        </div>
      {/if}

      {#if ctxMenu}
        {@const cf = ctxMenu.file}
        <div class="fixed z-[60] w-44 bg-slate-800 border border-white/10 rounded-lg shadow-2xl py-1 text-sm" style="left:{ctxMenu.x}px; top:{ctxMenu.y}px;" on:click|stopPropagation>
          <button on:click={() => { handleOpen(cf); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-white/10 flex items-center gap-2"><Eye size={13} /> Open</button>
          {#if isArchive(cf)}
            <button on:click={() => { openPreview(cf); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-white/10 flex items-center gap-2"><ArchiveIcon size={13} /> Preview contents</button>
          {/if}
          <button on:click={() => { startRename(cf); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-white/10 flex items-center gap-2"><Edit size={13} /> Rename</button>
          <button on:click={() => { selected = new Set([cf.path]); copySelected(); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-white/10 flex items-center gap-2"><Copy size={13} /> Copy</button>
          <button on:click={() => { selected = new Set([cf.path]); cutSelected(); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-white/10 flex items-center gap-2"><Scissors size={13} /> Cut</button>
          {#if cf.is_dir}
            <button on:click={() => { toggleCustomBookmark(cf.path); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-white/10 flex items-center gap-2">
              {#if isCustomBookmark(cf.path)}<StarOff size={13} /> Remove bookmark{:else}<Star size={13} /> Add bookmark{/if}
            </button>
          {/if}
          <div class="h-px bg-white/10 my-1" />
          <button on:click={() => { selected = new Set([cf.path]); deleteSelected(); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-red-500/20 text-red-400 flex items-center gap-2"><Trash2 size={13} /> Delete</button>
          <div class="h-px bg-white/10 my-1" />
          <button on:click={() => { openProperties(cf); closeContextMenu(); }} class="w-full text-left px-3 py-1.5 hover:bg-white/10 flex items-center gap-2"><Info size={13} /> Properties</button>
        </div>
      {/if}

      {#if propertiesFile}
        {@const pf = propertiesFile}
        <div class="fixed inset-0 z-[70] bg-black/50 flex items-center justify-center" on:click={closeProperties}>
          <div class="w-80 bg-slate-800 border border-white/10 rounded-xl shadow-2xl p-4" on:click|stopPropagation>
            <div class="flex items-center gap-2 mb-3">
              <FileIcon file={pf} size={28} />
              <span class="font-medium truncate">{pf.name}</span>
              <button on:click={closeProperties} class="ml-auto p-1 hover:bg-white/10 rounded"><X size={14} /></button>
            </div>
            <div class="space-y-1.5 text-xs text-slate-300">
              <div class="flex justify-between"><span class="text-slate-500">Type</span><span class="truncate max-w-[180px]">{pf.is_dir ? 'Folder' : (pf.mime_type || 'Unknown')}</span></div>
              <div class="flex justify-between"><span class="text-slate-500">Location</span><span class="truncate max-w-[180px]">{pf.path.slice(0, pf.path.lastIndexOf('/')) || '/'}</span></div>
              <div class="flex justify-between"><span class="text-slate-500">Full path</span><span class="truncate max-w-[180px]" title={pf.path}>{pf.path}</span></div>
              <div class="flex justify-between"><span class="text-slate-500">Size</span><span>{pf.size}</span></div>
              <div class="flex justify-between"><span class="text-slate-500">Modified</span><span>{pf.modified ?? '—'}</span></div>
              {#if isArchive(pf)}
                <div class="flex justify-between"><span class="text-slate-500">Archive entries</span><span>{archiveEntries?.length ?? '—'}</span></div>
              {/if}
            </div>
          </div>
        </div>
      {/if}

      {#if dualPane}
        <div class="w-72 border-l border-white/5 bg-slate-900 flex flex-col shrink-0">
          <div class="flex items-center gap-2 p-2 bg-slate-800 border-b border-white/5 text-xs text-slate-400">
            <Columns size={11} />
            <span class="truncate flex-1">{rightPath}</span>
            <button on:click={() => { rightPath = rightPath.includes('/') ? rightPath.split('/').slice(0, -1).join('/') || '/' : 'HOME'; }} class="hover:text-white shrink-0"><ArrowLeft size={11} /></button>
          </div>
          <RightPane path={rightPath} {thumbnails} on:navigate={(e) => (rightPath = e.detail)} />
        </div>
      {/if}
    </div>

    <div class="h-5 border-t border-white/5 bg-slate-800/50 flex items-center px-3 gap-4 text-[11px] text-slate-600 shrink-0">
      <span>{sorted.length} items</span>
      {#if selected.size > 0}<span>{selected.size} selected</span>{/if}
      {#if clipboard}<span class="text-blue-500">{clipboard.action === 'copy' ? 'Copied' : 'Cut'} {clipboard.files.length}</span>{/if}
      <span class="ml-auto">{activeTab.path}</span>
    </div>
  </div>
</div>
