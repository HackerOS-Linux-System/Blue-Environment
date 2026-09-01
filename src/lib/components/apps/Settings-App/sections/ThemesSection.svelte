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
  import { Check, RefreshCw, Store as StoreIcon, Sparkles as SparklesIcon, Clock, ExternalLink, HardDrive, Info } from 'lucide-svelte';
  import type { UserConfig, SystemTheme } from '../../../../types';
  import { t } from '../../../../stores/language';
  import { BUILTIN_THEMES, DEFAULT_SHELL_THEME_ID, type ShellTheme } from '../../../../data/builtinThemes';
  import { SystemBridge } from '../../../../utils/systemBridge';
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

  onMount(() => { if (tab === 'store') loadStore(); });

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
