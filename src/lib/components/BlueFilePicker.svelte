<script lang="ts">
  /**
   * Blue Environment's own file/folder picker — see stores/filePicker.ts's
   * doc comment for why this exists (previously every "Open Folder"/
   * "Open File" flow fell through to the OS-native GTK-ish dialog, which
   * is exactly the "no own window for this, unlike KDE/GNOME" gap that
   * was reported). Mounted once near the app root (App.svelte), same
   * pattern as DialogHost.svelte.
   */
  import { activeFilePicker, closeFilePicker } from '../stores/filePicker';
  import { SystemBridge } from '../utils/systemBridge';
  import { configStore } from '../utils/configStore';
  import { BOOKMARKS } from './apps/Explorer-App/types';
  import type { FileEntry } from './apps/Explorer-App/types';
  import FileIcon from './apps/Explorer-App/FileIcon.svelte';
  import {
    Home, Folder, FileText, Download, Image, Music, Video, HardDrive,
    ArrowLeft, ArrowRight, ArrowUp, RefreshCw, FolderPlus, Loader2,
    Search, X, ArrowUpDown, Check, Pencil, Trash2,
  } from 'lucide-svelte';
  import { dialogPrompt, dialogConfirm } from '../stores/dialog';
  import { t } from '../stores/language';
  import { tick } from 'svelte';

  const BM_ICONS: Record<string, any> = { Home, Folder, FileText, Download, Image, Music, Video };
  const bookmarks = [
    ...BOOKMARKS.map((b) => ({ ...b, icon: BM_ICONS[b.iconName] ?? Folder })),
    { name: 'Filesystem', path: '/', icon: HardDrive },
  ];

  let currentPath = 'HOME';
  let rawEntries: FileEntry[] = [];
  let loading = false;
  let history: string[] = [];
  let future: string[] = [];
  let filenameInput = '';
  let errorMsg = '';
  let listEl: HTMLDivElement;

  // ── Selection ────────────────────────────────────────────────────────
  // A Set even in single-select mode (rather than a plain `string |
  // null`) so multi-select ("multiple: true", e.g. Blue Video/Images'
  // "open files" flows) and single-select share one code path instead
  // of two parallel ones that could drift apart.
  let selectedPaths = new Set<string>();
  // Which row keyboard navigation (arrows) is currently sitting on —
  // independent from `selectedPaths` because arrowing through the list
  // shouldn't itself change what's selected until Enter/Space commits it,
  // same as a native file dialog's list box.
  let activeIndex = -1;

  function isPickable(entry: FileEntry): boolean {
    return mode === 'directory' ? entry.is_dir : !entry.is_dir;
  }

  function setSingleSelection(path: string) {
    selectedPaths = new Set([path]);
  }
  function toggleSelection(path: string) {
    const next = new Set(selectedPaths);
    if (next.has(path)) next.delete(path); else next.add(path);
    selectedPaths = next;
  }
  /** Click on a row — plain click replaces the selection; ctrl/cmd-click
   * (or the checkbox itself) toggles membership when multi-select is on,
   * matching Explorer/Finder/Nautilus conventions rather than inventing
   * a new one. */
  function selectEntry(entry: FileEntry, index: number, additive: boolean) {
    activeIndex = index;
    if (multiple && additive) { toggleSelection(entry.path); return; }
    setSingleSelection(entry.path);
    if (mode === 'directory' || isPickable(entry)) filenameInput = entry.name;
  }

  // ── Live search — filters the current folder's listing as you type,
  // rather than requiring Enter or a separate search mode. ────────────
  let searchQuery = '';

  // ── Sorting ──────────────────────────────────────────────────────────
  type SortKey = 'name' | 'size' | 'modified';
  let sortKey: SortKey = 'name';
  let sortAsc = true;
  const SORT_LABELS: Record<SortKey, string> = { name: 'Nazwa', size: 'Rozmiar', modified: 'Data' };
  function cycleSortKey() {
    const keys: SortKey[] = ['name', 'size', 'modified'];
    sortKey = keys[(keys.indexOf(sortKey) + 1) % keys.length];
  }

  // ── Image thumbnails — same mechanism ExplorerApp.svelte already uses
  // (SystemBridge.readFileAsDataURL), just scoped to this component so a
  // picker used to choose an image shows an actual preview instead of a
  // generic file icon for every entry. ─────────────────────────────────
  let thumbnails: Record<string, string> = {};
  function loadThumbnails(list: FileEntry[]) {
    for (const f of list) {
      if (!f.is_dir && f.mime_type?.startsWith('image/') && !thumbnails[f.path]) {
        SystemBridge.readFileAsDataURL(f.path)
          .then((d: string | null) => { if (d) thumbnails = { ...thumbnails, [f.path]: d }; })
          .catch(() => {});
      }
    }
  }

  $: picker = $activeFilePicker;
  $: mode = picker?.options.mode ?? 'file';
  $: filters = picker?.options.filters ?? [];
  $: rememberKey = picker?.options.rememberKey;
  $: multiple = !!picker?.options.multiple;

  $: if (picker) init(picker.options);

  async function init(options: NonNullable<typeof picker>['options']) {
    searchQuery = '';
    sortKey = 'name';
    sortAsc = true;
    // Explicit `startPath` wins; otherwise fall back to wherever this
    // exact flow (`rememberKey`) last left off — see that field's doc
    // comment in filePicker.ts — and only default to HOME if neither is
    // set (a picker used for the first time, or with no memory key).
    const remembered = options.rememberKey ? configStore.get().filePickerLastPaths?.[options.rememberKey] : undefined;
    currentPath = options.startPath ?? remembered ?? 'HOME';
    history = [];
    future = [];
    selectedPaths = new Set();
    activeIndex = -1;
    filenameInput = '';
    errorMsg = '';
    await load();
    await tick();
    listEl?.focus();
  }

  async function load() {
    loading = true;
    errorMsg = '';
    try {
      const list = await SystemBridge.getFiles(currentPath);
      rawEntries = (list as FileEntry[])
        .filter((f) => f.is_dir || mode === 'file')
        .filter((f) => f.is_dir || matchesFilter(f));
      loadThumbnails(rawEntries);
      if (rememberKey) {
        await configStore.save({ filePickerLastPaths: { ...(configStore.get().filePickerLastPaths ?? {}), [rememberKey]: currentPath } });
      }
    } catch {
      rawEntries = [];
      errorMsg = 'Nie można odczytać tego katalogu.';
    } finally {
      loading = false;
    }
  }

  function matchesFilter(f: FileEntry): boolean {
    if (!filters.length) return true;
    const ext = f.name.split('.').pop()?.toLowerCase() ?? '';
    return filters.some((flt) => flt.extensions.includes('*') || flt.extensions.map((e) => e.toLowerCase()).includes(ext));
  }

  function parseSize(size: string): number {
    // FileEntry.size is a pre-formatted string like "12.3 KB" or "DIR" —
    // fine for display, useless for sorting numerically, so pull the
    // number back out rather than plumbing a second raw-bytes field
    // through the backend just for this.
    const m = size.match(/([\d.]+)\s*(KB|MB|GB)?/i);
    if (!m) return 0;
    const n = parseFloat(m[1]);
    const unit = (m[2] ?? '').toUpperCase();
    return unit === 'GB' ? n * 1e9 : unit === 'MB' ? n * 1e6 : unit === 'KB' ? n * 1e3 : n;
  }

  // Folders always float to the top regardless of sort — the one
  // Explorer/Finder/Nautilus convention this deliberately keeps rather
  // than a "pure" sort, since interleaving files and folders by size or
  // date makes a picker much harder to scan.
  $: entries = rawEntries
    .filter((f) => !searchQuery.trim() || f.name.toLowerCase().includes(searchQuery.trim().toLowerCase()))
    .slice()
    .sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      if (sortKey === 'name') cmp = a.name.localeCompare(b.name);
      else if (sortKey === 'size') cmp = parseSize(a.size) - parseSize(b.size);
      else cmp = (a.modified ?? '').localeCompare(b.modified ?? '');
      return sortAsc ? cmp : -cmp;
    });

  function navigate(path: string, pushHistory = true) {
    if (pushHistory) { history = [...history, currentPath]; future = []; }
    currentPath = path;
    selectedPaths = new Set();
    activeIndex = -1;
    filenameInput = '';
    searchQuery = '';
    load();
  }

  function goBack() {
    if (!history.length) return;
    const prev = history[history.length - 1];
    future = [currentPath, ...future];
    history = history.slice(0, -1);
    currentPath = prev;
    load();
  }
  function goForward() {
    if (!future.length) return;
    const next = future[0];
    history = [...history, currentPath];
    future = future.slice(1);
    currentPath = next;
    load();
  }
  function goUp() {
    const parts = currentPath.split('/').filter(Boolean);
    if (currentPath === 'HOME' || parts.length <= 1) return;
    const parent = currentPath.startsWith('/') ? '/' + parts.slice(0, -1).join('/') : parts.slice(0, -1).join('/') || 'HOME';
    navigate(parent || '/');
  }

  function rowClick(entry: FileEntry, index: number, e: MouseEvent) {
    selectEntry(entry, index, e.ctrlKey || e.metaKey);
  }
  function rowDblClick(entry: FileEntry) {
    if (entry.is_dir) { navigate(entry.path); return; }
    if (mode === 'file') confirm();
  }

  // ── Keyboard navigation — arrows to move, Enter to open/confirm,
  // Backspace to go up a level, Space to toggle the checkbox in
  // multi-select mode. Attached to the list container (not `window`)
  // so it only acts while the picker's list actually has focus, and
  // doesn't need any of the "is a dialog open" guards the rest of the
  // app's global keydown handlers needed (see Desktop.svelte/
  // ExplorerApp.svelte/keyboardShortcuts.ts) — this listener is already
  // scoped to just this element. ───────────────────────────────────────
  // Focus trap — same rationale as DialogHost.svelte's (a modal that
  // lets Tab walk focus out into the desktop/window underneath isn't
  // really modal). Scoped to `pickerRootEl` rather than `document`
  // since this is a much bigger surface (sidebar, search box, list,
  // footer buttons) than DialogHost's single input+two-buttons.
  let pickerRootEl: HTMLDivElement;
  function getFocusable(): HTMLElement[] {
    if (!pickerRootEl) return [];
    return Array.from(
      pickerRootEl.querySelectorAll<HTMLElement>('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])')
    ).filter((el) => !el.hasAttribute('disabled') && el.offsetParent !== null);
  }
  function handleRootKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.preventDefault(); cancel(); return; }
    if (e.key !== 'Tab') return;
    const focusable = getFocusable();
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (e.shiftKey) {
      if (active === first || !focusable.includes(active as HTMLElement)) { e.preventDefault(); last.focus(); }
    } else {
      if (active === last || !focusable.includes(active as HTMLElement)) { e.preventDefault(); first.focus(); }
    }
  }

  function handleListKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!entries.length) return;
      const dir = e.key === 'ArrowDown' ? 1 : -1;
      activeIndex = Math.max(0, Math.min(entries.length - 1, activeIndex + dir));
      const entry = entries[activeIndex];
      if (!(e.shiftKey && multiple)) setSingleSelection(entry.path);
      else toggleSelection(entry.path);
      if (mode === 'directory' || isPickable(entry)) filenameInput = entry.name;
      listEl?.querySelector<HTMLElement>(`[data-index="${activeIndex}"]`)?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const entry = activeIndex >= 0 ? entries[activeIndex] : undefined;
      if (entry?.is_dir) navigate(entry.path);
      else confirm();
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      goUp();
    } else if (e.key === ' ' && multiple) {
      e.preventDefault();
      const entry = activeIndex >= 0 ? entries[activeIndex] : undefined;
      if (entry) toggleSelection(entry.path);
    } else if (e.key === 'Delete') {
      e.preventDefault();
      deleteSelected();
    }
  }

  async function newFolder() {
    const name = await dialogPrompt({ title: 'Nowy folder', placeholder: 'Nowy folder', defaultValue: 'Nowy folder', confirmLabel: 'Utwórz' });
    if (!name?.trim()) return;
    try { await SystemBridge.createFolder(currentPath, name.trim()); await load(); } catch { /* best effort */ }
  }

  // ── Rename / delete directly from the picker — previously only
  // possible by cancelling out to a real Explorer window first. ───────
  $: selectedEntries = entries.filter((f) => selectedPaths.has(f.path));

  async function renameSelected() {
    if (selectedEntries.length !== 1) return;
    const file = selectedEntries[0];
    const newName = await dialogPrompt({ title: 'Zmień nazwę', defaultValue: file.name, confirmLabel: 'Zapisz' });
    if (!newName?.trim() || newName.trim() === file.name) return;
    const dir = file.path.slice(0, file.path.lastIndexOf('/')) || '/';
    const newPath = `${dir}/${newName.trim()}`.replace(/\/+/g, '/');
    try {
      await SystemBridge.moveFile(file.path, newPath);
      selectedPaths = new Set([newPath]);
      await load();
    } catch { /* best effort */ }
  }
  async function deleteSelected() {
    if (!selectedEntries.length) return;
    const ok = await dialogConfirm({
      title: 'Usuń',
      message: selectedEntries.length === 1
        ? `Usunąć „${selectedEntries[0].name}"? Tej operacji nie można cofnąć.`
        : `Usunąć ${selectedEntries.length} pozycji? Tej operacji nie można cofnąć.`,
      confirmLabel: 'Usuń',
      danger: true,
    });
    if (!ok) return;
    try {
      for (const f of selectedEntries) await SystemBridge.deleteFile(f.path);
      selectedPaths = new Set();
      await load();
    } catch { /* best effort */ }
  }

  // ── Drag-and-drop between folders — drag a row onto a folder row (or
  // a sidebar bookmark) in the same listing to move it there, same
  // gesture ExplorerApp.svelte already supports in its own windows. ───
  let dragOverPath: string | null = null;
  function handleRowDragStart(e: DragEvent, entry: FileEntry) {
    if (!selectedPaths.has(entry.path)) setSingleSelection(entry.path);
    e.dataTransfer?.setData('text/plain', JSON.stringify([...selectedPaths]));
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }
  function handleRowDragOver(e: DragEvent, entry: FileEntry) {
    if (!entry.is_dir) return;
    e.preventDefault();
    dragOverPath = entry.path;
  }
  function handleBookmarkDragOver(e: DragEvent, path: string) {
    e.preventDefault();
    dragOverPath = path;
  }
  function handleDragLeave() { dragOverPath = null; }
  async function handleDropOnFolder(e: DragEvent, targetDir: string) {
    e.preventDefault();
    dragOverPath = null;
    const raw = e.dataTransfer?.getData('text/plain');
    if (!raw) return;
    let paths: string[] = [];
    try { paths = JSON.parse(raw); } catch { return; }
    for (const p of paths) {
      if (p === targetDir) continue;
      const name = p.split('/').pop();
      const dest = `${targetDir}/${name}`.replace(/\/+/g, '/');
      try { await SystemBridge.moveFile(p, dest); } catch { /* best effort */ }
    }
    selectedPaths = new Set();
    await load();
  }

  function confirm() {
    if (!picker) return;
    if (mode === 'directory') {
      const target = selectedPaths.size ? [...selectedPaths][0] : currentPath;
      closeFilePicker(picker, [target]);
    } else {
      if (!selectedPaths.size) return;
      closeFilePicker(picker, [...selectedPaths]);
    }
  }
  function cancel() {
    if (!picker) return;
    closeFilePicker(picker, []);
  }

  $: canConfirm = mode === 'directory' ? true : selectedPaths.size > 0;
</script>

{#if picker}
  <div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm" on:mousedown={(e) => { if (e.target === e.currentTarget) cancel(); }}>
    <div bind:this={pickerRootEl} on:keydown={handleRootKeydown} role="dialog" aria-modal="true" class="w-[760px] h-[540px] bg-slate-900 border border-white/10 rounded-2xl shadow-2xl flex flex-col overflow-hidden">
      <!-- Title bar -->
      <div class="h-11 flex items-center gap-2 px-4 border-b border-white/10 shrink-0">
        <h3 class="text-sm font-semibold text-white flex-1 truncate">
          {picker.options.title ?? (mode === 'directory' ? 'Wybierz katalog' : 'Wybierz plik')}
        </h3>
        {#if selectedEntries.length === 1}
          <button on:click={renameSelected} title="Zmień nazwę" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><Pencil size={13} /></button>
        {/if}
        {#if selectedEntries.length > 0}
          <button on:click={deleteSelected} title="Usuń" class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-red-400 transition-colors"><Trash2 size={13} /></button>
        {/if}
        <button on:click={goBack} disabled={!history.length} class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white disabled:opacity-30 disabled:hover:bg-transparent transition-colors"><ArrowLeft size={14} /></button>
        <button on:click={goForward} disabled={!future.length} class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white disabled:opacity-30 disabled:hover:bg-transparent transition-colors"><ArrowRight size={14} /></button>
        <button on:click={goUp} class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><ArrowUp size={14} /></button>
        <button on:click={load} class="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"><RefreshCw size={13} class={loading ? 'animate-spin' : ''} /></button>
      </div>

      <div class="flex flex-1 min-h-0">
        <!-- Sidebar -->
        <div class="w-44 border-r border-white/10 p-2 overflow-y-auto shrink-0">
          {#each bookmarks as bm (bm.path)}
            <button
              on:click={() => navigate(bm.path)}
              on:dragover={(e) => handleBookmarkDragOver(e, bm.path)}
              on:dragleave={handleDragLeave}
              on:drop={(e) => handleDropOnFolder(e, bm.path)}
              class="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-xs text-left transition-colors {currentPath === bm.path ? 'bg-blue-600/30 text-blue-300' : dragOverPath === bm.path ? 'bg-blue-500/20 ring-1 ring-blue-400' : 'text-slate-300 hover:bg-white/5'}"
            >
              <svelte:component this={bm.icon} size={13} class="shrink-0" />
              <span class="truncate">{bm.name}</span>
            </button>
          {/each}
        </div>

        <!-- Main pane -->
        <div class="flex-1 min-w-0 flex flex-col">
          <div class="px-3 py-1.5 border-b border-white/5 text-[11px] text-slate-500 truncate shrink-0">{currentPath}</div>

          <!-- Search + sort bar -->
          <div class="flex items-center gap-2 px-3 py-1.5 border-b border-white/5 shrink-0">
            <div class="relative flex-1">
              <Search size={12} class="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500" />
              <input
                bind:value={searchQuery}
                placeholder="Szukaj w tym folderze…"
                class="w-full bg-slate-800 border border-white/10 rounded-lg pl-7 pr-7 py-1 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60"
              />
              {#if searchQuery}
                <button on:click={() => (searchQuery = '')} class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white transition-colors"><X size={12} /></button>
              {/if}
            </div>
            {#if multiple && selectedEntries.length > 0}
              <span class="text-[11px] text-slate-500 shrink-0">{selectedEntries.length} wybrane</span>
            {/if}
            <button on:click={cycleSortKey} title="Zmień kolejność sortowania" class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-white/10 text-[11px] text-slate-400 hover:text-white transition-colors shrink-0">
              <ArrowUpDown size={11} /> {SORT_LABELS[sortKey]}
            </button>
            <button on:click={() => (sortAsc = !sortAsc)} title={sortAsc ? 'Rosnąco' : 'Malejąco'} class="p-1 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors shrink-0">
              <ArrowUp size={12} class="transition-transform {sortAsc ? '' : 'rotate-180'}" />
            </button>
          </div>

          <!-- svelte-ignore a11y-no-noninteractive-tabindex -->
          <div
            bind:this={listEl}
            tabindex="0"
            on:keydown={handleListKeydown}
            class="flex-1 overflow-y-auto outline-none"
          >
            {#if loading}
              <div class="flex items-center justify-center gap-2 text-slate-500 text-xs py-8"><Loader2 size={14} class="animate-spin" /> Ładowanie…</div>
            {:else if errorMsg}
              <div class="text-center text-slate-500 text-xs py-8">{errorMsg}</div>
            {:else if entries.length === 0}
              <div class="text-center text-slate-600 text-xs py-8">{searchQuery ? 'Brak pasujących wyników' : 'Ten folder jest pusty'}</div>
            {:else}
              {#each entries as entry, i (entry.path)}
                {@const isSelected = selectedPaths.has(entry.path)}
                {@const isDragTarget = dragOverPath === entry.path && entry.is_dir}
                <button
                  data-index={i}
                  draggable="true"
                  on:dragstart={(e) => handleRowDragStart(e, entry)}
                  on:dragover={(e) => handleRowDragOver(e, entry)}
                  on:dragleave={handleDragLeave}
                  on:drop={(e) => entry.is_dir && handleDropOnFolder(e, entry.path)}
                  on:click={(e) => rowClick(entry, i, e)}
                  on:dblclick={() => rowDblClick(entry)}
                  class="w-full flex items-center gap-2.5 px-3 py-1.5 text-left transition-colors {isSelected ? 'bg-blue-600/20' : activeIndex === i ? 'bg-white/5' : 'hover:bg-white/5'} {isDragTarget ? 'ring-1 ring-inset ring-blue-400 bg-blue-500/10' : ''}"
                >
                  {#if multiple && isPickable(entry)}
                    <span
                      role="checkbox"
                      aria-checked={isSelected}
                      tabindex="-1"
                      on:click|stopPropagation={() => { activeIndex = i; toggleSelection(entry.path); }}
                      class="w-4 h-4 rounded border shrink-0 flex items-center justify-center transition-colors {isSelected ? 'bg-blue-600 border-blue-500' : 'border-white/20 hover:border-white/40'}"
                    >
                      {#if isSelected}<Check size={10} class="text-white" />{/if}
                    </span>
                  {/if}
                  {#if !entry.is_dir && thumbnails[entry.path]}
                    <img src={thumbnails[entry.path]} alt="" class="w-6 h-6 object-cover rounded shrink-0" />
                  {:else}
                    <FileIcon file={entry} size={15} />
                  {/if}
                  <span class="text-xs text-slate-200 truncate flex-1">{entry.name}</span>
                  {#if isSelected && !multiple}<Check size={12} class="text-blue-400 shrink-0" />{/if}
                  {#if !entry.is_dir}<span class="text-[10px] text-slate-500 shrink-0 w-14 text-right">{entry.size}</span>{/if}
                </button>
              {/each}
            {/if}
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="border-t border-white/10 p-3 flex items-center gap-2 shrink-0">
        <button on:click={newFolder} title="Nowy folder" class="p-2 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors shrink-0"><FolderPlus size={14} /></button>
        <input
          bind:value={filenameInput}
          readonly={mode === 'file'}
          placeholder={mode === 'directory' ? 'Nazwa folderu' : (multiple ? `${selectedEntries.length || ''} plik(ów)`.trim() : 'Nazwa pliku')}
          class="flex-1 bg-slate-800 border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/60"
        />
        <button on:click={cancel} class="px-3.5 py-1.5 text-xs bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors text-slate-200">{$t('settings.common.cancel')}</button>
        <button on:click={confirm} disabled={!canConfirm} class="px-3.5 py-1.5 text-xs rounded-lg transition-colors text-white bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:hover:bg-blue-600">
          {picker.options.confirmLabel ?? (mode === 'directory' ? 'Otwórz' : 'Otwórz')}
        </button>
      </div>
    </div>
  </div>
{/if}
