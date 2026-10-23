<script lang="ts">
  import { run } from 'svelte/legacy';

  /**
   * Shell Themes settings section — two tabs: "Installed" (the 10
   * built-in themes from builtinThemes.ts) and "Store" (community
   * `.blue` theme packages, installed via Blue Store).
   *
   * ── Store fetch strategy ──────────────────────────────────────────
   * BUGFIX: this used to `fetch(STORE_URL_REMOTE)` straight from the
   * webview and fall back to a bundled JSON snapshot when that failed —
   * which it always did, silently, because the shell's CSP
   * (`connect-src`) does not allow arbitrary https hosts. So the
   * "Store" tab only ever showed the bundled copy, and its one
   * "Download" button had no handler at all — nothing could actually be
   * installed. Both the index fetch and the install now go through
   * `blueStore.ts` → Rust (`BlueStore::net`/`BlueStore::install`),
   * which isn't subject to the webview's CSP and can actually write
   * into `/usr/share/themes/` (elevating via pkexec/sudo when needed —
   * see `src-tauri/src/privileged.rs`).
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
  import ThemePreview from './ThemePreview.svelte';
  import { onDestroy, onMount } from 'svelte';
  import * as Icons from 'lucide-svelte';
  import { Check, RefreshCw, Store as StoreIcon, Sparkles as SparklesIcon, Sparkles, Clock, ExternalLink, HardDrive, Info, ArrowUp, ArrowDown, RotateCcw, GripVertical, Edit2, Download, Trash2, X, AlertTriangle } from 'lucide-svelte';
  import { DEFAULT_WINDOW_CONTROLS_ORDER, type WindowControlId } from '../../../../data/builtinThemes';
  import type { UserConfig, SystemTheme } from '../../../../types';
  import { t } from '../../../../stores/language';
  import { BUILTIN_THEMES, DEFAULT_SHELL_THEME_ID, type ShellTheme, type ShellThemeExtras } from '../../../../data/builtinThemes';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import {
    customShellThemes, ensureCustomThemesLoaded, saveCustomTheme, removeCustomTheme,
    blankCustomTheme, exportCustomThemeBundle, importCustomThemeJson,
  } from '../../../../utils/customThemes';
  import { previewShellTheme } from '../../../../stores/shellTheme';
  import { themeContrastIssues } from '../../../../utils/contrast';
  import {
    fetchStoreIndex, installPackage, uninstallPackage, listInstalled, storeErrorMessage,
    type StoreIndexEntry, type Receipt,
  } from '../../../../utils/blueStore';

  interface Props {
    config: UserConfig;
    onSave: (p: Partial<UserConfig>) => Promise<void>;
  }

  let { config, onSave }: Props = $props();

  let tab = $state<'installed' | 'system' | 'store'>('installed');
  let storeThemes = $state<StoreIndexEntry[]>([]);
  let storeWarnings = $state<string[]>([]);
  let storeLoading = $state(false);
  let storeErrorMsg = $state<string | null>(null);
  let storeLoaded = false;
  let installedStoreThemes: Receipt[] = [];
  /** downloadUrl -> progress (0-100) while installing, absent otherwise. */
  let installing = $state<Record<string, number>>({});

  let systemThemes = $state<SystemTheme[]>([]);
  let systemThemesLoaded = false;
  let systemThemesLoading = $state(false);
  let activeSystemThemeId = $derived(config.systemThemeId ?? null);

  let activeId = $derived(config.shellThemeId ?? DEFAULT_SHELL_THEME_ID);
  let restartPending = $state(false);
  let stagedThemeName = $state('');

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
  let activeTheme = $derived(BUILTIN_THEMES.find((th) => th.id === activeId));
  let activeControlsStyle = $derived(activeTheme?.layout.windowControlsStyle ?? 'windows');
  let styleDefaultOrder = $derived((activeControlsStyle === 'macos'
    ? ['close', 'minimize', 'maximize', 'pip']
    : DEFAULT_WINDOW_CONTROLS_ORDER) as WindowControlId[]);
  let orderOverrides = $derived(config.windowControlsOrderByTheme ?? {});
  let hasOverrideForStyle = $derived(!!orderOverrides[activeId]);
  let controlsOrder = $derived((orderOverrides[activeId] as WindowControlId[] | undefined) ?? styleDefaultOrder);

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
  let dragFromIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);
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
  let editingCustom = $state<EditableTheme | null>(null);
  // Declared here (typed, script-side) rather than as an inline array
  // literal in the template's `{#each}` below: svelte-check parses
  // template expressions with a more limited expression grammar than a
  // <script> block, which doesn't accept a TS `as` type-assertion
  // inline in markup — this way `key` is already known to be
  // `keyof ShellTheme['colors']` from this array's own type, with no
  // cast needed at the `bind:value={editingCustom.colors[key]}` call
  // site in the template.
  const COLOR_FIELDS: [keyof ShellTheme['colors'], string][] = [
    ['accent', 'thm.c_accent'], ['background', 'thm.c_background'], ['surface', 'thm.c_surface'],
    ['surfaceElevated', 'thm.c_elevated'], ['text', 'thm.c_text'], ['textMuted', 'thm.c_muted'], ['border', 'thm.c_border'],
  ];

  // ── "Try live": apply the unsaved draft to the running shell ──────────
  // Colours and extras apply instantly (see stores/shellTheme.ts); layout
  // choices that need a restart don't. Ending the edit always ends the preview.
  let previewing = $state(false);
  run(() => {
    previewShellTheme.set(previewing && editingCustom ? editingCustom : null);
  });
  run(() => {
    if (!editingCustom) previewing = false;
  });
  onDestroy(() => previewShellTheme.set(null));

  // ── Contrast (WCAG) ───────────────────────────────────────────────────
  let contrastIssues = $derived(editingCustom ? themeContrastIssues(editingCustom.colors) : []);

  // ── Wallpaper that travels with the theme ─────────────────────────────
  let wallpaperName = $derived(editingCustom?.wallpaper ? (editingCustom.wallpaper.split('/').filter(Boolean).pop() ?? '') : '');
  function useCurrentWallpaper() {
    if (!editingCustom || !config.wallpaper) return;
    editingCustom.wallpaper = config.wallpaper.replace(/^file:\/\//, '');
    editingCustom = editingCustom;
  }
  function clearWallpaper() {
    if (!editingCustom) return;
    editingCustom.wallpaper = undefined;
    editingCustom = editingCustom;
  }
  let customError = $state<string | null>(null);
  let customSaving = $state(false);
  let importFileInput = $state() as HTMLInputElement;
  let importError = $state<string | null>(null);

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

  async function exportCustom(theme: ShellTheme) {
    // The wallpaper (if the theme has one) is embedded in the file.
    const blob = new Blob([await exportCustomThemeBundle(theme)], { type: 'application/json' });
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
    storeErrorMsg = null;
    try {
      const [index, installed] = await Promise.all([fetchStoreIndex('theme'), listInstalled('theme')]);
      storeThemes = index.entries;
      storeWarnings = index.warnings;
      installedStoreThemes = installed;
    } catch (e) {
      storeErrorMsg = storeErrorMessage(e);
      storeThemes = [];
    } finally {
      storeLoading = false;
      storeLoaded = true;
    }
  }

  function isThemeInstalled(entry: StoreIndexEntry): Receipt | undefined {
    return installedStoreThemes.find((r) => r.sourceUrl === entry.downloadUrl);
  }

  async function installStoreTheme(entry: StoreIndexEntry) {
    installing = { ...installing, [entry.downloadUrl]: 0 };
    try {
      await installPackage(entry.downloadUrl, 'theme', (pct) => { installing = { ...installing, [entry.downloadUrl]: pct }; });
      installedStoreThemes = await listInstalled('theme');
    } catch (e) {
      storeErrorMsg = `${entry.name}: ${storeErrorMessage(e)}`;
    } finally {
      const { [entry.downloadUrl]: _drop, ...rest } = installing;
      installing = rest;
    }
  }

  async function uninstallStoreTheme(receipt: Receipt) {
    try {
      await uninstallPackage('theme', receipt.id);
      installedStoreThemes = await listInstalled('theme');
    } catch (e) {
      storeErrorMsg = `${receipt.name}: ${storeErrorMessage(e)}`;
    }
  }

  function selectTab(next: 'installed' | 'system' | 'store') {
    tab = next;
    if (next === 'store' && !storeLoaded && !storeLoading) loadStore();
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
  <p class="text-xs text-slate-400 mb-4">{$t('thm.header_desc')}</p>

  {#if restartPending}
    <div class="mb-4 flex items-center justify-between gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2.5">
      <div class="flex items-center gap-2 text-xs text-amber-200">
        <Clock size={14} class="shrink-0" />
        <span>{$t('thm.restart_selected', { name: stagedThemeName })}</span>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <button class="text-[11px] px-2.5 py-1 rounded-md bg-amber-500 text-black font-medium hover:bg-amber-400 transition-colors" onclick={restartNow}>{$t('thm.restart_now')}</button>
        <button class="text-[11px] px-2 py-1 rounded-md text-amber-200/70 hover:text-amber-100 transition-colors" onclick={dismissRestartPrompt}>{$t('thm.later')}</button>
      </div>
    </div>
  {/if}

  <div class="flex gap-1 mb-4 border-b border-white/5">
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors {tab === 'installed' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" onclick={() => selectTab('installed')}>Installed</button>
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors flex items-center gap-1.5 {tab === 'system' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" onclick={() => selectTab('system')}>
      <HardDrive size={12} /> {$t('thm.tab_system')}
    </button>
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors flex items-center gap-1.5 {tab === 'store' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" onclick={() => selectTab('store')}>
      <StoreIcon size={12} /> {$t('thm.tab_store')}
    </button>
  </div>

  {#if tab === 'system'}
    <p class="text-[11px] text-slate-500 mb-3 flex items-start gap-1.5">
      <Info size={12} class="shrink-0 mt-0.5" />
      {$t('thm.system_desc')}
    </p>
    {#if systemThemesLoading}
      <div class="flex items-center gap-2 text-xs text-slate-400 py-8 justify-center">
        <RefreshCw size={14} class="animate-spin" /> {$t('thm.system_scanning')}
      </div>
    {:else if systemThemes.length === 0}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <HardDrive size={28} class="text-slate-600" />
        <p class="text-sm text-slate-300">{$t('thm.system_none')}</p>
        <p class="text-xs text-slate-500 max-w-sm">{$t('thm.system_none_hint')}</p>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-3">
        {#each systemThemes as theme (theme.id)}
          <button
            class="relative text-left rounded-xl border p-3 transition-colors cursor-pointer
              {theme.id === activeSystemThemeId ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 bg-slate-800/40 hover:border-white/20'}"
            onclick={() => selectSystemTheme(theme)}
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
                  {#if theme.effects.accentColor}<span class="w-3 h-3 rounded-full border border-white/10" style="background: {theme.effects.accentColor}"></span>{/if}
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
        <h3 class="text-sm font-semibold text-white">{$t('thm.your_themes')}</h3>
        <div class="flex gap-2">
          <button onclick={triggerImport} class="text-xs px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300">{$t('thm.import')}</button>
          <button onclick={startCreateCustom} class="text-xs px-2.5 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white">{$t('thm.create')}</button>
        </div>
      </div>
      <input bind:this={importFileInput} type="file" accept="application/json,.json" class="hidden" onchange={handleImportFile} />
      {#if importError}<p class="text-xs text-red-400 mb-2">{importError}</p>{/if}

      {#if $customShellThemes.length === 0 && !editingCustom}
        <p class="text-xs text-slate-500 italic">{$t('thm.none_yet')}</p>
      {:else}
        <div class="grid grid-cols-2 gap-3">
          {#each $customShellThemes as theme (theme.id)}
            <div class="relative rounded-xl border p-3 group {theme.id === activeId ? 'border-blue-500 bg-blue-500/10' : 'border-white/10 bg-slate-800/40 hover:border-white/20'}">
              <button class="w-full text-left" onclick={() => selectTheme(theme)}>
                <div class="flex items-start gap-3">
                  <div class="w-11 h-11 rounded-lg flex items-center justify-center shrink-0" style="background: {theme.extras?.accentSecondary ? `linear-gradient(135deg, ${theme.colors.accent}, ${theme.extras.accentSecondary})` : theme.colors.surfaceElevated}; border: 1px solid {theme.colors.border};">
                    <Sparkles size={18} style="color: {theme.extras?.accentSecondary ? '#fff' : theme.colors.accent}" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-1.5">
                      <span class="text-sm font-medium text-white truncate">{theme.name}</span>
                      {#if theme.id === activeId}<Check size={13} class="text-blue-400 shrink-0" />{/if}
                    </div>
                    <p class="text-[11px] text-slate-400 mt-0.5 line-clamp-1">{theme.description || $t('thm.custom_default_desc')}</p>
                    <div class="flex items-center gap-1 mt-2">
                      {#each [theme.colors.accent, theme.colors.background, theme.colors.surface, theme.colors.text] as c}
                        <span class="w-3 h-3 rounded-full border border-white/10" style="background: {c}"></span>
                      {/each}
                    </div>
                  </div>
                </div>
              </button>
              <div class="absolute top-2 right-2 hidden group-hover:flex gap-1">
                <button onclick={() => startEditCustom(theme)} class="p-1 rounded bg-slate-900/80 hover:bg-slate-700 text-slate-300" title={$t('thm.edit')} aria-label={$t('thm.edit')}><Edit2 size={12} /></button>
                <button onclick={() => exportCustom(theme)} class="p-1 rounded bg-slate-900/80 hover:bg-slate-700 text-slate-300" title={$t('thm.export')} aria-label={$t('thm.export')}><Download size={12} /></button>
                <button onclick={() => deleteCustom(theme.id)} disabled={theme.id === activeId}
                  class="p-1 rounded bg-slate-900/80 hover:bg-red-500/30 text-slate-300 hover:text-red-400 disabled:opacity-30" title={$t('thm.delete')} aria-label={$t('thm.delete')}><Trash2 size={12} /></button>
              </div>
            </div>
          {/each}
        </div>
      {/if}

      {#if editingCustom}
        <div class="mt-3 bg-slate-800 rounded-2xl border border-white/5 p-4 space-y-3">
          <div class="flex items-center justify-between">
            <h4 class="text-sm font-semibold text-white">{$customShellThemes.some((th) => th.id === editingCustom?.id) ? $t('thm.editor_edit') : $t('thm.editor_new')}</h4>
            <button onclick={cancelCustomEdit} class="text-slate-500 hover:text-white" aria-label={$t('thm.cancel')}><X size={16} /></button>
          </div>

          <input bind:value={editingCustom.name} placeholder={$t("thm.name_ph")}
            class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white" />
          <input bind:value={editingCustom.description} placeholder={$t("thm.desc_ph")}
            class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white" />

          <ThemePreview theme={editingCustom} />

          <!-- Try the unsaved draft on the real shell -->
          <div class="flex items-center gap-2 flex-wrap">
            <button class="text-xs px-2.5 py-1.5 rounded-lg {previewing ? 'bg-amber-500 text-black font-medium' : 'bg-slate-700 hover:bg-slate-600 text-slate-200'}"
              onclick={() => (previewing = !previewing)}>{previewing ? $t('thm.preview_stop') : $t('thm.preview_try')}</button>
            {#if previewing}<span class="text-[11px] text-amber-300">{$t('thm.preview_active')}</span>{/if}
          </div>

          {#if contrastIssues.length}
            <div class="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 space-y-1" role="alert">
              <div class="text-xs font-medium text-amber-200">{$t('thm.contrast_title')}</div>
              {#each contrastIssues as issue (issue.pair)}
                <div class="text-[11px] text-amber-100/80">{$t(`thm.ct_${issue.pair}`, { ratio: issue.ratio, min: issue.min })}</div>
              {/each}
            </div>
          {/if}

          <div class="grid grid-cols-4 gap-2">
            {#each COLOR_FIELDS as [key, label]}
              <label class="flex flex-col items-center gap-1 text-[10px] text-slate-500">
                {$t(label)}
                <input type="color" bind:value={editingCustom.colors[key]} class="w-9 h-9 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
              </label>
            {/each}
          </div>

          <div class="flex items-center gap-2">
            <label class="flex items-center gap-1.5 text-xs text-slate-400">
              <input type="checkbox" checked={!!editingCustom.extras?.accentSecondary}
                onchange={(e) => editingCustom && (editingCustom.extras = { ...editingCustom.extras, accentSecondary: e.currentTarget.checked ? (editingCustom.extras?.accentSecondary || '#a855f7') : undefined })}
                class="accent-blue-500" />
              {$t('thm.gradient')}
            </label>
            {#if editingCustom.extras?.accentSecondary}
              <input type="color" bind:value={editingCustom.extras.accentSecondary} class="w-8 h-8 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
            {/if}
          </div>

          <div class="grid grid-cols-2 gap-3">
            <label class="text-xs text-slate-500">{$t('thm.panel_position')}
              <select bind:value={editingCustom.layout.panelPosition} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="top">{$t('thm.pos_top')}</option><option value="bottom">{$t('thm.pos_bottom')}</option><option value="left">{$t('thm.pos_left')}</option><option value="right">{$t('thm.pos_right')}</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">{$t('thm.wc_style')}
              <select bind:value={editingCustom.layout.windowControlsStyle} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="macos">macOS</option><option value="windows">Windows</option><option value="gnome">GNOME</option><option value="minimal">{$t('thm.wc_minimal')}</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">{$t('thm.icon_style')}
              <select bind:value={editingCustom.layout.iconStyle} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="outline">{$t('thm.icon_outline')}</option><option value="filled">{$t('thm.icon_filled')}</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">{$t('thm.anim_speed')}
              <select bind:value={editingCustom.extras.animationSpeed} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="normal">{$t('thm.anim_normal')}</option><option value="fast">{$t('thm.anim_fast')}</option><option value="none">{$t('thm.anim_none')}</option>
              </select>
            </label>
          </div>

          <label class="block text-xs text-slate-500">{$t('thm.corner_radius', { n: editingCustom.extras.cornerRadiusPx ?? 12 })}
            <input type="range" min="0" max="32" bind:value={editingCustom.extras.cornerRadiusPx} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">{$t('thm.wp_blur', { n: editingCustom.extras.wallpaperBlur ?? 0 })}
            <input type="range" min="0" max="40" bind:value={editingCustom.extras.wallpaperBlur} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">{$t('thm.panel_opacity', { n: editingCustom.extras.panelOpacity ?? 100 })}
            <input type="range" min="10" max="100" bind:value={editingCustom.extras.panelOpacity} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">{$t('thm.font_family')}
            <input bind:value={editingCustom.extras.fontFamily} placeholder="'Fira Sans', sans-serif"
              class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white font-mono" />
          </label>

          <!-- ── Windows ─────────────────────────────────────────────── -->
          <div class="pt-2 border-t border-white/5 text-[11px] font-semibold uppercase tracking-wider text-slate-400">{$t('thm.sec_windows')}</div>
          <div class="grid grid-cols-2 gap-3">
            <label class="text-xs text-slate-500">{$t('thm.window_shadow')}
              <select bind:value={editingCustom.extras.windowShadow} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="none">{$t('thm.shadow_none')}</option><option value="soft">{$t('thm.shadow_soft')}</option><option value="medium">{$t('thm.shadow_medium')}</option><option value="strong">{$t('thm.shadow_strong')}</option><option value="glow">{$t('thm.shadow_glow')}</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">{$t('thm.scrollbars')}
              <select bind:value={editingCustom.extras.scrollbarStyle} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="default">{$t('thm.sb_default')}</option><option value="thin">{$t('thm.sb_thin')}</option><option value="accent">{$t('thm.sb_accent')}</option><option value="hidden">{$t('thm.sb_hidden')}</option>
              </select>
            </label>
          </div>
          <label class="block text-xs text-slate-500">{$t('thm.window_opacity', { n: editingCustom.extras.windowOpacity ?? 100 })}
            <input type="range" min="60" max="100" bind:value={editingCustom.extras.windowOpacity} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">{$t('thm.titlebar_height', { n: editingCustom.extras.titlebarHeight ?? 36 })}
            <input type="range" min="28" max="56" bind:value={editingCustom.extras.titlebarHeight} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">{$t('thm.window_border', { n: editingCustom.extras.windowBorderWidth ?? 1 })}
            <input type="range" min="0" max="4" bind:value={editingCustom.extras.windowBorderWidth} class="w-full accent-blue-500" />
          </label>
          <div class="flex items-center gap-2">
            <label class="flex items-center gap-1.5 text-xs text-slate-400">
              <input type="checkbox" checked={!!editingCustom.extras.windowBorderColor}
                onchange={(e) => editingCustom && (editingCustom.extras = { ...editingCustom.extras, windowBorderColor: e.currentTarget.checked ? (editingCustom.extras.windowBorderColor || editingCustom.colors.accent) : undefined })}
                class="accent-blue-500" />
              {$t('thm.own_border_color')}
            </label>
            {#if editingCustom.extras.windowBorderColor}
              <input type="color" bind:value={editingCustom.extras.windowBorderColor} class="w-8 h-8 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
            {/if}
          </div>

          <!-- ── Wallpaper ───────────────────────────────────────────── -->
          <div class="pt-2 border-t border-white/5 text-[11px] font-semibold uppercase tracking-wider text-slate-400">{$t('thm.sec_wallpaper')}</div>
          <div class="flex items-center gap-2 flex-wrap">
            <span class="text-xs text-slate-400 truncate max-w-[14rem]">{wallpaperName || $t('thm.wallpaper_none')}</span>
            <button class="text-xs px-2.5 py-1 rounded-lg bg-slate-700 hover:bg-slate-600 text-slate-200 disabled:opacity-40" disabled={!config.wallpaper} onclick={useCurrentWallpaper}>{$t('thm.wallpaper_use_current')}</button>
            {#if editingCustom.wallpaper}<button class="text-xs px-2.5 py-1 rounded-lg bg-slate-700 hover:bg-slate-600 text-slate-200" onclick={clearWallpaper}>{$t('thm.wallpaper_remove')}</button>{/if}
          </div>
          {#if editingCustom.wallpaper}<p class="text-[11px] text-slate-500">{$t('thm.wallpaper_bundled_hint')}</p>{/if}
          <label class="block text-xs text-slate-500">{$t('thm.wp_brightness', { n: editingCustom.extras.wallpaperBrightness ?? 100 })}
            <input type="range" min="30" max="130" bind:value={editingCustom.extras.wallpaperBrightness} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">{$t('thm.wp_saturation', { n: editingCustom.extras.wallpaperSaturation ?? 100 })}
            <input type="range" min="0" max="200" bind:value={editingCustom.extras.wallpaperSaturation} class="w-full accent-blue-500" />
          </label>

          <!-- ── Panel & interface ───────────────────────────────────── -->
          <div class="pt-2 border-t border-white/5 text-[11px] font-semibold uppercase tracking-wider text-slate-400">{$t('thm.sec_panel')}</div>
          <div class="flex items-center gap-2">
            <label class="flex items-center gap-1.5 text-xs text-slate-400">
              <input type="checkbox" checked={!!editingCustom.extras.panelColor}
                onchange={(e) => editingCustom && (editingCustom.extras = { ...editingCustom.extras, panelColor: e.currentTarget.checked ? (editingCustom.extras.panelColor || editingCustom.colors.surface) : undefined })}
                class="accent-blue-500" />
              {$t('thm.own_panel_color')}
            </label>
            {#if editingCustom.extras.panelColor}
              <input type="color" bind:value={editingCustom.extras.panelColor} class="w-8 h-8 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
            {/if}
          </div>
          <label class="block text-xs text-slate-500">{$t('thm.ui_size', { n: editingCustom.extras.uiScalePercent ?? 100 })}
            <input type="range" min="80" max="130" step="5" bind:value={editingCustom.extras.uiScalePercent} class="w-full accent-blue-500" />
          </label>

          <!-- ── Full-screen application menu ─────────────────────────── -->
          <div class="pt-2 border-t border-white/5 text-[11px] font-semibold uppercase tracking-wider text-slate-400">{$t('thm.sec_menu')}</div>
          <div class="flex items-center gap-2">
            <label class="flex items-center gap-1.5 text-xs text-slate-400">
              <input type="checkbox" checked={!!editingCustom.extras.menuBackgroundColor}
                onchange={(e) => editingCustom && (editingCustom.extras = { ...editingCustom.extras, menuBackgroundColor: e.currentTarget.checked ? (editingCustom.extras.menuBackgroundColor || editingCustom.colors.background) : undefined })}
                class="accent-blue-500" />
              {$t('thm.own_menu_color')}
            </label>
            {#if editingCustom.extras.menuBackgroundColor}
              <input type="color" bind:value={editingCustom.extras.menuBackgroundColor} class="w-8 h-8 rounded-lg bg-slate-900 border border-white/10 cursor-pointer" />
            {/if}
          </div>
          <label class="block text-xs text-slate-500">{$t('thm.menu_opacity', { n: editingCustom.extras.menuOpacity ?? 70 })}
            <input type="range" min="20" max="100" bind:value={editingCustom.extras.menuOpacity} class="w-full accent-blue-500" />
          </label>
          <label class="block text-xs text-slate-500">{$t('thm.menu_blur', { n: editingCustom.extras.menuBlur ?? 40 })}
            <input type="range" min="0" max="60" bind:value={editingCustom.extras.menuBlur} class="w-full accent-blue-500" />
          </label>
          <div class="grid grid-cols-2 gap-3">
            <label class="text-xs text-slate-500">{$t('thm.menu_sidebar')}
              <select bind:value={editingCustom.extras.menuSidebar} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="default">{$t('thm.msb_default')}</option><option value="transparent">{$t('thm.msb_transparent')}</option><option value="solid">{$t('thm.msb_solid')}</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">{$t('thm.menu_tile_size')}
              <select bind:value={editingCustom.extras.menuTileSize} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="compact">{$t('thm.mts_compact')}</option><option value="normal">{$t('thm.mts_normal')}</option><option value="large">{$t('thm.mts_large')}</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">{$t('thm.menu_tile_shape')}
              <select bind:value={editingCustom.extras.menuTileShape} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="rounded">{$t('thm.shape_rounded')}</option><option value="circle">{$t('thm.shape_circle')}</option><option value="square">{$t('thm.shape_square')}</option>
              </select>
            </label>
            <label class="text-xs text-slate-500">{$t('thm.menu_search_shape')}
              <select bind:value={editingCustom.extras.menuSearchShape} class="w-full mt-1 bg-slate-900 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white">
                <option value="rounded">{$t('thm.shape_rounded')}</option><option value="pill">{$t('thm.shape_pill')}</option><option value="square">{$t('thm.shape_square')}</option>
              </select>
            </label>
          </div>

          {#if customError}<p class="text-xs text-red-400 bg-red-500/10 rounded-lg px-3 py-2">{customError}</p>{/if}

          <div class="flex gap-2 pt-1">
            <button onclick={submitCustom} disabled={customSaving || !editingCustom.name.trim()}
              class="px-4 py-2 bg-blue-600 hover:bg-blue-500 disabled:bg-slate-700 text-white rounded-lg text-sm font-medium">{$t('thm.save')}</button>
            <button onclick={cancelCustomEdit} class="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-lg text-sm">{$t('thm.cancel')}</button>
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
          onclick={() => selectTheme(theme)}
          disabled={theme.placeholder}
        >
          <div class="flex items-start gap-3">
            <div class="w-11 h-11 rounded-lg flex items-center justify-center shrink-0 overflow-hidden" style="background: {theme.colors.surfaceElevated}; border: 1px solid {theme.colors.border};">
              {#if theme.previewImage}
                <img src={theme.previewImage} alt="" class="w-full h-full object-cover" />
              {:else}
                {@const SvelteComponent = iconFor(theme.previewIcon)}
                <SvelteComponent size={20} style="color: {theme.colors.accent}" />
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
                  <span class="w-3 h-3 rounded-full border border-white/10" style="background: {c}"></span>
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
          <button onclick={resetControlsOrder} class="flex items-center gap-1 text-[11px] text-slate-400 hover:text-white transition-colors">
            <RotateCcw size={11} /> Domyślna
          </button>
        {/if}
      </div>
      <p class="text-[11px] text-slate-500 mb-3">Kolejność liczona od krawędzi okna do wewnątrz. Ustawienie dotyczy motywu: <span class="text-slate-400">{activeTheme?.name ?? activeId}</span> (styl {activeControlsStyle}) — każdy zainstalowany motyw ma własną, osobną kolejność, nawet jeśli używa tego samego stylu co inny. Przeciągnij, aby zmienić kolejność.</p>
      <div class="space-y-1">
        {#each controlsOrder as id, i (id)}
          <div
            role="presentation"
            draggable="true"
            ondragstart={() => handleDragStart(i)}
            ondragover={(e) => handleDragOver(e, i)}
            ondragleave={handleDragLeave}
            ondrop={(e) => handleDrop(e, i)}
            ondragend={handleDragEnd}
            class="flex items-center gap-2 bg-slate-900/60 border rounded-lg px-3 py-1.5 cursor-grab active:cursor-grabbing transition-colors {dragOverIndex === i && dragFromIndex !== i ? 'border-blue-500/60 bg-blue-500/10' : 'border-white/5'} {dragFromIndex === i ? 'opacity-40' : ''}"
          >
            <GripVertical size={12} class="text-slate-600 shrink-0" />
            <span class="text-[10px] text-slate-500 w-4">{i + 1}</span>
            <span class="text-xs text-slate-200 flex-1">{CONTROL_LABELS[id]}</span>
            <button onclick={() => moveControl(i, -1)} disabled={i === 0} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white disabled:opacity-25 disabled:hover:bg-transparent transition-colors"><ArrowUp size={12} /></button>
            <button onclick={() => moveControl(i, 1)} disabled={i === controlsOrder.length - 1} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white disabled:opacity-25 disabled:hover:bg-transparent transition-colors"><ArrowDown size={12} /></button>
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
    {:else if storeErrorMsg}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <AlertTriangle size={28} class="text-amber-400" />
        <p class="text-sm text-slate-300">Couldn't reach the theme store</p>
        <p class="text-xs text-slate-500 max-w-sm">{storeErrorMsg}</p>
        <button class="mt-1 text-[11px] px-2.5 py-1 rounded-md bg-slate-700 hover:bg-slate-600 text-white" onclick={loadStore}>Try again</button>
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
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-3">
        {#each storeThemes as theme (theme.downloadUrl)}
          {@const installed = isThemeInstalled(theme)}
          {@const pct = installing[theme.downloadUrl]}
          <div class="rounded-xl border border-white/10 bg-slate-800/40 p-3">
            <span class="text-sm font-medium text-white">{theme.name}</span>
            {#if theme.author}<span class="text-[10px] text-slate-500 ml-1">by {theme.author}</span>{/if}
            <p class="text-[11px] text-slate-400 mt-0.5">{theme.description}</p>
            {#if pct !== undefined}
              <div class="mt-2 h-1.5 rounded-full bg-slate-700 overflow-hidden"><div class="h-full bg-blue-500 transition-all" style="width: {pct}%"></div></div>
            {:else if installed}
              <div class="mt-2 flex items-center gap-2">
                <span class="text-[11px] px-2 py-1 rounded-md bg-emerald-500/15 text-emerald-300 flex items-center gap-1"><Check size={12} /> Installed</span>
                <button class="text-[11px] px-2 py-1 rounded-md bg-slate-700 hover:bg-red-500/80 text-white" onclick={() => uninstallStoreTheme(installed)}>Remove</button>
              </div>
            {:else}
              <button class="mt-2 text-[11px] px-2.5 py-1 rounded-md bg-blue-500 text-white font-medium hover:bg-blue-400 transition-colors" onclick={() => installStoreTheme(theme)}>Download</button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
    {#if storeWarnings.length > 0}
      <p class="text-[10px] text-amber-400/70 mt-3">{storeWarnings.length} entr{storeWarnings.length === 1 ? 'y was' : 'ies were'} skipped (invalid store listing).</p>
    {/if}
  {/if}
</div>
