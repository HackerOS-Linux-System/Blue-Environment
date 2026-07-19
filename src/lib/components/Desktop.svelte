<script lang="ts">
  import { createEventDispatcher, onMount, tick } from 'svelte';
  import { FolderPlus, FilePlus, ClipboardPaste, RefreshCw, Image as ImageIcon } from 'lucide-svelte';
  import { SystemBridge, toAssetUrl } from '../utils/systemBridge';
  import { dialogPrompt, dialogConfirm } from '../stores/dialog';
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
    if (e.ctrlKey || e.metaKey || e.shiftKey) toggleSelect(file.path);
    else selectOnly(file.path);
  }

  function handleOpen(file: FileEntry) {
    if (file.is_dir) { openApp(AppId.EXPLORER); return; }
    SystemBridge.launchApp(`xdg-open "${file.path}"`);
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
    try { await SystemBridge.createFolder(desktopPath, name.trim()); notify('success', `Utworzono: ${name}`); loadFiles(); }
    catch { notify('error', 'Nie udało się utworzyć folderu'); }
  }

  async function createTextFile() {
    closeMenu();
    const name = await dialogPrompt({ title: 'Nowy plik tekstowy', placeholder: 'nowy_plik.txt', defaultValue: 'nowy_plik.txt', confirmLabel: 'Utwórz' });
    if (!name?.trim()) return;
    try { await SystemBridge.createTextFile(desktopPath, name.trim(), ''); notify('success', `Utworzono: ${name}`); loadFiles(); }
    catch { notify('error', 'Nie udało się utworzyć pliku'); }
  }

  function copySelected() { if (!selected.size) return; clipboard = { action: 'copy', files: [...selected] }; closeMenu(); notify('info', `Skopiowano ${selected.size} element(y)`); }
  function cutSelected() { if (!selected.size) return; clipboard = { action: 'cut', files: [...selected] }; closeMenu(); notify('info', `Wycięto ${selected.size} element(y)`); }

  async function paste() {
    closeMenu();
    if (!clipboard) return;
    let errors = 0;
    for (const src of clipboard.files) {
      const name = src.split('/').pop() ?? '';
      const dst = `${desktopPath}/${name}`;
      try { if (clipboard.action === 'copy') await SystemBridge.copyFile(src, dst); else await SystemBridge.moveFile(src, dst); }
      catch { errors++; }
    }
    notify(errors ? 'error' : 'success', errors ? `${errors} element(y) nie powiodło się` : `Wklejono ${clipboard.files.length} element(y)`);
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
    try { await SystemBridge.moveFile(renaming, newPath); notify('success', `Zmieniono nazwę na: ${renameVal}`); loadFiles(); }
    catch { notify('error', 'Nie udało się zmienić nazwy'); }
    finally { renaming = null; }
  }

  function openWallpaperSettings() {
    closeMenu();
    openApp(AppId.SETTINGS);
  }

  function handleKeydown(e: KeyboardEvent) {
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
  style="padding-top:3rem;"
  on:mousedown={onContainerMouseDown}
  on:click={onContainerClick}
  on:contextmenu={(e) => openMenu(e, null)}
>
  <div class="flex flex-col flex-wrap content-start h-full py-2 pl-2 gap-0.5" style="align-content:flex-start;">
    {#each files as file (file.path)}
      <div
        data-desktop-icon
        bind:this={iconEls[file.path]}
        class="w-24 flex flex-col items-center gap-1 p-2 rounded-lg cursor-default text-center {selected.has(file.path) ? 'bg-blue-500/30 ring-1 ring-blue-400/50' : 'hover:bg-white/10'}"
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
