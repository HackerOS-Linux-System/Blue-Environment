<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import {
    Folder, HardDrive, ArrowLeft, ArrowRight, RefreshCw,
    Plus, Trash2, Copy, Clipboard, Scissors, Home, Image,
    Music, Video, FileText, Download, ChevronRight,
    Grid, List, Eye, X, Edit, Search,
    SortAsc, SortDesc, Columns, Info, Check, AlertCircle, Loader2,
    Star, StarOff, Archive as ArchiveIcon, FileBox,
    FilePlus, FolderPlus, ExternalLink, Link2, ArchiveRestore, FolderOpen, ClipboardPaste, CheckSquare, AppWindow,
  } from 'lucide-svelte';
  import { openFileWithDefaultApp } from '../../../utils/openFile';
  import { showContextMenu, type MenuItem } from '../../../stores/contextMenu';
  import { SystemBridge, shellQuote, type TrashEntry } from '../../../utils/systemBridge';
  import { dialogPrompt, dialogConfirm, activeDialog } from '../../../stores/dialog';
  import { get } from 'svelte/store';
  import { configStore } from '../../../utils/configStore';
  import { activeShellThemeId } from '../../../stores/shellTheme';
  import { fileTypeAssociations, ensureFileTypeAssociationsLoaded, resolveAssociationLocally } from '../../../utils/fileTypeAssociations';

  // Real (if partial — see this app's own comment further down for the
  // honest scope) Hydra retrofit: this app previously had zero
  // shell-theme awareness like every other app in this codebase (see
  // `src/lib/stores/shellTheme.ts`'s own doc on why apps don't get a
  // theme id prop-drilled in at all) — root background, sidebar, and
  // the selection-highlight color (the single most-repeated themed
  // surface in this file, appearing in bookmarks/tree/grid/list views)
  // follow the active theme directly, via `isHydra` below.
  //
  // The many *other* hardcoded `bg-slate-800`/`border-white/5`/
  // `text-slate-400`/`hover:bg-white/10` surfaces throughout this file
  // (toolbar, tabs, context menu, properties panel, ...) now inherit
  // their retint from the generic `[data-shell-theme='hydra']
  // .app-content-area` block in app.css instead of needing an
  // `isHydra` branch on every single one — that block's own comment
  // originally only covered `bg-*`/`border-*` on slate/blue, missing
  // `bg-white/*` and `text-slate-*` entirely, which is exactly the gap
  // that left this file's toolbar/tabs/context-menu looking untouched;
  // it's been extended to close that. A few surfaces the app
  // deliberately branches on directly (this selection highlight,
  // sidebar, root background) still do, because they need a genuinely
  // different look (pill-shaped ring, distinct sidebar tint) rather
  // than a straight color swap of the same shape.
  $: isHydra = $activeShellThemeId === 'hydra';
  $: selClass = isHydra ? 'bg-pink-600/30 ring-2 ring-pink-500/60' : 'bg-blue-600/30 ring-2 ring-blue-500/60';
  $: selClassSubtle = isHydra ? 'bg-pink-600/20 text-pink-400' : 'bg-blue-600/20 text-blue-400';
  // Drag-and-drop drop-target highlight — was hardcoded blue regardless
  // of theme (unlike the selection classes right above, which already
  // went pink under Hydra). A folder you're dragging a file onto is one
  // of the most visually prominent moments in the whole app, so leaving
  // it stuck on blue was a pretty conspicuous gap in "reacts to the
  // shell theme".
  $: dragTgtClassGrid = isHydra ? 'bg-pink-500/20 ring-2 ring-pink-400' : 'bg-blue-500/20 ring-2 ring-blue-400';
  $: dragTgtClassRow = isHydra ? 'bg-pink-500/15' : 'bg-blue-500/15';
  import { openApp } from '../../../stores/windowManager';
  import { AppId } from '../../../types';
  import type { FileEntry, Tab, Notif, SortKey } from './types';
  import { BOOKMARKS, TRASH_PATH } from './types';
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
  let propertiesDetails: { permissions: string; owner: string; group: string; size_bytes: number; accessed: string; modified: string; symlink_target?: string } | null = null;
  function formatBytes(n: number) {
    if (n < 1024) return `${n} B`;
    const u = ['KB', 'MB', 'GB', 'TB']; let v = n / 1024, i = 0;
    while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
    return `${v.toFixed(v < 10 ? 2 : 1)} ${u[i]} (${n.toLocaleString()} bytes)`;
  }
  async function openProperties(file: FileEntry) {
    propertiesFile = file; propertiesDetails = null;
    try { const d = await SystemBridge.getFileDetails(file.path); if (propertiesFile?.path === file.path) propertiesDetails = d; } catch { /* panel still shows basic info */ }
  }
  function closeProperties() { propertiesFile = null; propertiesDetails = null; }

  // --- Right-click menus (rendered by the shell-wide <ContextMenu/>) ----------------------
  // Every right-click inside Explorer is handled here: on a file/folder, on
  // empty space, on a sidebar bookmark and on a tab — the webview's native
  // "Back / Forward / Reload / Inspect" menu never shows (see globalContextMenu.ts).

  const openWithCache = new Map<string, { id: string; name: string; icon?: string; exec: string; recommended: boolean }[]>();
  async function openWithApps(mime: string) {
    const key = mime || 'application/octet-stream';
    if (!openWithCache.has(key)) {
      try { openWithCache.set(key, await SystemBridge.getOpenWithApps(key, true)); } catch { openWithCache.set(key, []); }
    }
    return openWithCache.get(key)!;
  }

  let homeCache = '';
  async function realPath(p: string): Promise<string> {
    if (p !== 'HOME' && !p.startsWith('HOME/')) return p;
    if (!homeCache) { try { homeCache = await SystemBridge.invokeCommand<string>('get_home_path'); } catch { homeCache = ''; } }
    return homeCache ? homeCache + p.slice(4) : p;
  }
  async function copyPathToClipboard(paths: string[]) {
    const real = await Promise.all(paths.map(realPath));
    try { await navigator.clipboard.writeText(real.join('\n')); notify('info', paths.length > 1 ? `Copied ${paths.length} paths` : 'Path copied'); }
    catch { notify('error', 'Could not access the clipboard'); }
  }

  async function createFile(defaultName = 'New File.txt', content = '', title = 'New File') {
    if (inTrash) return;
    const name = await dialogPrompt({ title, placeholder: defaultName, defaultValue: defaultName, confirmLabel: 'Create' });
    if (!name?.trim()) return;
    if (name.includes('/')) { notify('error', 'A name cannot contain "/"'); return; }
    if (files.some((f) => f.name === name.trim())) { notify('error', `"${name.trim()}" already exists`); return; }
    try { await SystemBridge.createTextFile(activeTab.path, name.trim(), content); notify('success', `Created: ${name.trim()}`); await loadFiles(activeTab.path); }
    catch { notify('error', 'Failed to create file'); }
  }

  async function trashSelected() {
    if (selected.size === 0) return;
    if (inTrash) return deleteTrashSelected(); // "Delete" inside the Trash = delete for good
    const items = [...selected];
    try { await SystemBridge.moveToTrash(items); notify('success', `Moved ${items.length} item(s) to Trash`); }
    catch (e) { notify('error', typeof e === 'string' ? e : 'Could not move to Trash'); }
    selected = new Set();
    loadFiles(activeTab.path);
    refreshTrashCount();
  }

  async function compressSelected(format: string) {
    if (!selected.size) return;
    notify('info', 'Compressing…');
    try { const out = await SystemBridge.compressFiles([...selected], format); notify('success', `Created ${out.split('/').pop()}`); loadFiles(activeTab.path); }
    catch (e) { notify('error', typeof e === 'string' ? e : 'Compression failed'); }
  }
  async function extractHere(file: FileEntry) {
    notify('info', 'Extracting…');
    try { const out = await SystemBridge.extractArchiveHere(file.path); notify('success', `Extracted to ${out.split('/').pop()}`); loadFiles(activeTab.path); }
    catch (e) { notify('error', typeof e === 'string' ? e : 'Extraction failed'); }
  }

  const NEW_TEMPLATES: { label: string; name: string; body: string }[] = [
    { label: 'Text file', name: 'New File.txt', body: '' },
    { label: 'Markdown', name: 'README.md', body: '# Title\n' },
    { label: 'H# source (.h#)', name: 'main.h#', body: ';; H# program\n' },
    { label: 'Hacker Lang (.hl)', name: 'script.hl', body: ';; Hacker Lang script\n' },
    { label: 'HackerScript (.hcs)', name: 'main.hcs', body: '!! HackerScript\n' },
    { label: 'HK config (.hk)', name: 'config.hk', body: '! HK config\n[config]\n' },
    { label: 'Hacker config (.hacker)', name: 'config.hacker', body: '# Hacker config\n' },
    { label: 'Blue manifest (.blue)', name: 'blue.blue', body: '! Blue manifest\n[package]\n' },
  ];

  async function buildFileMenu(file: FileEntry): Promise<MenuItem[]> {
    if (inTrash) return buildTrashMenu(file);
    const many = selected.size > 1;
    const items: MenuItem[] = [];
    const n = selected.size;

    items.push({ label: many ? `Open ${n} items` : 'Open', icon: Eye, action: () => { for (const p of selected) { const f = files.find((x) => x.path === p); if (f) handleOpen(f); } } });
    if (!many && file.is_dir) {
      items.push({ label: 'Open in new tab', icon: FolderOpen, action: () => { const id = `tab-${Date.now()}`; tabs = [...tabs, { id, path: file.path, history: [file.path], historyIndex: 0 }]; activeTabId = id; loadFiles(file.path); } });
    }

    if (!many && !file.is_dir) {
      const apps = await openWithApps(file.mime_type);
      const rec = apps.filter((a) => a.recommended);
      const other = apps.filter((a) => !a.recommended);
      const children: MenuItem[] = [
        { label: 'Notepad', icon: FileText, action: () => openApp(AppId.NOTEPAD, false, undefined, { openPath: file.path }) },
        { label: 'Blue Code', icon: FileText, action: () => openApp(AppId.BLUE_CODE, false, undefined, { openPath: file.path }) },
        { label: 'Blue Images', icon: FileText, action: () => openApp(AppId.BLUE_IMAGES, false, undefined, { openPath: file.path }) },
        { label: 'Blue Archive', icon: ArchiveIcon, action: () => openApp(AppId.BLUE_ARCHIVE, false, undefined, { openPath: file.path }) },
        { label: 'Blue Video', icon: FileText, action: () => openApp(AppId.BLUE_VIDEOS, false, undefined, { openPath: file.path }) },
        { label: 'Blue Music', icon: FileText, action: () => openApp(AppId.BLUE_MUSIC, false, undefined, { openPath: file.path }) },
      ];
      if (rec.length) { children.push({ separator: true }); for (const a of rec) children.push({ label: a.name, icon: AppWindow, action: () => SystemBridge.openWithApp(a.exec, file.path) }); }
      if (other.length) { children.push({ separator: true }); for (const a of other) children.push({ label: a.name, icon: AppWindow, action: () => SystemBridge.openWithApp(a.exec, file.path) }); }
      items.push({ label: 'Open with', icon: ExternalLink, children });
    }

    items.push({ separator: true });
    items.push({ label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X', action: cutSelected });
    items.push({ label: 'Copy', icon: Copy, shortcut: 'Ctrl+C', action: copySelected });
    items.push({ label: many ? 'Copy paths' : 'Copy path', icon: Link2, action: () => copyPathToClipboard([...selected]) });
    if (!many && file.is_dir) items.push({ label: 'Paste into folder', icon: ClipboardPaste, disabled: !clipboard, action: () => pasteInto(file.path) });
    items.push({ separator: true });
    if (!many) items.push({ label: 'Rename', icon: Edit, shortcut: 'F2', action: () => startRename(file) });

    items.push({
      label: 'Compress', icon: ArchiveIcon,
      children: [
        { label: 'ZIP (.zip)', action: () => compressSelected('zip') },
        { label: 'TAR.GZ (.tar.gz)', action: () => compressSelected('tar.gz') },
        { label: 'TAR.XZ (.tar.xz)', action: () => compressSelected('tar.xz') },
        { label: 'TAR.ZST (.tar.zst)', action: () => compressSelected('tar.zst') },
      ],
    });
    if (!many && isArchive(file)) items.push({ label: 'Extract here', icon: ArchiveRestore, action: () => extractHere(file) });

    if (!many && file.is_dir) {
      items.push({ label: isCustomBookmark(file.path) ? 'Remove from bookmarks' : 'Add to bookmarks', icon: isCustomBookmark(file.path) ? StarOff : Star, action: () => toggleCustomBookmark(file.path) });
    }

    items.push({ separator: true });
    items.push({ label: 'Move to Trash', icon: Trash2, shortcut: 'Del', danger: true, action: trashSelected });
    items.push({ label: 'Delete permanently', icon: X, shortcut: 'Shift+Del', danger: true, action: deleteSelected });
    items.push({ separator: true });
    items.push({ label: 'Properties', icon: Info, disabled: many, action: () => openProperties(file) });
    return items;
  }

  async function openContextMenu(e: MouseEvent, file: FileEntry) {
    e.preventDefault();
    e.stopPropagation();
    if (!selected.has(file.path)) selected = new Set([file.path]);
    const ev = { clientX: e.clientX, clientY: e.clientY };
    showContextMenu(ev, await buildFileMenu(file));
  }

  function openBackgroundMenu(e: MouseEvent) {
    // Only for empty space — file rows/tiles stop propagation themselves.
    e.preventDefault();
    selected = new Set();
    if (inTrash) {
      showContextMenu(e, [
        { label: 'Empty Trash', icon: Trash2, danger: true, action: emptyTrashAll },
        { separator: true },
        { label: 'Select all', icon: CheckSquare, shortcut: 'Ctrl+A', action: () => (selected = new Set(sorted.map((f) => f.path))) },
        { label: 'Refresh', icon: RefreshCw, shortcut: 'F5', action: () => loadFiles(TRASH_PATH) },
      ]);
      return;
    }
    const cur = activeTab.path;
    const here: FileEntry = { name: cur === 'HOME' ? '~' : (cur.split('/').pop() || '/'), path: cur, is_dir: true, size: '', mime_type: 'inode/directory' };
    showContextMenu(e, [
      {
        label: 'Create new', icon: FilePlus,
        children: [
          { label: 'Folder…', icon: FolderPlus, shortcut: 'Ctrl+N', action: createFolder },
          { separator: true },
          ...NEW_TEMPLATES.map((t) => ({ label: t.label + '…', icon: FilePlus, action: () => createFile(t.name, t.body, `New ${t.label}`) })),
        ],
      },
      { separator: true },
      { label: 'Paste', icon: ClipboardPaste, shortcut: 'Ctrl+V', disabled: !clipboard, action: paste },
      { label: 'Select all', icon: CheckSquare, shortcut: 'Ctrl+A', action: () => (selected = new Set(sorted.map((f) => f.path))) },
      { separator: true },
      {
        label: 'View', icon: Grid,
        children: [
          { label: 'Icons', icon: Grid, checked: viewMode === 'grid', action: () => (viewMode = 'grid') },
          { label: 'List', icon: List, checked: viewMode === 'list', action: () => (viewMode = 'list') },
        ],
      },
      {
        label: 'Sort by', icon: SortAsc,
        children: [
          ...(['name', 'size', 'modified', 'type'] as SortKey[]).map((k) => ({ label: k[0].toUpperCase() + k.slice(1), checked: sortBy === k, action: () => { sortBy = k; } })),
          { separator: true },
          { label: 'Ascending', checked: sortAsc, action: () => (sortAsc = true) },
          { label: 'Descending', checked: !sortAsc, action: () => (sortAsc = false) },
        ],
      },
      { label: 'Show hidden files', icon: Eye, checked: showHidden, action: () => (showHidden = !showHidden) },
      { separator: true },
      { label: 'Copy folder path', icon: Link2, action: () => copyPathToClipboard([cur]) },
      { label: 'Refresh', icon: RefreshCw, shortcut: 'F5', action: () => loadFiles(cur) },
      { label: 'Properties', icon: Info, action: () => openProperties(here) },
    ]);
  }

  function openBookmarkMenu(e: MouseEvent, path: string, removable = false) {
    showContextMenu(e, [
      { label: 'Open', icon: FolderOpen, action: () => navigateTo(path) },
      { label: 'Open in new tab', icon: ExternalLink, action: () => { const id = `tab-${Date.now()}`; tabs = [...tabs, { id, path, history: [path], historyIndex: 0 }]; activeTabId = id; loadFiles(path); } },
      { label: 'Copy path', icon: Link2, action: () => copyPathToClipboard([path]) },
      ...(removable ? [{ separator: true } as MenuItem, { label: 'Remove bookmark', icon: StarOff, danger: true, action: () => toggleCustomBookmark(path) } as MenuItem] : []),
    ]);
  }

  function openTabMenu(e: MouseEvent, t: Tab) {
    showContextMenu(e, [
      { label: 'New tab', icon: Plus, shortcut: 'Ctrl+T', action: addTab },
      { label: 'Duplicate tab', icon: Copy, action: () => { const id = `tab-${Date.now()}`; tabs = [...tabs, { id, path: t.path, history: [t.path], historyIndex: 0 }]; activeTabId = id; loadFiles(t.path); } },
      { separator: true },
      { label: 'Close tab', icon: X, disabled: tabs.length === 1, action: () => closeTab(t.id) },
      { label: 'Close other tabs', disabled: tabs.length === 1, action: () => { tabs = tabs.filter((x) => x.id === t.id); activeTabId = t.id; loadFiles(t.path); } },
    ]);
  }

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

  // --- Trash (Blue's own: ~/.cache/Blue-Environment/trash/, see trash.rs) -------------------
  // Shown as the virtual location `trash://`; every item inside is `trash://<id>`.
  $: inTrash = activeTab?.path === TRASH_PATH;
  let trashEntries = new Map<string, TrashEntry>();
  let trashCount = 0;
  const trashIdOf = (path: string) => path.slice(TRASH_PATH.length);

  async function refreshTrashCount() {
    try { trashCount = await SystemBridge.trashItemCount(); } catch { /* badge only */ }
  }
  onMount(refreshTrashCount);

  function trashToFileEntry(t: TrashEntry): FileEntry {
    return {
      name: t.name, path: TRASH_PATH + t.id, is_dir: t.is_dir,
      size: t.is_dir ? 'DIR' : `${(t.size_bytes / 1024).toFixed(1)} KB`,
      mime_type: t.is_dir ? 'inode/directory' : 'application/octet-stream',
      modified: t.deleted_at ? t.deleted_at.replace('T', ' ').slice(0, 16) : undefined,
    };
  }

  async function restoreSelected() {
    if (!inTrash || !selected.size) return;
    const ids = [...selected].map(trashIdOf);
    try {
      const back = await SystemBridge.restoreFromTrash(ids);
      notify('success', ids.length === 1 && back[0] ? `Restored to ${back[0]}` : `Restored ${ids.length} item(s)`);
    } catch (e) { notify('error', typeof e === 'string' ? e : 'Could not restore'); }
    selected = new Set();
    await loadFiles(TRASH_PATH);
  }

  async function deleteTrashSelected() {
    if (!inTrash || !selected.size) return;
    const ok = await dialogConfirm({ title: 'Delete permanently', message: `Permanently delete ${selected.size} item(s) from the Trash? This cannot be undone.`, confirmLabel: 'Delete', danger: true });
    if (!ok) return;
    try { await SystemBridge.deleteFromTrash([...selected].map(trashIdOf)); notify('success', `Deleted ${selected.size} item(s)`); }
    catch (e) { notify('error', typeof e === 'string' ? e : 'Could not delete'); }
    selected = new Set();
    await loadFiles(TRASH_PATH);
  }

  async function emptyTrashAll() {
    if (trashCount === 0 && !files.length) { notify('info', 'The Trash is already empty'); return; }
    const ok = await dialogConfirm({ title: 'Empty Trash', message: 'Permanently delete everything in the Trash? This cannot be undone.', confirmLabel: 'Empty Trash', danger: true });
    if (!ok) return;
    try { const n = await SystemBridge.emptyTrash(); notify('success', `Trash emptied (${n} item${n === 1 ? '' : 's'})`); }
    catch (e) { notify('error', typeof e === 'string' ? e : 'Could not empty the Trash'); }
    selected = new Set();
    await loadFiles(TRASH_PATH);
  }

  /** Dropping files on the sidebar's Trash entry moves them there. */
  async function onDropToTrash(e: DragEvent) {
    e.preventDefault(); e.stopPropagation(); dragOver = null;
    if (inTrash) return;
    try {
      const paths: string[] = JSON.parse(e.dataTransfer?.getData('text/plain') ?? '[]');
      if (!paths.length) return;
      await SystemBridge.moveToTrash(paths);
      notify('success', `Moved ${paths.length} item(s) to Trash`);
      selected = new Set();
      loadFiles(activeTab.path);
    } catch (err) { notify('error', typeof err === 'string' ? err : 'Could not move to Trash'); }
  }

  async function buildTrashMenu(file: FileEntry): Promise<MenuItem[]> {
    const t = trashEntries.get(trashIdOf(file.path));
    const n = selected.size;
    return [
      { label: n > 1 ? `Restore ${n} items` : 'Restore', icon: ArchiveRestore, action: restoreSelected },
      { label: 'Delete permanently', icon: X, shortcut: 'Del', danger: true, action: deleteTrashSelected },
      { separator: true },
      { label: 'Show original location', icon: Info, disabled: n > 1 || !t?.original_path, action: () => notify('info', t?.original_path ?? '') },
      { label: 'Copy original path', icon: Link2, disabled: n > 1 || !t?.original_path, action: () => copyPathToClipboard([t?.original_path ?? '']) },
      { separator: true },
      { label: 'Empty Trash', icon: Trash2, danger: true, action: emptyTrashAll },
    ];
  }

  async function loadFiles(path: string) {
    loading = true; selected = new Set(); previewFile = null;
    if (path === TRASH_PATH) {
      try {
        const list = await SystemBridge.listTrash();
        trashEntries = new Map(list.map((t) => [t.id, t]));
        files = list.map(trashToFileEntry);
        trashCount = list.length;
      } catch (e) { files = []; notify('error', typeof e === 'string' ? e : 'Cannot open the Trash'); }
      finally { loading = false; }
      return;
    }
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
  onMount(() => { ensureFileTypeAssociationsLoaded(); });

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
    if (cur === TRASH_PATH) { navigateTo('HOME'); return; }
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

  /** `.blue` is either a Blue Store package (tar+zstd containing `blue.hk`)
   * or a plain-text HK-style description. For a package we pull out just the
   * manifest into a temp file and show it highlighted — nothing is installed
   * or executed by merely opening it. */
  async function openBlueFile(file: FileEntry) {
    const editorApp = (configStore.get().defaultTextEditor ?? 'notepad') === 'blue_code' ? AppId.BLUE_CODE : AppId.NOTEPAD;
    try {
      const tmp = `/tmp/blue-manifest-${file.name.replace(/[^A-Za-z0-9._-]/g, '_')}.blue`;
      const r: any = await SystemBridge.executeCommand(
        `tar --zstd -xOf ${shellQuote(file.path)} blue.hk > ${shellQuote(tmp)} 2>/dev/null && [ -s ${shellQuote(tmp)} ] && echo OK`
      );
      const ok = (typeof r === 'string' ? r : (r?.stdout ?? '')).includes('OK');
      if (ok) { notify('info', `Showing manifest of ${file.name} (blue.hk)`); openApp(editorApp, false, undefined, { openPath: tmp }); return; }
    } catch { /* not an archive → treat as text */ }
    openApp(editorApp, false, undefined, { openPath: file.path });
  }

  function handleOpen(file: FileEntry) {
    if (inTrash) { notify('info', 'Restore this item first to open it (right-click → Restore)'); return; }
    if (file.is_dir) { navigateTo(file.path); return; }

    // A user-defined "open with" (Settings > Custom File Types) takes
    // priority over every built-in rule below — that's the whole point
    // of letting someone associate their own file format with their own
    // command, the way KDE's file-associations settings do.
    const customMatch = resolveAssociationLocally(file.name, file.mime_type, get(fileTypeAssociations));
    if (customMatch?.open_with_command) {
      const cmd = customMatch.open_with_command.replaceAll('{path}', shellQuote(file.path));
      SystemBridge.executeCommand(cmd).catch(() => notify('error', `Failed to open with custom command: ${customMatch.label || customMatch.pattern}`));
      return;
    }

    // Everything else: the shell's own apps are the defaults (images → Blue Images,
    // archives → Blue Archive, video/audio → Blue Video/Music, text & Hacker* → editor).
    openFileWithDefaultApp(file, () => openBlueFile(file));
  }

  async function createFolder() {
    if (inTrash) { notify('info', 'You cannot create items in the Trash'); return; }
    const name = await dialogPrompt({ title: 'New Folder', placeholder: 'Untitled Folder', defaultValue: 'New Folder', confirmLabel: 'Create' });
    if (!name?.trim()) return;
    try { await SystemBridge.createFolder(activeTab.path, name.trim()); notify('success', `Created: ${name}`); loadFiles(activeTab.path); }
    catch { notify('error', 'Failed to create folder'); }
  }

  async function deleteSelected() {
    if (selected.size === 0) return;
    if (inTrash) return deleteTrashSelected();
    const ok = await dialogConfirm({ title: 'Delete items', message: `Delete ${selected.size} item(s)? This cannot be undone.`, confirmLabel: 'Delete', danger: true });
    if (!ok) return;
    let errors = 0;
    for (const path of selected) { try { await SystemBridge.deleteFile(path); } catch { errors++; } }
    notify(errors ? 'error' : 'success', errors ? `${errors} item(s) failed` : `Deleted ${selected.size} item(s)`);
    selected = new Set();
    loadFiles(activeTab.path);
  }

  function copySelected() { if (!selected.size || inTrash) return; clipboard = { action: 'copy', files: [...selected] }; notify('info', `Copied ${selected.size} item(s)`); }
  function cutSelected() { if (!selected.size || inTrash) return; clipboard = { action: 'cut', files: [...selected] }; notify('info', `Cut ${selected.size} item(s)`); }

  async function paste() { return pasteInto(activeTab.path); }
  async function pasteInto(destDir: string) {
    if (!clipboard || inTrash) return;
    let errors = 0;
    for (const src of clipboard.files) {
      const name = src.split('/').pop() ?? '';
      const dst = `${destDir}/${name}`;
      try { if (clipboard.action === 'copy') await SystemBridge.copyFile(src, dst); else await SystemBridge.moveFile(src, dst); }
      catch { errors++; }
    }
    notify(errors ? 'error' : 'success', errors ? `${errors} item(s) failed` : `Pasted ${clipboard.files.length} item(s)`);
    if (clipboard.action === 'cut') clipboard = null;
    loadFiles(activeTab.path);
  }

  async function startRename(file: FileEntry) {
    if (inTrash) return;
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
    if (inTrash) { e.preventDefault(); return; }
    const paths = selected.has(file.path) ? [...selected] : [file.path];
    e.dataTransfer?.setData('text/plain', JSON.stringify(paths));
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copyMove';
  }
  async function onDrop(e: DragEvent, targetDir: FileEntry | null) {
    e.preventDefault(); dragOver = null;
    if (inTrash) return;
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
    // Same fix as Desktop.svelte: while a "New Folder"/rename/etc. modal
    // dialog is open, this window-wide listener must not intercept
    // Delete/Ctrl+A/Ctrl+N and friends — otherwise pressing Delete while
    // typing a name here deleted the *currently selected files* instead
    // of editing the dialog's text, and Ctrl+A/etc. fired against the
    // Explorer's own file list instead of the input.
    if (get(activeDialog)) return;
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
    if (e.key === 'Delete') { e.preventDefault(); if (e.shiftKey) deleteSelected(); else trashSelected(); }
    if (e.key === 'F5') { e.preventDefault(); loadFiles(activeTab.path); }
    if (e.key === 'Escape') { searchTerm = ''; showSearch = false; selected = new Set(); }
  }
  onMount(() => window.addEventListener('keydown', handleKeyDown));
  onDestroy(() => window.removeEventListener('keydown', handleKeyDown));

  $: breadcrumbs = inTrash ? [{ label: 'Trash', path: TRASH_PATH }] : activeTab.path.split('/').map((part, i, arr) => ({
    label: part === 'HOME' ? '~' : part,
    path: arr.slice(0, i + 1).join('/') || '/',
  }));
</script>

<div class="flex h-full text-white overflow-hidden {isHydra ? 'bg-[#12071f]' : 'bg-slate-900'}" on:dragover={(e) => e.preventDefault()} on:drop={(e) => onDrop(e, null)}>
  <div class="fixed top-16 right-4 z-50 flex flex-col gap-2 pointer-events-none">
    {#each notifs as n (n.id)}
      <div class="flex items-center gap-2 px-4 py-2 rounded-xl shadow-lg text-sm {n.type === 'success' ? 'bg-green-600/90' : n.type === 'error' ? 'bg-red-600/90' : 'bg-slate-700/90'}">
        {#if n.type === 'success'}<Check size={13} />{:else if n.type === 'error'}<AlertCircle size={13} />{:else}<Info size={13} />{/if}
        {n.message}
      </div>
    {/each}
  </div>

  <div class="w-44 border-r flex flex-col shrink-0 {isHydra ? 'bg-[#1d0f2e]/60 border-pink-500/10' : 'bg-slate-800/50 border-white/5'}">
    <div class="p-3 border-b border-white/5 flex items-center justify-between">
      <span class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Places</span>
      <button on:click={() => toggleCustomBookmark(activeTab.path)} title="Bookmark current folder" class="p-0.5 hover:text-yellow-400 text-slate-500">
        {#if isCustomBookmark(activeTab.path)}<Star size={13} class="text-yellow-400 fill-yellow-400" />{:else}<Star size={13} />{/if}
      </button>
    </div>
    <div class="flex-1 overflow-y-auto p-2 space-y-0.5">
      {#each BOOKMARKS_WITH_ICONS as bm (bm.path)}
        <button on:click={() => navigateTo(bm.path)} on:contextmenu={(e) => openBookmarkMenu(e, bm.path)} class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm transition-colors {activeTab.path === bm.path ? selClassSubtle : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
          <svelte:component this={bm.icon} size={14} />{bm.name}
        </button>
      {/each}
      <div class="h-px bg-white/5 my-2" />
      <button on:click={() => navigateTo('/')} on:contextmenu={(e) => openBookmarkMenu(e, '/')} class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm text-slate-400 hover:bg-white/5 hover:text-white">
        <HardDrive size={14} /> / (root)
      </button>
      <button on:click={() => navigateTo(TRASH_PATH)}
        on:dragover={(e) => { e.preventDefault(); dragOver = TRASH_PATH; }} on:dragleave={() => (dragOver = null)} on:drop={onDropToTrash}
        title="Trash — drop files here to delete them safely"
        class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm transition-colors {inTrash ? selClassSubtle : 'text-slate-400 hover:bg-white/5 hover:text-white'} {dragOver === TRASH_PATH ? dragTgtClassRow : ''}">
        <Trash2 size={14} /> Trash
        {#if trashCount > 0}<span class="ml-auto text-[10px] px-1.5 rounded-full bg-white/10 text-slate-300">{trashCount}</span>{/if}
      </button>
      {#if customBookmarks.length}
        <div class="h-px bg-white/5 my-2" />
        <div class="px-2 pb-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">Bookmarks</div>
        {#each customBookmarks as path (path)}
          <div on:contextmenu={(e) => openBookmarkMenu(e, path, true)} class="group w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-sm transition-colors {activeTab.path === path ? selClassSubtle : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
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
        <div on:contextmenu={(e) => openTabMenu(e, t)} on:click={() => { activeTabId = t.id; loadFiles(t.path); }} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); (() => { activeTabId = t.id; loadFiles(t.path); })(); } }}
          class="flex items-center gap-1.5 px-3 py-2 cursor-pointer border-r border-white/5 shrink-0 group max-w-[140px] {t.id === activeTabId ? 'bg-slate-900 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700/50'}">
          <Folder size={12} />
          <span class="text-xs truncate">{t.path === 'HOME' ? '~' : t.path === TRASH_PATH ? 'Trash' : t.path.split('/').pop()}</span>
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
      <button on:click={trashSelected} disabled={!selected.size} title={inTrash ? 'Delete permanently (Del)' : 'Move to Trash (Del) — Shift+Del deletes permanently'} class="p-1.5 hover:bg-white/10 rounded disabled:opacity-30"><Trash2 size={15} /></button>
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

    {#if inTrash}
      <div class="flex items-center gap-2 px-3 py-1.5 bg-slate-800/60 border-b border-white/5 text-xs text-slate-400 shrink-0">
        <Trash2 size={13} class="shrink-0" />
        <span class="truncate">Trash — items stay here until you restore or delete them for good.</span>
        <div class="ml-auto flex items-center gap-1 shrink-0">
          <button on:click={restoreSelected} disabled={!selected.size} class="flex items-center gap-1 px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-30"><ArchiveRestore size={12} /> Restore</button>
          <button on:click={deleteTrashSelected} disabled={!selected.size} class="flex items-center gap-1 px-2 py-1 rounded bg-white/5 hover:bg-red-500/20 text-red-300 disabled:opacity-30"><X size={12} /> Delete</button>
          <button on:click={emptyTrashAll} disabled={!files.length} class="flex items-center gap-1 px-2 py-1 rounded bg-red-500/15 hover:bg-red-500/30 text-red-300 disabled:opacity-30"><Trash2 size={12} /> Empty Trash</button>
        </div>
      </div>
    {/if}

    <div class="flex-1 flex overflow-hidden">
      <div bind:this={gridEl} class="flex-1 overflow-auto p-2" on:contextmenu={openBackgroundMenu} on:click={(e) => { if (e.target === gridEl) selected = new Set(); }} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); ((e) => { if (e.target === gridEl) selected = new Set(); })(e); } }}>
        {#if loading}
          <div class="flex items-center justify-center h-full"><Loader2 size={22} class="animate-spin text-blue-400" /></div>
        {:else if sorted.length === 0}
          <div class="flex flex-col items-center justify-center h-full text-slate-600 gap-2"><Folder size={36} /><span class="text-sm">{searchTerm ? 'No results' : inTrash ? 'The Trash is empty' : 'Empty folder'}</span></div>
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
                class="relative flex flex-col items-center p-2 rounded-xl cursor-pointer transition-colors duration-100 select-none {isSel ? selClass : 'hover:bg-white/5'} {isDragTgt ? dragTgtClassGrid : ''} {isCut ? 'opacity-50' : ''}"
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
                  class="border-b border-white/5 cursor-pointer group {isSel ? selClassSubtle : 'hover:bg-white/5'} {isDragTgt ? dragTgtClassRow : ''} {isCut ? 'opacity-50' : ''}"
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
                      <button on:click|stopPropagation={() => { selected = new Set([file.path]); setTimeout(trashSelected, 0); }} class="p-1 hover:bg-red-500/20 rounded text-red-400"><Trash2 size={12} /></button>
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

      {#if propertiesFile}
        {@const pf = propertiesFile}
        <div class="fixed inset-0 z-[70] bg-black/50 flex items-center justify-center" on:click={closeProperties} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); closeProperties(); } }}>
          <div class="w-80 bg-slate-800 border rounded-xl shadow-2xl p-4 {isHydra ? 'border-pink-500/20' : 'border-white/10'}" on:click|stopPropagation>
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
              <div class="flex justify-between"><span class="text-slate-500">Modified</span><span>{propertiesDetails?.modified ?? pf.modified ?? '—'}</span></div>
              {#if propertiesDetails}
                <div class="flex justify-between"><span class="text-slate-500">Exact size</span><span class="text-right">{formatBytes(propertiesDetails.size_bytes)}</span></div>
                <div class="flex justify-between"><span class="text-slate-500">Accessed</span><span>{propertiesDetails.accessed}</span></div>
                <div class="flex justify-between"><span class="text-slate-500">Permissions</span><span class="font-mono">{propertiesDetails.permissions}</span></div>
                <div class="flex justify-between"><span class="text-slate-500">Owner</span><span>{propertiesDetails.owner}:{propertiesDetails.group}</span></div>
                {#if propertiesDetails.symlink_target}<div class="flex justify-between"><span class="text-slate-500">Link to</span><span class="truncate max-w-[180px]" title={propertiesDetails.symlink_target}>{propertiesDetails.symlink_target}</span></div>{/if}
              {/if}
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
