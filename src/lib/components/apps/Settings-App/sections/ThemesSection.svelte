<script lang="ts">
  /**
   * Shell Themes settings section — two tabs: "Installed" (the 10
   * built-in themes from builtinThemes.ts) and "Store" (downloadable
   * themes fetched at runtime).
   *
   * ── Store fetch strategy ──────────────────────────────────────────
   * Tries the live GitHub-hosted JSON first
   * (raw.githubusercontent.com/HackerOS-Linux-System/Blue-Environment/
   * main/config/stores/themes-store.json — the raw form of the repo
   * URL given for this feature), falls back to the copy bundled with
   * the app (config/stores/themes-store.json, same schema) if that
   * fetch fails — offline, GitHub unreachable, rate-limited, etc. Both
   * are empty right now (`"themes": []`), so the Store tab currently
   * always shows an empty state either way; the fetch/fallback plumbing
   * is real and working, there's just nothing to show yet — see that
   * JSON file's own description field.
   *
   * No CSP restrictions block this (`tauri.conf.json`'s `security.csp`
   * is `null`), so this is a plain `fetch()` from the frontend — no
   * Rust round-trip needed for something this simple.
   *
   * ── Applying a theme ────────────────────────────────────────────────
   * Selecting a non-placeholder theme stages it (`shellThemeId` saved
   * to config immediately, so it "sticks" even if the person navigates
   * away) and shows a restart prompt — real window-control/layout
   * changes need the shell process restarted to take effect (some are
   * read once at startup, not reactively watched — see
   * builtinThemes.ts's module doc). "Restart Shell Now" calls
   * `system_power('restart_shell')`; "Later" just leaves the pending
   * banner up (checked again on next mount) so it's not a nagging
   * modal that can't be dismissed.
   *
   * ── What isn't implemented yet ───────────────────────────────────────
   * The actual *rendering* of a selected shell theme's colors/layout
   * (wiring `shellThemeId`'s resolved `ShellTheme` into the panel's
   * position, window-control component's style/position, and a CSS
   * custom-property palette every other component reads from) is a
   * separate, substantial follow-up — this section covers selection,
   * persistence, and the restart flow, which is the real foundation
   * that follow-up needs, not the rendering integration itself.
   */
  import { onMount } from 'svelte';
  import * as Icons from 'lucide-svelte';
  import { Check, RefreshCw, Store as StoreIcon, Sparkles as SparklesIcon, Sparkles, Clock, ExternalLink, HardDrive, Info, ArrowUp, ArrowDown, RotateCcw, GripVertical, Edit2, Download, Trash2, X } from 'lucide-svelte';
  import { DEFAULT_WINDOW_CONTROLS_ORDER, type WindowControlId } from '../../../../data/builtinThemes';
  import type { UserConfig, SystemTheme } from '../../../../types';
  import { t } from '../../../../stores/language';
  import { BUILTIN_THEMES, DEFAULT_SHELL_THEME_ID, type ShellTheme, type ShellThemeExtras } from '../../../../data/builtinThemes';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import {
    customShellThemes, ensureCustomThemesLoaded, saveCustomTheme, removeCustomTheme,
    blankCustomTheme, exportCustomThemeJson, importCustomThemeJson,
  } from '../../../../utils/customThemes';
  // Static import, not `fetch('/config/stores/...')` — this project has
  // no `public/` directory (Vite's default static-asset root), so a
  // runtime fetch of a project-root-relative path like that would
  // always 404. A plain ES import of the JSON file is Vite-native
  // (bundled at build time, no serving/path concerns at all) and is
  // what this local fallback actually needs — it's a fixed snapshot
  // shipped with the app, not something that changes at runtime the
  // way the remote GitHub copy does.
  import themesStoreLocal from '../../../../../../config/stores/themes-store.json';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  const STORE_URL_REMOTE = 'https://raw.githubusercontent.com/HackerOS-Linux-System/Blue-Environment/main/config/stores/themes-store.json';

  type StoreTheme = { id: string; name: string; description: string; author: string; version: string; downloadUrl: string; previewImageUrl?: string };

  let tab: 'installed' | 'system' | 'store' = 'installed';
  let storeThemes: StoreTheme[] = [];
  let storeLoading = false;
  let storeError = false;
  let storeSource: 'remote' | 'local' | null = null;

  let systemThemes: SystemTheme[] = [];
  let systemThemesLoaded = false;
  let systemThemesLoading = false;
  $: activeSystemThemeId = config.systemThemeId ?? null;

  $: activeId = config.shellThemeId ?? DEFAULT_SHELL_THEME_ID;
  let restartPending = false;
  let stagedThemeName = '';

  // ── Window control button order ─────────────────────────────────────
  // Was previously not customizable at all (and had a real ordering bug
  // in the default itself — see WindowControls.svelte's doc comment).
  // Kept per-*installed-theme* (`windowControlsOrderByTheme`, keyed by
  // `activeId`) rather than per base style — see that field's doc
  // comment in systemBridge.ts — so two themes that happen to share a
  // style (say, both 'windows') can still be customized independently;
  // switching to a different theme below shows/edits that theme's own
  // separate order (falling back to its style's built-in default if
  // that theme hasn't been customized).
  $: activeTheme = BUILTIN_THEMES.find((th) => th.id === activeId);
  $: activeControlsStyle = activeTheme?.layout.windowControlsStyle ?? 'windows';
  $: styleDefaultOrder = (activeControlsStyle === 'macos'
    ? ['close', 'minimize', 'maximize', 'pip']
    : DEFAULT_WINDOW_CONTROLS_ORDER) as WindowControlId[];
  $: orderOverrides = config.windowControlsOrderByTheme ?? {};
  $: hasOverrideForStyle = !!orderOverrides[activeId];
  $: controlsOrder = (orderOverrides[activeId] as WindowControlId[] | undefined) ?? styleDefaultOrder;

  const CONTROL_LABELS: Record<WindowControlId, string> = {
    close: 'Zamknij (X)', maximize: 'Pełny ekran', minimize: 'Minimalizuj', pip: 'Obraz w obrazie',
  };

  async function moveControl(index: number, dir: -1 | 1) {
    const next = [...controlsOrder];
    const swapWith = index + dir;
    if (swapWith < 0 || swapWith >= next.length) return;
    [next[index], next[swapWith]] = [next[swapWith], next[index]];
    await onSave({ windowControlsOrderByTheme: { ...orderOverrides, [activeId]: next } });
  }
  async function reorderControlTo(fromIndex: number, toIndex: number) {
    if (fromIndex === toIndex) return;
    const next = [...controlsOrder];
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);
    await onSave({ windowControlsOrderByTheme: { ...orderOverrides, [activeId]: next } });
  }
  async function resetControlsOrder() {
    const next = { ...orderOverrides };
    delete next[activeId];
    await onSave({ windowControlsOrderByTheme: next });
  }

  // ── Drag-and-drop reordering ─────────────────────────────────────────
  // The up/down arrows above still work (and stay — not everyone wants
  // to drag, and they're the more predictable option for keyboard/
  // switch-access users), but "real" drag-and-drop is what most people
  // reach for first when a list says it's reorderable.
  let dragFromIndex: number | null = null;
  let dragOverIndex: number | null = null;
  function handleDragStart(index: number) { dragFromIndex = index; }
  function handleDragOver(e: DragEvent, index: number) { e.preventDefault(); dragOverIndex = index; }
  function handleDragLeave() { dragOverIndex = null; }
  async function handleDrop(e: DragEvent, index: number) {
    e.preventDefault();
    dragOverIndex = null;
    if (dragFromIndex === null) return;
    await reorderControlTo(dragFromIndex, index);
    dragFromIndex = null;
  }
  function handleDragEnd() { dragFromIndex = null; dragOverIndex = null; }

  onMount(() => { if (tab === 'store') loadStore(); ensureCustomThemesLoaded(); });

  // ── Custom (user-created) themes ────────────────────────────────────
  // `extras` is optional on `ShellTheme` (absent on every builtin theme —
  // see that field's own doc comment), but always present on anything
  // this editor itself ever assigns to `editingCustom` (both functions
  // below guarantee it with `?? {}`). Narrowing the local type this way,
  // rather than leaving it as plain `ShellTheme | null`, is what lets
  // the editor's markup use `editingCustom.extras.foo` directly instead
  // of an optional-chain/non-null-assertion on every single field.
  type EditableTheme = ShellTheme & { extras: ShellThemeExtras };
  let editingCustom: EditableTheme | null = null;
  // Declared here (typed, script-side) rather than as an inline array
  // literal in the template's `{#each}` below: svelte-check parses
  // template expressions with a more limited expression grammar than a
  // <script> block, which doesn't accept a TS `as` type-assertion
  // inline in markup — this way `key` is already known to be
  // `keyof ShellTheme['colors']` from this array's own type, with no
  // cast needed at the `bind:value={editingCustom.colors[key]}` call
  // site in the template.
  const COLOR_FIELDS: [keyof ShellTheme['colors'], string][] = [
    ['accent', 'Accent'], ['background', 'Background'], ['surface', 'Surface'],
    ['surfaceElevated', 'Elevated'], ['text', 'Text'], ['textMuted', 'Muted text'], ['border', 'Border'],
  ];
  let customError: string | null = null;
  let customSaving = false;
  let importFileInput: HTMLInputElement;
  let importError: string | null = null;

  function startCreateCustom() {
    customError = null;
    const base = blankCustomTheme();
    editingCustom = { ...base, extras: base.extras ?? {} };
  }
  function startEditCustom(theme: ShellTheme) {
    customError = null;
    const cloned: ShellTheme = JSON.parse(JSON.stringify(theme));
    editingCustom = { ...cloned, extras: cloned.extras ?? {} };
  }
  function cancelCustomEdit() { editingCustom = null; customError = null; }

  async function submitCustom() {
    if (!editingCustom) return;
    customSaving = true; customError = null;
    const result = await saveCustomTheme(editingCustom);
    customSaving = false;
    if (result.ok) editingCustom = null; else customError = result.error;
  }

  async function deleteCustom(id: string) {
    if (id === activeId) return; // guard: can't delete the theme currently applied
    await removeCustomTheme(id);
  }

  function exportCustom(theme: ShellTheme) {
    const blob = new Blob([exportCustomThemeJson(theme)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${theme.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}.blue-theme.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  function triggerImport() { importError = null; importFileInput?.click(); }

  async function handleImportFile(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    const text = await file.text();
    const result = await importCustomThemeJson(text);
    if (result.ok) {
      startEditCustom(result.theme); // land in the editor so the person can rename/tweak before saving
    } else {
      importError = result.error;
    }
    (e.target as HTMLInputElement).value = '';
  }

  async function loadSystemThemes() {
    systemThemesLoading = true;
    try {
      systemThemes = await SystemBridge.listSystemThemes();
    } finally {
      systemThemesLoading = false;
      systemThemesLoaded = true;
    }
  }

  async function selectSystemTheme(theme: SystemTheme) {
    if (theme.id === activeSystemThemeId) {
      // Selecting the already-active one again turns it off — a
      // filesystem theme package is meant to be an optional overlay,
      // not a one-way ratchet with no way back to "none" from the
      // grid itself.
      await onSave({ systemThemeId: null });
      return;
    }
    await onSave({ systemThemeId: theme.id });
    // Filesystem theme CSS applies live via SystemThemeStyle.svelte's
    // reactive `config.systemThemeId` watch (see App.svelte) — no
    // restart needed, unlike a built-in `shellThemeId` change, since
    // this only injects a `<style>` tag + a `data-system-theme`
    // attribute rather than touching panel position/window-control
    // layout the way a shell theme can.
  }

  async function loadStore() {
    storeLoading = true;
    storeError = false;
    storeSource = null;
    try {
      const res = await fetch(STORE_URL_REMOTE, { cache: 'no-store' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      storeThemes = data.themes ?? [];
      storeSource = 'remote';
    } catch {
      // Bundled fallback (static import, see this file's own import —
      // was a broken `fetch('/config/...')` before, 404ing always
      // since this project has no `public/` dir for Vite to have
      // served it from).
      storeThemes = (themesStoreLocal as any).themes ?? [];
      storeSource = 'local';
    } finally {
      storeLoading = false;
    }
  }

  function selectTab(next: 'installed' | 'system' | 'store') {
    tab = next;
    if (next === 'store' && storeSource === null && !storeLoading) loadStore();
    if (next === 'system' && !systemThemesLoaded && !systemThemesLoading) loadSystemThemes();
  }

  async function selectTheme(theme: ShellTheme) {
    if (theme.placeholder) return;
    if (theme.id === activeId) return;
    await onSave({ shellThemeId: theme.id });
    stagedThemeName = theme.name;
    restartPending = true;
  }

  async function restartNow() {
    await SystemBridge.invokeCommand('system_power', { action: 'restart_shell' });
  }

  function dismissRestartPrompt() {
    restartPending = false;
  }

  function iconFor(name: string) {
    return (Icons as any)[name] ?? Icons.Palette;
  }
</script>

<div class="max-w-3xl">
  <h2 class="text-lg font-semibold text-white mb-1">{$t('settings.tab.themes') ?? 'Themes'}</h2>
  <p class="text-xs text-slate-400 mb-4">Full shell themes — colors, panel position, window-control style. Changing one requires a shell restart.</p>

  {#if restartPending}
    <div class="mb-4 flex items-center justify-between gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2.5">
      <div class="flex items-center gap-2 text-xs text-amber-200">
        <Clock size={14} class="shrink-0" />
        <span><strong>{stagedThemeName}</strong> selected — restart the shell to apply it.</span>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <button class="text-[11px] px-2.5 py-1 rounded-md bg-amber-500 text-black font-medium hover:bg-amber-400 transition-colors" on:click={restartNow}>Restart Shell Now</button>
        <button class="text-[11px] px-2 py-1 rounded-md text-amber-200/70 hover:text-amber-100 transition-colors" on:click={dismissRestartPrompt}>Later</button>
      </div>
    </div>
  {/if}

  <div class="flex gap-1 mb-4 border-b border-white/5">
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors {tab === 'installed' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" on:click={() => selectTab('installed')}>Installed</button>
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors flex items-center gap-1.5 {tab === 'system' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" on:click={() => selectTab('system')}>
      <HardDrive size={12} /> System
    </button>
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors flex items-center gap-1.5 {tab === 'store' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" on:click={() => selectTab('store')}>
      <StoreIcon size={12} /> Store
    </button>
  </div>

  {#if tab === 'system'}
    <p class="text-[11px] text-slate-500 mb-3 flex items-start gap-1.5">
      <Info size={12} class="shrink-0 mt-0.5" />
      Themes installed to <code class="text-slate-400">/usr/share/themes/</code> — a separate system from the app-bundled themes above. Applies instantly, no restart needed.
    </p>
    {#if systemThemesLoading}
      <div class="flex items-center gap-2 text-xs text-slate-400 py-8 justify-center">
        <RefreshCw size={14} class="animate-spin" /> Scanning /usr/share/themes…
      </div>
    {:else if systemThemes.length === 0}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <HardDrive size={28} class="text-slate-600" />
        <p class="text-sm text-slate-300">No filesystem themes installed</p>
        <p class="text-xs text-slate-500 max-w-sm">Install a theme package to <code class="text-slate-400">/usr/share/themes/&lt;name&gt;/</code> (config.hk + styles.css) and it will show up here.</p>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-3">
        {#each systemThemes as theme (theme.id)}
          <button
            class="relative text-left rounded-xl border p-3 transition-colors cursor-pointer
              {theme.id === activeSystemThemeId ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 bg-slate-800/40 hover:border-white/20'}"
            on:click={() => selectSystemTheme(theme)}
          >
            <div class="flex items-start gap-3">
              <div class="w-11 h-11 rounded-lg flex items-center justify-center shrink-0 overflow-hidden bg-slate-900 border border-white/10">
                {#if theme.previewDataUrl}
                  <img src={theme.previewDataUrl} alt="" class="w-full h-full object-cover" />
                {:else}
                  <HardDrive size={18} class="text-slate-500" />
                {/if}
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-1.5">
                  <span class="text-sm font-medium text-white truncate">{theme.name}</span>
                  {#if theme.id === activeSystemThemeId}<Check size={13} class="text-blue-400 shrink-0" />{/if}
                </div>
                <p class="text-[11px] text-slate-400 mt-0.5 line-clamp-2">{theme.description}</p>
                <p class="text-[10px] text-slate-500 mt-1">by {theme.author} · v{theme.version}</p>
                <div class="flex items-center gap-1 mt-1.5 flex-wrap">
                  {#if theme.effects.accentColor}<span class="w-3 h-3 rounded-full border border-white/10" style="background: {theme.effects.accentColor}" />{/if}
                  <span class="text-[10px] text-slate-500">{theme.effects.cornerStyle} corners</span>
                  {#if theme.effects.blur}<span class="text-[10px] text-slate-500">· blur</span>{/if}
                  {#if theme.effects.animations}<span class="text-[10px] text-slate-500">· animated</span>{/if}
                </div>
              </div>
            </div>
          </button>
        {/each}
      </div>
    {/if}
  {:else if tab === 'installed'}
    <!-- ── Your Themes (user-created, via custom_shell_themes.rs) ──────
         Richer than any builtin theme: a second accent color for a
         gradient, precise corner radius, wallpaper blur, panel
         opacity, a chosen font and animation speed — see
         customThemes.ts / ShellThemeStyle.svelte for how these apply. -->
    <div class="mb-5">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-semibold text-white">Your Themes</h3>
        <div class="flex gap-2">
          <button on:click={triggerImport} class="text-xs px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300">Import…</button>
          <button on:click={startCreateCustom} class="text-xs px-2.5 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white">+ Create theme</button>
        </div>
      </div>
      <input bind:this={importFileInput} type="file" accept="application/json,.json" class="hidden" on:change={handleImportFile} />
      {#if importError}<p class="text-xs text-red-400 mb-2">{importError}</p>{/if}

      {#if $customShellThemes.length === 0 && !editingCustom}
        <p class="text-xs text-slate-500 italic">No custom themes yet — create one, or import a `.blue-theme.json` someone shared with you.</p>
      {:else}
        <div class="grid grid-cols-2 gap-3">
          {#each $customShellThemes as theme (theme.id)}
            <div class="relative rounded-xl border p-3 group {theme.id === activeId ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 bg-slate-800/40 hover:border-white/20'}">
              <button class="w-full text-left" on:click={() => selectTheme(theme)}>
                <div class="flex items-start gap-3">
                  <div class="w-11 h-11 rounded-lg flex items-center justify-center shrink-0" style="background: {theme.extras?.accentSecondary ? `linear-gradient(135deg, ${theme.colors.accent}, ${theme.extras.accentSecondary})` : theme.colors.surfaceElevated}; border: 1px solid {theme.colors.border};">
                    <Sparkles size={18} style="color: {theme.extras?.accentSecondary ? '#fff' : theme.colors.accent}" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-1.5">
                      <span class="text-sm font-medium text-white truncate">{theme.name}</span>
                      {#if theme.id === activeId}<Check size={13} class="text-blue-400 shrink-0" />{/if}
                    </div>
                    <p class="text-[11px] text-slate-400 mt-0.5 line-clamp-1">{theme.description || 'Custom theme'}</p>
                    <div class="flex items-center gap-1 mt-2">
                      {#each [theme.colors.accent, theme.colors.background, theme.colors.surface, theme.colors.text] as c}
                        <span class="w-3 h-3 rounded-full border border-white/10" style="background: {c}" />
                      {/each}
                    </div>
                  </div>
                </div>
              </button>
              <div class="absolute top-2 right-2 hidden group-hover:flex gap-1">
                <button on:click={() => startEditCustom(theme)} class="p-1 rounded bg-slate-900/80 hover:bg-slate-700 text-slate-300" title="Edit"><Edit2 size={12} /></button>
                <button on:click={() => exportCustom(theme)} class="p-1 rounded bg-slate-900/80 hover:bg-slate-700 text-slate-300" title="Export"><Download size={12} /></button>
                <button on:click={() => deleteCustom(theme.id)} disabled={theme.id === activeId}
                  class="p-1 rounded bg-slate-900/80 hover:bg-red-500/30 text-slate-300 hover:text-red-400 disabled:opacity-30" title="Delete"><Trash2 size={12} /></button>
              </div>
            </div>
          {/each}
        </div>
      {/if}

      {#if editingCustom}
        <div class="mt-3 bg-slate-800 rounded-2xl border border-white/5 p-4 space-y-3">
          <div class="flex items-center justify-between">
            <h4 class="text-sm font-semibold text-white">{$customShellThemes.some(t => t.id === editingCustom?.id) ? 'Edit theme' : 'New theme'}</h4>
            <button on:click={cancelCustomEdit} class="text-slate-500 hover:text-white"><X size={16} /></button>
          </div>

          <input bind:value={editingCustom.name} placeholder="Theme name"
            class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white" />
          <input bind:value={editingCustom.description} placeholder="Short description (optional)"
            class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white" />

          <div class="grid grid-cols-4 gap-2">
            {#each COLOR_FIELDS as [key, label]}
              <label class="flex flex-col items-center gap-1 text-[10px] text-slate-500">
                {label}
                <input type="color" bind:value={editingCustom.colors[key]} class="w-9 h-9 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
              </label>
            {/each}
          </div>

          <div class="flex items-center gap-2">
            <label class="flex items-center gap-1.5 text-xs text-slate-400">
              <input type="checkbox" checked={!!editingCustom.extras?.accentSecondary}
                on:change={(e) => editingCustom && (editingCustom.extras = { ...editingCustom.extras, accentSecondary: e.currentTarget.checked ? (editingCustom.extras?.accentSecondary || '#a855f7') : undefined })}
                class="accent-blue-500" />
              Gradient accent (2nd color)
            </label>
            {#if editingCustom.extras?.accentSecondary}
              <input type="color" bind:value={editingCustom.extras.accentSecondary} class="w-8 h-8 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
            {/if}
          </div>

          <div class="grid grid-cols-2 gap-3">
            <label class="text-xs text-slate-500">Panel position
              <select bind:value={editingCustom.layout.panelPosition} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="top">Top</option><option value="bottom">Bottom</option><option value="left">Left</option><option value="right">Right</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">Window controls style
              <select bind:value={editingCustom.layout.windowControlsStyle} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="macos">macOS</option><option value="windows">Windows</option><option value="gnome">GNOME</option><option value="minimal">Minimal</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">Icon style
              <select bind:value={editingCustom.layout.iconStyle} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="outline">Outline</option><option value="filled">Filled</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">Animation speed
              <select bind:value={editingCustom.extras.animationSpeed} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="normal">Normal</option><option value="fast">Fast</option><option value="none">None</option>
              </select>
            </label>
          </div>

          <label class="block text-xs text-slate-500">Corner radius: {editingCustom.extras.cornerRadiusPx ?? 12}px
            <input type="range" min="0" max="32" bind:value={editingCustom.extras.cornerRadiusPx} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">Wallpaper blur: {editingCustom.extras.wallpaperBlur ?? 0}px
            <input type="range" min="0" max="40" bind:value={editingCustom.extras.wallpaperBlur} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">Panel opacity: {editingCustom.extras.panelOpacity ?? 100}%
            <input type="range" min="10" max="100" bind:value={editingCustom.extras.panelOpacity} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">Font family (CSS value, optional)
            <input bind:value={editingCustom.extras.fontFamily} placeholder="'Fira Sans', sans-serif"
              class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white font-mono" />
          </label>

          {#if customError}<p class="text-xs text-red-400 bg-red-500/10 rounded-lg px-3 py-2">{customError}</p>{/if}

          <div class="flex gap-2 pt-1">
            <button on:click={submitCustom} disabled={customSaving || !editingCustom.name.trim()}
              class="px-4 py-2 bg-blue-600 hover:bg-blue-500 disabled:bg-slate-700 text-white rounded-lg text-sm font-medium">Save theme</button>
            <button on:click={cancelCustomEdit} class="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-lg text-sm">Cancel</button>
          </div>
        </div>
      {/if}
    </div>

    <h3 class="text-sm font-semibold text-white mb-2">Built-in</h3>
    <div class="grid grid-cols-2 gap-3">
      {#each BUILTIN_THEMES as theme (theme.id)}
        <button
          class="relative text-left rounded-xl border p-3 transition-colors group
            {theme.id === activeId ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 bg-slate-800/40 hover:border-white/20'}
            {theme.placeholder ? 'opacity-60 cursor-default' : 'cursor-pointer'}"
          on:click={() => selectTheme(theme)}
          disabled={theme.placeholder}
        >
          <div class="flex items-start gap-3">
            <div class="w-11 h-11 rounded-lg flex items-center justify-center shrink-0 overflow-hidden" style="background: {theme.colors.surfaceElevated}; border: 1px solid {theme.colors.border};">
              {#if theme.previewImage}
                <img src={theme.previewImage} alt="" class="w-full h-full object-cover" />
              {:else}
                <svelte:component this={iconFor(theme.previewIcon)} size={20} style="color: {theme.colors.accent}" />
              {/if}
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="text-sm font-medium text-white truncate">{theme.name}</span>
                {#if theme.id === activeId}<Check size={13} class="text-blue-400 shrink-0" />{/if}
                {#if theme.comingSoon}<span class="text-[9px] uppercase tracking-wide px-1.5 py-0.5 rounded bg-white/10 text-slate-300 shrink-0">Coming soon</span>{/if}
              </div>
              <p class="text-[11px] text-slate-400 mt-0.5 line-clamp-2">{theme.description}</p>
              <div class="flex items-center gap-1 mt-2">
                {#each [theme.colors.accent, theme.colors.background, theme.colors.surface, theme.colors.text] as c}
                  <span class="w-3 h-3 rounded-full border border-white/10" style="background: {c}" />
                {/each}
                <span class="text-[10px] text-slate-500 ml-1.5">{theme.layout.panelPosition} panel · {theme.layout.windowControlsStyle}</span>
              </div>
            </div>
          </div>
        </button>
      {/each}
    </div>

    <!-- Window control button order — see WindowControls.svelte's doc
         comment and this section's script for why this exists (a real
         ordering bug in the default itself, plus "should also be
         customizable, per style" as a follow-up request). Shown under
         the installed-themes grid since it edits whichever style the
         currently active theme uses; switching to a theme with a
         different style below shows/edits that style's own separate
         order (see `windowControlsOrderByStyle`'s doc comment). -->
    <div class="mt-5 rounded-xl border border-white/10 bg-slate-800/40 p-4">
      <div class="flex items-center justify-between mb-1">
        <h4 class="text-sm font-medium text-white">Kolejność przycisków okna</h4>
        {#if hasOverrideForStyle}
          <button on:click={resetControlsOrder} class="flex items-center gap-1 text-[11px] text-slate-400 hover:text-white transition-colors">
            <RotateCcw size={11} /> Domyślna
          </button>
        {/if}
      </div>
      <p class="text-[11px] text-slate-500 mb-3">Kolejność liczona od krawędzi okna do wewnątrz. Ustawienie dotyczy motywu: <span class="text-slate-400">{activeTheme?.name ?? activeId}</span> (styl {activeControlsStyle}) — każdy zainstalowany motyw ma własną, osobną kolejność, nawet jeśli używa tego samego stylu co inny. Przeciągnij, aby zmienić kolejność.</p>
      <div class="space-y-1">
        {#each controlsOrder as id, i (id)}
          <div
            draggable="true"
            on:dragstart={() => handleDragStart(i)}
            on:dragover={(e) => handleDragOver(e, i)}
            on:dragleave={handleDragLeave}
            on:drop={(e) => handleDrop(e, i)}
            on:dragend={handleDragEnd}
            class="flex items-center gap-2 bg-slate-900/60 border rounded-lg px-3 py-1.5 cursor-grab active:cursor-grabbing transition-colors {dragOverIndex === i && dragFromIndex !== i ? 'border-blue-500/60 bg-blue-500/10' : 'border-white/5'} {dragFromIndex === i ? 'opacity-40' : ''}"
          >
            <GripVertical size={12} class="text-slate-600 shrink-0" />
            <span class="text-[10px] text-slate-500 w-4">{i + 1}</span>
            <span class="text-xs text-slate-200 flex-1">{CONTROL_LABELS[id]}</span>
            <button on:click={() => moveControl(i, -1)} disabled={i === 0} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white disabled:opacity-25 disabled:hover:bg-transparent transition-colors"><ArrowUp size={12} /></button>
            <button on:click={() => moveControl(i, 1)} disabled={i === controlsOrder.length - 1} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white disabled:opacity-25 disabled:hover:bg-transparent transition-colors"><ArrowDown size={12} /></button>
          </div>
        {/each}
      </div>
    </div>
  {:else}
    <!-- Store tab -->
    {#if storeLoading}
      <div class="flex items-center gap-2 text-xs text-slate-400 py-8 justify-center">
        <RefreshCw size={14} class="animate-spin" /> Loading theme store…
      </div>
    {:else if storeThemes.length === 0}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <SparklesIcon size={28} class="text-slate-600" />
        <p class="text-sm text-slate-300">No downloadable themes yet</p>
        <p class="text-xs text-slate-500 max-w-sm">
          The theme store is live but empty for now — check back later, or
          <a class="text-blue-400 hover:underline inline-flex items-center gap-0.5" href="https://github.com/HackerOS-Linux-System/Blue-Environment/blob/main/config/stores/themes-store.json" target="_blank" rel="noopener">
            see the store source <ExternalLink size={10} />
          </a>.
        </p>
        {#if storeError}<p class="text-[10px] text-amber-400/80 mt-1">Couldn't reach the store — showing the bundled offline copy instead.</p>{/if}
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-3">
        {#each storeThemes as theme (theme.id)}
          <div class="rounded-xl border border-white/10 bg-slate-800/40 p-3">
            <span class="text-sm font-medium text-white">{theme.name}</span>
            <p class="text-[11px] text-slate-400 mt-0.5">{theme.description}</p>
            <button class="mt-2 text-[11px] px-2.5 py-1 rounded-md bg-blue-500 text-white font-medium hover:bg-blue-400 transition-colors">Download</button>
          </div>
        {/each}
      </div>
    {/if}
    {#if storeSource === 'local'}
      <p class="text-[10px] text-slate-500 mt-3">Showing the offline bundled copy (couldn't reach GitHub).</p>
    {/if}
  {/if}
</div>
