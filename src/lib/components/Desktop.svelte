<script lang="ts">
  import { transferWithDialog, createWithDialog, renameWithDialog, describeSummary, type TransferSummary } from '../utils/fileTransfer';
  import { openFileWithDefaultApp } from '../utils/openFile';
  import { createEventDispatcher, onMount, tick } from 'svelte';
  import { FolderPlus, FilePlus, ClipboardPaste, RefreshCw, Image as ImageIcon, LayoutGrid } from 'lucide-svelte';
  import { SystemBridge, toAssetUrl, shellQuote } from '../utils/systemBridge';
  import { t } from '../stores/language';
  import {
    CELL_W, CELL_H, gridSize, cellAt, cellOrigin, placeAll, moveGroup, renameKey, prune, parseLayout, uniqueName,
    type Layout, type Cell,
  } from './desktopLayout';
  import { dialogPrompt, dialogConfirm, activeDialog } from '../stores/dialog';
  import { get } from 'svelte/store';
  import { openApp } from '../stores/windowManager';
  import { AppId } from '../types';
  import FileIcon from './apps/Explorer-App/FileIcon.svelte';
  import type { FileEntry, Notif } from './apps/Explorer-App/types';

  export let desktopPath = 'HOME/Desktop';

  const dispatch = createEventDispatcher<{ closeMenus: void }>();

  let files: FileEntry[] = [];
  let iconEls: Record<string, HTMLElement> = {};
  let containerEl: HTMLDivElement;

  let selected = new Set<string>();
  let renaming: string | null = null;
  let renameVal = '';
  let renameEl: HTMLInputElement;

  let clipboard: { action: 'copy' | 'cut'; files: string[] } | null = null;
  let notifs: Notif[] = [];

  let marquee: { x0: number; y0: number; x1: number; y1: number } | null = null;
  let didDrag = false;

  let ctxMenu: { x: number; y: number; target: FileEntry | null } | null = null;

  // ── Icon grid / remembered positions ─────────────────────────────────────
  const LAYOUT_PATH = 'HOME/.config/Blue-Environment/desktop-layout.json';
  let saved: Layout = {};
  let cols = 12, rows = 8;
  let layoutLoaded = false;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  $: layout = placeAll(files.map((f) => f.name), saved, cols, rows);

  function measureGrid() {
    if (!containerEl) return;
    const r = containerEl.getBoundingClientRect();
    ({ cols, rows } = gridSize(r.width, r.height));
  }

  async function loadLayout() {
    try { saved = parseLayout(await SystemBridge.readFile(LAYOUT_PATH)); } catch { saved = {}; }
    layoutLoaded = true;
  }
  function persistLayout() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => { SystemBridge.writeFile(LAYOUT_PATH, JSON.stringify(saved)).catch(() => {}); }, 300);
  }
  /** Commit the CURRENT effective positions plus `next` (so auto-flowed icons stay put once the user rearranges). */
  function commitLayout(next: Layout) {
    saved = prune(next, files.map((f) => f.name));
    persistLayout();
  }
  function arrangeIcons() {
    closeMenu();
    saved = {};
    persistLayout();
  }

  function notify(type: Notif['type'], message: string) {
    const id = `${Date.now()}-${Math.random()}`;
    notifs = [...notifs, { id, type, message }];
    setTimeout(() => (notifs = notifs.filter((n) => n.id !== id)), 3500);
  }

  async function loadFiles() {
    try {
      const list = await SystemBridge.getFiles(desktopPath);
      files = (list as FileEntry[]).slice().sort((a, b) => a.name.localeCompare(b.name));
    } catch {
      files = [];
    }
  }

  onMount(() => {
    loadFiles();
    loadLayout();
    measureGrid();
    const ro = new ResizeObserver(measureGrid);
    ro.observe(containerEl);
    // Other apps (Explorer…) announce changes they make to the desktop folder.
    const onFsChanged = () => loadFiles();
    window.addEventListener('blue-fs-changed', onFsChanged);
    return () => { ro.disconnect(); window.removeEventListener('blue-fs-changed', onFsChanged); clearTimeout(saveTimer); };
  });

  $: if (desktopPath) loadFiles();

  // ── Selection ────────────────────────────────────────────────────────────
  function selectOnly(path: string) { selected = new Set([path]); }
  function toggleSelect(path: string) {
    const next = new Set(selected);
    if (next.has(path)) next.delete(path); else next.add(path);
    selected = next;
  }

  function iconClick(e: MouseEvent, file: FileEntry) {
    e.stopPropagation();
    if (suppressClick) return;
    if (e.ctrlKey || e.metaKey || e.shiftKey) { if (wasSelectedOnDown) toggleSelect(file.path); }
    else selectOnly(file.path);
  }

  function handleOpen(file: FileEntry) {
    closeMenu();
    // Open Files AT the double-clicked folder (it used to open at Home, ignoring the folder).
    if (file.is_dir) { openApp(AppId.EXPLORER, false, undefined, { initialPath: file.path }, file.name); return; }
    // Application shortcuts on the desktop: launch them, don't open them as text.
    if (file.name.endsWith('.desktop')) {
      SystemBridge.executeCommand(`gio launch ${shellQuote(file.path)} >/dev/null 2>&1 || dex ${shellQuote(file.path)} >/dev/null 2>&1 &`).catch(() => {});
      return;
    }
    // Same resolver Files uses (Blue Images/Video/Music/Archive/Notepad…), xdg-open only as a last resort.
    openFileWithDefaultApp(file).catch(() => notify('error', 'Nie udało się otworzyć pliku'));
  }

  // ── Dragging icons (pointer-based: deterministic, snaps to the grid) ─────
  interface IconDrag { names: string[]; anchor: string; startX: number; startY: number; dx: number; dy: number; active: boolean; offX: number; offY: number }
  let drag: IconDrag | null = null;
  let dropFolder: string | null = null;   // desktop folder icon currently hovered while dragging
  let ghostCell: Cell | null = null;      // where the group will land
  let suppressClick = false;
  let wasSelectedOnDown = false;

  function onIconMouseDown(e: MouseEvent, file: FileEntry) {
    if (e.button !== 0 || renaming) return;
    e.stopPropagation();
    dispatch('closeMenus');
    ctxMenu = null;
    const mod = e.ctrlKey || e.metaKey || e.shiftKey;
    wasSelectedOnDown = selected.has(file.path);
    if (!wasSelectedOnDown) { if (mod) toggleSelect(file.path); else selectOnly(file.path); }

    const crect = containerEl.getBoundingClientRect();
    const origin = cellOrigin(layout[file.name] ?? { col: 0, row: 0 });
    const names = files.filter((f) => selected.has(f.path)).map((f) => f.name);
    drag = {
      names: names.length ? names : [file.name], anchor: file.name,
      startX: e.clientX, startY: e.clientY, dx: 0, dy: 0, active: false,
      // pointer position inside the anchor's cell → keeps the icon under the cursor
      offX: e.clientX - crect.left - origin.left, offY: e.clientY - crect.top - origin.top,
    };
    window.addEventListener('mousemove', onDragMove);
    window.addEventListener('mouseup', onDragEnd, { once: true });
  }

  function folderUnder(x: number, y: number): FileEntry | null {
    const el = document.elementFromPoint(x, y)?.closest?.('[data-desktop-icon][data-is-dir="true"]') as HTMLElement | null;
    if (!el || !drag) return null;
    const f = files.find((q) => q.path === el.dataset.path);
    return f && !drag.names.includes(f.name) ? f : null;
  }

  function onDragMove(e: MouseEvent) {
    if (!drag) return;
    drag.dx = e.clientX - drag.startX; drag.dy = e.clientY - drag.startY;
    if (!drag.active && Math.hypot(drag.dx, drag.dy) > 4) drag.active = true;
    if (drag.active) {
      const crect = containerEl.getBoundingClientRect();
      dropFolder = folderUnder(e.clientX, e.clientY)?.name ?? null;
      ghostCell = dropFolder ? null : cellAt(e.clientX - crect.left - drag.offX + CELL_W / 2, e.clientY - crect.top - drag.offY + CELL_H / 2, cols, rows);
    }
    drag = drag; // notify Svelte
  }

  /** Kopiuje/przenosi do `destDir` z dialogiem konfliktów. `null` = anulowano. */
  async function moveInto(destDir: string, names: string[], copy: boolean): Promise<TransferSummary | null> {
    const sources = names.map((name) => files.find((f) => f.name === name)?.path ?? `${desktopPath}/${name}`);
    try { return await transferWithDialog(sources, destDir, copy ? 'copy' : 'move'); }
    catch (err) { return { done: 0, skipped: 0, errors: [String(err)] }; }
  }
  function reportTransfer(r: TransferSummary | null, copy: boolean) {
    if (!r) return;
    const d = describeSummary(r, copy ? 'copy' : 'move');
    notify(d.type === 'info' ? 'info' : d.type, d.message);
  }

  async function onDragEnd(e: MouseEvent) {
    window.removeEventListener('mousemove', onDragMove);
    const d = drag; const folder = dropFolder; const cell = ghostCell;
    drag = null; dropFolder = null; ghostCell = null;
    if (!d) return;
    if (!d.active) return; // plain click — handled by `iconClick`
    suppressClick = true; setTimeout(() => (suppressClick = false), 0);

    const target = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;

    // 1) dropped on a folder icon of the desktop → move into it
    const folderFile = folder ? files.find((f) => f.name === folder) : null;
    if (folderFile) {
      reportTransfer(await moveInto(folderFile.path, d.names, e.ctrlKey), e.ctrlKey);
      selected = new Set(); await loadFiles(); return;
    }
    // 2) dropped on another app's folder / file pane (e.g. an open Files window)
    const dirEl = target?.closest?.('[data-drop-dir]') as HTMLElement | null;
    if (dirEl && !containerEl.contains(target)) {
      const dest = dirEl.dataset.dropDir!;
      reportTransfer(await moveInto(dest, d.names, e.ctrlKey), e.ctrlKey);
      selected = new Set(); await loadFiles();
      window.dispatchEvent(new CustomEvent('blue-fs-changed', { detail: { path: dest } }));
      return;
    }
    // 3) dropped on the bare desktop → snap to the grid
    if (cell && target && containerEl.contains(target)) {
      commitLayout(moveGroup(layout, d.names, d.anchor, cell, cols, rows));
    }
    // otherwise (released over some other window) → cancelled, icons snap back
  }

  // Files dragged IN from Files (HTML5 drag, `text/plain` = JSON path list)
  function onDesktopDragOver(e: DragEvent) {
    if (e.dataTransfer?.types.includes('text/plain')) { e.preventDefault(); if (e.dataTransfer) e.dataTransfer.dropEffect = e.ctrlKey ? 'copy' : 'move'; }
  }
  async function onDesktopDrop(e: DragEvent) {
    e.preventDefault();
    let paths: string[] = [];
    try { paths = JSON.parse(e.dataTransfer?.getData('text/plain') ?? '[]'); } catch { return; }
    if (!Array.isArray(paths) || !paths.length) return;
    const inDesktop = new Set(files.map((f) => f.path));
    const incoming = paths.filter((p) => typeof p === 'string' && !inDesktop.has(p));
    if (!incoming.length) return;
    const crect = containerEl.getBoundingClientRect();
    const dropCell = cellAt(e.clientX - crect.left, e.clientY - crect.top, cols, rows);
    const before = new Set(files.map((f) => f.name));
    let errors = 0;
    try {
      const r = await transferWithDialog(incoming, desktopPath, e.ctrlKey ? 'copy' : 'move');
      if (!r) return;                       // anulowano w dialogu konfliktów
      errors = r.errors.length;
    } catch { errors = incoming.length; }
    await loadFiles();
    const placed = files.map((f) => f.name).filter((n) => !before.has(n));   // nowo doszłe (także „(2)” po Zachowaj oba)
    if (placed.length) {
      // put the newcomers where they were dropped (nearest free cells)
      const base = placeAll(files.map((f) => f.name), saved, cols, rows);
      const tmp: Layout = { ...base };
      placed.forEach((n, i) => { tmp[n] = { col: Math.min(cols - 1, dropCell.col + Math.floor(i / rows)), row: (dropCell.row + i) % rows }; });
      commitLayout(moveGroup(tmp, placed, placed[0], tmp[placed[0]], cols, rows));
    }
    notify(errors ? 'error' : 'success', errors ? `${errors} element(y) nie powiodło się` : `${e.ctrlKey ? 'Skopiowano' : 'Przeniesiono'} ${placed.length} element(y) na pulpit`);
    window.dispatchEvent(new CustomEvent('blue-fs-changed'));
  }

  // ── Rubber-band marquee selection ───────────────────────────────────────
  function onContainerMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('[data-desktop-icon]')) return;
    dispatch('closeMenus');
    ctxMenu = null;
    didDrag = false;
    const rect = containerEl.getBoundingClientRect();
    const x0 = e.clientX - rect.left, y0 = e.clientY - rect.top;
    marquee = { x0, y0, x1: x0, y1: y0 };
    if (!e.ctrlKey && !e.metaKey && !e.shiftKey) selected = new Set();

    const onMove = (ev: MouseEvent) => {
      const x1 = ev.clientX - rect.left, y1 = ev.clientY - rect.top;
      if (Math.abs(x1 - x0) > 3 || Math.abs(y1 - y0) > 3) didDrag = true;
      marquee = { x0, y0, x1, y1 };
      updateMarqueeSelection();
    };
    const onUp = () => {
      marquee = null;
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function updateMarqueeSelection() {
    if (!marquee || !containerEl) return;
    const mx0 = Math.min(marquee.x0, marquee.x1), mx1 = Math.max(marquee.x0, marquee.x1);
    const my0 = Math.min(marquee.y0, marquee.y1), my1 = Math.max(marquee.y0, marquee.y1);
    const crect = containerEl.getBoundingClientRect();
    const next = new Set<string>();
    for (const file of files) {
      const el = iconEls[file.path];
      if (!el) continue;
      const r = el.getBoundingClientRect();
      const ix0 = r.left - crect.left, iy0 = r.top - crect.top;
      const ix1 = r.right - crect.left, iy1 = r.bottom - crect.top;
      if (ix0 < mx1 && ix1 > mx0 && iy0 < my1 && iy1 > my0) next.add(file.path);
    }
    selected = next;
  }

  function onContainerClick() {
    if (didDrag) { didDrag = false; return; }
    selected = new Set();
    ctxMenu = null;
    dispatch('closeMenus');
  }

  // ── Context menu ─────────────────────────────────────────────────────────
  function openMenu(e: MouseEvent, file: FileEntry | null) {
    e.preventDefault();
    e.stopPropagation();
    dispatch('closeMenus');
    if (file && !selected.has(file.path)) selectOnly(file.path);
    ctxMenu = { x: e.clientX, y: e.clientY, target: file };
  }
  function closeMenu() { ctxMenu = null; }

  // ── File operations ─────────────────────────────────────────────────────
  async function createFolder() {
    closeMenu();
    const name = await dialogPrompt({ title: 'Nowy folder', placeholder: 'Nowy folder', defaultValue: 'Nowy folder', confirmLabel: 'Utwórz' });
    if (!name?.trim()) return;
    try { const made = await createWithDialog(desktopPath, name.trim(), 'folder'); if (made) { notify('success', `Utworzono: ${made.split('/').pop()}`); loadFiles(); } }
    catch { notify('error', 'Nie udało się utworzyć folderu'); }
  }

  async function createTextFile() {
    closeMenu();
    const name = await dialogPrompt({ title: 'Nowy plik tekstowy', placeholder: 'nowy_plik.txt', defaultValue: 'nowy_plik.txt', confirmLabel: 'Utwórz' });
    if (!name?.trim()) return;
    try { const made = await createWithDialog(desktopPath, name.trim(), 'file', ''); if (made) { notify('success', `Utworzono: ${made.split('/').pop()}`); loadFiles(); } }
    catch { notify('error', 'Nie udało się utworzyć pliku'); }
  }

  function copySelected() { if (!selected.size) return; clipboard = { action: 'copy', files: [...selected] }; closeMenu(); notify('info', `Skopiowano ${selected.size} element(y)`); }
  function cutSelected() { if (!selected.size) return; clipboard = { action: 'cut', files: [...selected] }; closeMenu(); notify('info', `Wycięto ${selected.size} element(y)`); }

  async function paste() {
    closeMenu();
    if (!clipboard) return;
    const mode = clipboard.action === 'copy' ? 'copy' : 'move';
    let r: TransferSummary | null;
    try { r = await transferWithDialog([...clipboard.files], desktopPath, mode); }
    catch (err) { r = { done: 0, skipped: 0, errors: [String(err)] }; }
    if (!r) return;                         // anulowano — schowek zostaje nietknięty
    { const d = describeSummary(r, mode); notify(d.type === 'info' ? 'info' : d.type, d.message); }
    if (clipboard.action === 'cut') clipboard = null;
    loadFiles();
  }

  async function deleteSelected() {
    closeMenu();
    if (selected.size === 0) return;
    const ok = await dialogConfirm({ title: 'Usuń elementy', message: `Usunąć ${selected.size} element(y)? Tej operacji nie można cofnąć.`, confirmLabel: 'Usuń', danger: true });
    if (!ok) return;
    let errors = 0;
    for (const path of selected) { try { await SystemBridge.deleteFile(path); } catch { errors++; } }
    notify(errors ? 'error' : 'success', errors ? `${errors} element(y) nie powiodło się` : `Usunięto ${selected.size} element(y)`);
    selected = new Set();
    loadFiles();
  }

  async function startRename(file: FileEntry) {
    closeMenu();
    renaming = file.path; renameVal = file.name;
    await tick();
    renameEl?.focus();
    renameEl?.select();
  }
  async function commitRename() {
    if (!renaming || !renameVal.trim()) { renaming = null; return; }
    const file = files.find((f) => f.path === renaming);
    if (!file || renameVal === file.name) { renaming = null; return; }
    const newPath = renaming.slice(0, renaming.lastIndexOf('/') + 1) + renameVal.trim();
    try {
      const done = await renameWithDialog(renaming, renameVal.trim(), file.is_dir ?? false);
      if (done) {
        const finalName = done.split('/').pop() ?? renameVal.trim();
        saved = renameKey(layout, file.name, finalName); persistLayout();
        notify('success', `Zmieniono nazwę na: ${finalName}`); loadFiles();
      }
    }
    catch { notify('error', 'Nie udało się zmienić nazwy'); }
    finally { renaming = null; }
  }

  function openWallpaperSettings() {
    closeMenu();
    openApp(AppId.SETTINGS);
  }

  function handleKeydown(e: KeyboardEvent) {
    // A DialogHost modal (e.g. "New Text File"/"New Folder" naming
    // prompt) is open and its input has focus — this desktop-wide
    // listener must not steal Delete/Ctrl+A/F2 from it (that was the
    // actual cause of "can't type a filename / can't delete text" in
    // that dialog: pressing Delete here deleted the *selected desktop
    // icons* instead of editing the dialog's text field).
    if (get(activeDialog)) return;
    if (e.key === 'Escape') { ctxMenu = null; if (renaming) renaming = null; }
    if (e.key === 'Delete' && selected.size && !renaming) deleteSelected();
    if (e.key === 'F2' && selected.size === 1 && !renaming) {
      const f = files.find((x) => selected.has(x.path));
      if (f) startRename(f);
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'a' && !renaming) { e.preventDefault(); selected = new Set(files.map((f) => f.path)); }
  }
</script>

<svelte:window on:keydown={handleKeydown} on:click={() => (ctxMenu = null)} />

<div
  bind:this={containerEl}
  class="absolute inset-0 select-none z-[1]"
  on:mousedown={onContainerMouseDown}
  on:click={onContainerClick} role="button" tabindex="0" on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onContainerClick(); } }}
  on:contextmenu={(e) => openMenu(e, null)}
  on:dragover={onDesktopDragOver}
  on:drop={onDesktopDrop}
>
  <div class="absolute inset-0">
    {#each files as file (file.path)}
      {@const cell = layout[file.name]}
      {@const pos = cell ? cellOrigin(cell) : { left: 8, top: 8 }}
      {@const dragging = drag?.active && drag.names.includes(file.name)}
      <div
        data-desktop-icon
        data-path={file.path}
        data-is-dir={file.is_dir ? 'true' : 'false'}
        bind:this={iconEls[file.path]}
        class="absolute flex flex-col items-center gap-1 p-2 rounded-lg cursor-default text-center {selected.has(file.path) ? 'bg-blue-500/30 ring-1 ring-blue-400/50' : 'hover:bg-white/10'} {dropFolder === file.name ? 'ring-2 ring-emerald-400 bg-emerald-500/20' : ''} {dragging ? 'opacity-70 pointer-events-none z-10' : ''}"
        style="width:{CELL_W}px; min-height:{CELL_H - 8}px; left:{pos.left}px; top:{pos.top}px; {dragging ? `transform: translate(${drag?.dx}px, ${drag?.dy}px); transition:none;` : ''}"
        on:mousedown={(e) => onIconMouseDown(e, file)}
        on:click={(e) => iconClick(e, file)}
        on:dblclick={() => handleOpen(file)}
        on:contextmenu={(e) => openMenu(e, file)}
        role="button"
        tabindex="-1"
      >
        <FileIcon {file} size={36} />
        {#if renaming === file.path}
          <input
            bind:this={renameEl}
            bind:value={renameVal}
            on:click|stopPropagation
            on:blur={commitRename}
            on:keydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); commitRename(); } if (e.key === 'Escape') { e.stopPropagation(); renaming = null; } }}
            class="w-full bg-slate-900 text-white text-[11px] text-center rounded px-1 py-0.5 focus:outline-none ring-1 ring-blue-500"
          />
        {:else}
          <span class="text-[11px] text-white leading-tight break-words line-clamp-2 drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)]">{file.name}</span>
        {/if}
      </div>
    {/each}
  </div>

  {#if drag?.active && ghostCell}
    {@const g = cellOrigin(ghostCell)}
    <div class="absolute rounded-lg border border-dashed border-blue-300/60 bg-blue-300/10 pointer-events-none" style="left:{g.left}px; top:{g.top}px; width:{CELL_W}px; height:{CELL_H - 8}px;" />
  {/if}

  {#if marquee}
    <div
      class="absolute border border-blue-400 bg-blue-400/15 pointer-events-none"
      style="left:{Math.min(marquee.x0, marquee.x1)}px; top:{Math.min(marquee.y0, marquee.y1)}px; width:{Math.abs(marquee.x1 - marquee.x0)}px; height:{Math.abs(marquee.y1 - marquee.y0)}px;"
    />
  {/if}
</div>

{#if ctxMenu}
  <div
    class="fixed z-[500] w-56 bg-slate-900/95 backdrop-blur-md border border-white/10 rounded-xl shadow-2xl py-1.5 text-sm"
    style="left:{ctxMenu.x}px; top:{ctxMenu.y}px;"
    on:click|stopPropagation
    on:contextmenu|preventDefault
  >
    {#if ctxMenu.target}
      {@const target = ctxMenu.target}
      <button on:click={() => handleOpen(target)} class="w-full text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10">Otwórz</button>
      <button on:click={() => startRename(target)} class="w-full text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10">Zmień nazwę</button>
      <button on:click={cutSelected} class="w-full text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10">Wytnij</button>
      <button on:click={copySelected} class="w-full text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10">Kopiuj</button>
      <div class="h-px bg-white/10 my-1" />
      <button on:click={deleteSelected} class="w-full text-left px-3.5 py-1.5 text-red-400 hover:bg-red-500/10">Usuń</button>
    {:else}
      <button on:click={createFolder} class="w-full flex items-center gap-2 text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10"><FolderPlus size={14} /> Nowy folder</button>
      <button on:click={createTextFile} class="w-full flex items-center gap-2 text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10"><FilePlus size={14} /> Nowy plik tekstowy</button>
      {#if clipboard}
        <button on:click={paste} class="w-full flex items-center gap-2 text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10"><ClipboardPaste size={14} /> Wklej</button>
      {/if}
      <div class="h-px bg-white/10 my-1" />
      <button on:click={arrangeIcons} class="w-full flex items-center gap-2 text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10"><LayoutGrid size={14} /> {$t('desktop.arrange')}</button>
      <button on:click={() => { closeMenu(); loadFiles(); }} class="w-full flex items-center gap-2 text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10"><RefreshCw size={14} /> Odśwież</button>
      <button on:click={openWallpaperSettings} class="w-full flex items-center gap-2 text-left px-3.5 py-1.5 text-slate-200 hover:bg-white/10"><ImageIcon size={14} /> Zmień tapetę</button>
    {/if}
  </div>
{/if}

{#if notifs.length}
  <div class="fixed bottom-16 right-4 z-[600] flex flex-col gap-2 items-end">
    {#each notifs as n (n.id)}
      <div class="px-3 py-2 rounded-lg text-xs text-white shadow-xl {n.type === 'error' ? 'bg-red-600' : n.type === 'success' ? 'bg-emerald-600' : 'bg-slate-700'}">
        {n.message}
      </div>
    {/each}
  </div>
{/if}
