<script lang="ts">
  /**
   * Plugins settings section — "Installed" (empty by default; see
   * builtinPlugins.ts's doc for why there's no meaningful built-in
   * plugin) and "Store" (downloadable plugins, same fetch/fallback
   * pattern as ThemesSection.svelte — see that component's module doc
   * for the remote-then-local-bundled-copy strategy, identical here
   * just pointed at plugins-store.json instead).
   *
   * ── What "installing" a plugin actually does right now ─────────────
   * Not a real plugin runtime — there is no sandboxed execution
   * environment, no code-loading mechanism, no permission enforcement
   * yet. "Install" from the Store, or "Add from URL" (which fetches
   * and shape-checks a manifest JSON from a person-supplied URL before
   * adding it — see `addFromUrl` below), both just append a manifest to
   * `UserConfig.installedPlugins` (persisted via the normal config
   * save path) with `enabled: true`, and it then shows up in the
   * "Installed" list like any other settings entry — a real, honest
   * foundation (the data model, the list UI, enable/disable/uninstall,
   * and now manifest-URL ingestion) for a future runtime to build on,
   * not a working plugin system pretending to be one. Explicitly not
   * oversold in the UI copy below either — the empty state and each
   * installed card both say plainly that this is early.
   */
  import { onMount } from 'svelte';
  import * as Icons from 'lucide-svelte';
  import { RefreshCw, Store as StoreIcon, Puzzle, ExternalLink, Trash2, Link as LinkIcon, Play as PlayIcon, FlaskConical, AlertTriangle, Check, PackageCheck } from 'lucide-svelte';
  import type { UserConfig } from '../../../../types';
  import { t } from '../../../../stores/language';
  import { BUILTIN_PLUGINS, EXAMPLE_PLUGIN, type PluginManifest, type InstalledPlugin } from '../../../../data/builtinPlugins';
  import PluginRuntime from '../../../PluginRuntime.svelte';
  import {
    fetchStoreIndex, installPackage, uninstallPackage, listInstalled, storeErrorMessage,
    type StoreIndexEntry, type Receipt,
  } from '../../../../utils/blueStore';

  export let config: UserConfig;
  export let onSave: (p: Partial<UserConfig>) => Promise<void>;

  let runningPlugin: PluginManifest | null = null;

  let tab: 'installed' | 'store' = 'installed';
  /**
   * BUGFIX: this tab used to `fetch(STORE_URL_REMOTE)` from the webview
   * (blocked by the shell's CSP `connect-src`, so it silently always fell
   * back to a bundled, empty JSON snapshot) and "Install" only appended a
   * manifest to config — no file was ever actually downloaded or written
   * anywhere. Both now go through `blueStore.ts` → Rust, which really
   * fetches `blue.hk`, downloads + checksums the `.blue` archive, and
   * installs it into `/usr/share/Blue-Environment/plugins/<id>/`
   * (elevating via pkexec/sudo as needed).
   */
  let storePlugins: StoreIndexEntry[] = [];
  let storeWarnings: string[] = [];
  let storeLoading = false;
  let storeErrorMsg: string | null = null;
  let storeLoaded = false;
  let installedStorePlugins: Receipt[] = [];
  let installing: Record<string, number> = {};
  let addUrlValue = '';
  let addUrlOpen = false;
  let addUrlBusy = false;
  let addUrlError = '';

  $: installed = config.installedPlugins ?? [];

  onMount(() => { if (tab === 'store') loadStore(); refreshInstalledStorePlugins(); });

  async function refreshInstalledStorePlugins() {
    try {
      installedStorePlugins = await listInstalled('plugin');
    } catch {
      // Non-fatal — the legacy (config-based) list above still works.
    }
  }

  /** Minimal shape-check on a fetched manifest — not a schema
   * validator, just enough to reject something that clearly isn't a
   * plugin manifest (wrong content type, a redirect to an HTML error
   * page, a JSON file for something else entirely) before it gets
   * treated as one. */
  function isPluginManifest(v: any): v is PluginManifest {
    return v && typeof v === 'object'
      && typeof v.id === 'string' && v.id.length > 0
      && typeof v.name === 'string' && v.name.length > 0
      && typeof v.version === 'string'
      && typeof v.description === 'string';
  }

  /** Fetches a plugin manifest JSON from a person-supplied URL and adds
   * it to `installedPlugins` — the "Add from URL" flow the UI
   * previously only showed a disabled button for. Still subject to
   * this section's own module doc: installing only registers the
   * manifest, there's no code-loading/sandboxed runtime that actually
   * executes anything from `downloadUrl` yet. This function's job is
   * only to get a real manifest JSON safely into that same "installed,
   * not yet runnable" list the Store's `installFromStore` already
   * populates — not to build the runtime that's still a separate,
   * larger follow-up. */
  async function addFromUrl() {
    const url = addUrlValue.trim();
    if (!url) return;
    addUrlError = '';
    addUrlBusy = true;
    try {
      let parsed: URL;
      try {
        parsed = new URL(url);
      } catch {
        addUrlError = 'Not a valid URL.';
        return;
      }
      if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
        addUrlError = 'Only http(s) URLs are supported.';
        return;
      }

      const res = await fetch(url, { cache: 'no-store' });
      if (!res.ok) {
        addUrlError = `Could not fetch manifest (HTTP ${res.status}).`;
        return;
      }
      const data = await res.json().catch(() => null);
      if (!isPluginManifest(data)) {
        addUrlError = 'That URL did not return a valid plugin manifest (expected at least id, name, version, description).';
        return;
      }
      if (installed.some((p) => p.manifest.id === data.id)) {
        addUrlError = `A plugin with id "${data.id}" is already installed.`;
        return;
      }

      const entry: InstalledPlugin = { manifest: data, installedAt: new Date().toISOString(), enabled: true };
      await onSave({ installedPlugins: [...installed, entry] });
      addUrlValue = '';
      addUrlOpen = false;
    } catch (e: any) {
      addUrlError = `Failed to add plugin: ${e?.message ?? e}`;
    } finally {
      addUrlBusy = false;
    }
  }

  async function loadStore() {
    storeLoading = true;
    storeErrorMsg = null;
    try {
      const index = await fetchStoreIndex('plugin');
      storePlugins = index.entries;
      storeWarnings = index.warnings;
    } catch (e) {
      storeErrorMsg = storeErrorMessage(e);
      storePlugins = [];
    } finally {
      storeLoading = false;
      storeLoaded = true;
    }
  }

  function selectTab(next: 'installed' | 'store') {
    tab = next;
    if (next === 'store' && !storeLoaded && !storeLoading) loadStore();
  }

  function isPluginInstalledFromStore(entry: StoreIndexEntry): Receipt | undefined {
    return installedStorePlugins.find((r) => r.sourceUrl === entry.downloadUrl);
  }

  async function installFromStore(entry: StoreIndexEntry) {
    installing = { ...installing, [entry.downloadUrl]: 0 };
    try {
      await installPackage(entry.downloadUrl, 'plugin', (pct) => { installing = { ...installing, [entry.downloadUrl]: pct }; });
      await refreshInstalledStorePlugins();
    } catch (e) {
      storeErrorMsg = `${entry.name}: ${storeErrorMessage(e)}`;
    } finally {
      const { [entry.downloadUrl]: _drop, ...rest } = installing;
      installing = rest;
    }
  }

  async function uninstallStorePlugin(receipt: Receipt) {
    try {
      await uninstallPackage('plugin', receipt.id);
      await refreshInstalledStorePlugins();
    } catch (e) {
      storeErrorMsg = `${receipt.name}: ${storeErrorMessage(e)}`;
    }
  }

  async function uninstall(id: string) {
    await onSave({ installedPlugins: installed.filter((p) => p.manifest.id !== id) });
  }

  async function toggleEnabled(id: string, enabled: boolean) {
    await onSave({ installedPlugins: installed.map((p) => (p.manifest.id === id ? { ...p, enabled } : p)) });
  }

  // Svelte's template-expression parser doesn't accept an inline `as`
  // type-cast inside a `{...}` attribute expression (confirmed by a
  // real svelte-check error: "Unexpected token" pointing exactly at
  // `(e.currentTarget as HTMLInputElement)` — this isn't a plain-TS
  // context the way a `<script>` block is) — pulled the cast out into
  // its own named handler instead of inlining it in the markup.
  function handleToggleChange(id: string, e: Event) {
    toggleEnabled(id, (e.currentTarget as HTMLInputElement).checked);
  }

  function iconFor(name: string) {
    return (Icons as any)[name] ?? Icons.Puzzle;
  }
</script>

<div class="max-w-3xl relative">
  <h2 class="text-lg font-semibold text-white mb-1">{$t('settings.tab.plugins') ?? 'Plugins'}</h2>
  <p class="text-xs text-slate-400 mb-4">
    Extend the shell. Early foundation — installing a plugin registers it here, there's no plugin runtime executing anything yet.
  </p>

  <div class="flex gap-1 mb-4 border-b border-white/5">
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors {tab === 'installed' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" on:click={() => selectTab('installed')}>
      Installed {#if installed.length}<span class="text-slate-500">({installed.length})</span>{/if}
    </button>
    <button class="px-3 py-2 text-xs font-medium border-b-2 transition-colors flex items-center gap-1.5 {tab === 'store' ? 'border-blue-500 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}" on:click={() => selectTab('store')}>
      <StoreIcon size={12} /> Store
    </button>
  </div>

  {#if tab === 'installed'}
    {#if installed.length === 0}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <Puzzle size={28} class="text-slate-600" />
        <p class="text-sm text-slate-300">No plugins installed</p>
        <p class="text-xs text-slate-500 max-w-sm">Blue Environment doesn't come with any built-in plugins. Browse the Store, or add one from a URL.</p>
        <button class="mt-2 text-[11px] px-2.5 py-1.5 rounded-md bg-slate-700 text-slate-200 hover:bg-slate-600 transition-colors flex items-center gap-1.5" on:click={() => (addUrlOpen = !addUrlOpen)}>
          <LinkIcon size={11} /> Add from URL
        </button>
        <button class="mt-1 text-[11px] px-2.5 py-1.5 rounded-md bg-emerald-600/20 text-emerald-300 hover:bg-emerald-600/30 transition-colors flex items-center gap-1.5" on:click={() => (runningPlugin = EXAMPLE_PLUGIN)}>
          <FlaskConical size={11} /> Try the sandbox (bundled demo, no install needed)
        </button>
        {#if addUrlOpen}
          <div class="mt-2 flex items-center gap-2 w-full max-w-sm">
            <input
              bind:value={addUrlValue}
              placeholder="https://…/plugin.json"
              on:keydown={(e) => e.key === 'Enter' && addFromUrl()}
              class="flex-1 bg-slate-800 border border-white/10 rounded-md px-2 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/50"
            />
            <button
              class="text-[11px] px-2.5 py-1.5 rounded-md bg-blue-500 text-white font-medium hover:bg-blue-400 disabled:opacity-40 transition-colors shrink-0"
              disabled={!addUrlValue.trim() || addUrlBusy}
              on:click={addFromUrl}
            >
              {addUrlBusy ? 'Adding…' : 'Add'}
            </button>
          </div>
          {#if addUrlError}
            <p class="text-[10px] text-red-400 max-w-sm">{addUrlError}</p>
          {:else}
            <p class="text-[10px] text-slate-600 max-w-sm">Fetches and validates the manifest JSON at that URL, then installs it the same way a Store entry would (see this section's module doc for what "installed" does and doesn't mean yet).</p>
          {/if}
        {/if}
      </div>
    {:else}
      <div class="flex items-center justify-between mb-2">
        <button class="text-[11px] px-2.5 py-1.5 rounded-md bg-slate-700 text-slate-200 hover:bg-slate-600 transition-colors flex items-center gap-1.5" on:click={() => (addUrlOpen = !addUrlOpen)}>
          <LinkIcon size={11} /> Add from URL
        </button>
      </div>
      {#if addUrlOpen}
        <div class="mb-3 flex items-center gap-2">
          <input
            bind:value={addUrlValue}
            placeholder="https://…/plugin.json"
            on:keydown={(e) => e.key === 'Enter' && addFromUrl()}
            class="flex-1 bg-slate-800 border border-white/10 rounded-md px-2 py-1.5 text-xs text-white placeholder:text-slate-500 focus:outline-none focus:border-blue-500/50"
          />
          <button
            class="text-[11px] px-2.5 py-1.5 rounded-md bg-blue-500 text-white font-medium hover:bg-blue-400 disabled:opacity-40 transition-colors shrink-0"
            disabled={!addUrlValue.trim() || addUrlBusy}
            on:click={addFromUrl}
          >
            {addUrlBusy ? 'Adding…' : 'Add'}
          </button>
        </div>
        {#if addUrlError}
          <p class="text-[10px] text-red-400 mb-2">{addUrlError}</p>
        {/if}
      {/if}
      <div class="space-y-2">
        {#each installed as entry (entry.manifest.id)}
          <div class="flex items-center gap-3 rounded-lg border border-white/10 bg-slate-800/40 p-3">
            <div class="w-9 h-9 rounded-lg bg-slate-700/50 flex items-center justify-center shrink-0">
              <svelte:component this={iconFor(entry.manifest.icon)} size={16} class="text-slate-300" />
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-white truncate">{entry.manifest.name}</span>
                <span class="text-[10px] text-slate-500">v{entry.manifest.version}</span>
              </div>
              <p class="text-[11px] text-slate-400 truncate">{entry.manifest.description}</p>
            </div>
            {#if entry.enabled && (entry.manifest.downloadUrl || entry.manifest.inlineSource)}
              <button
                class="text-[11px] px-2 py-1 rounded-md bg-emerald-600/20 text-emerald-300 hover:bg-emerald-600/30 transition-colors shrink-0 flex items-center gap-1"
                on:click={() => (runningPlugin = entry.manifest)}
                title="Run in sandbox"
              >
                <PlayIcon size={11} /> Run
              </button>
            {/if}
            <label class="inline-flex items-center cursor-pointer shrink-0">
              <input type="checkbox" checked={entry.enabled} on:change={(e) => handleToggleChange(entry.manifest.id, e)} class="sr-only peer" />
              <div class="w-8 h-4.5 bg-slate-700 rounded-full peer peer-checked:bg-blue-500 transition-colors relative">
                <div class="absolute top-0.5 left-0.5 w-3.5 h-3.5 bg-white rounded-full transition-transform peer-checked:translate-x-3.5"></div>
              </div>
            </label>
            <button class="p-1.5 rounded-md text-slate-500 hover:text-red-400 hover:bg-red-500/10 transition-colors shrink-0" on:click={() => uninstall(entry.manifest.id)} title="Uninstall">
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
    {#if installedStorePlugins.length > 0}
      <div class="mt-4">
        <p class="text-[11px] text-slate-500 mb-2 flex items-center gap-1.5"><PackageCheck size={12} /> From Blue Store (real .blue packages in /usr/share/Blue-Environment/plugins/)</p>
        <div class="space-y-2">
          {#each installedStorePlugins as receipt (receipt.id)}
            <div class="flex items-center gap-3 rounded-lg border border-white/10 bg-slate-800/40 p-3">
              <div class="w-9 h-9 rounded-lg bg-slate-700/50 flex items-center justify-center shrink-0">
                <svelte:component this={iconFor(receipt.icon ?? '')} size={16} class="text-slate-300" />
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-medium text-white truncate">{receipt.name}</span>
                  <span class="text-[10px] text-slate-500">v{receipt.version}</span>
                </div>
                <p class="text-[11px] text-slate-400 truncate">{receipt.description}</p>
              </div>
              <button class="p-1.5 rounded-md text-slate-500 hover:text-red-400 hover:bg-red-500/10 transition-colors shrink-0" on:click={() => uninstallStorePlugin(receipt)} title="Uninstall">
                <Trash2 size={14} />
              </button>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {:else}
    <!-- Store tab -->
    {#if storeLoading}
      <div class="flex items-center gap-2 text-xs text-slate-400 py-8 justify-center">
        <RefreshCw size={14} class="animate-spin" /> Loading plugin store…
      </div>
    {:else if storeErrorMsg}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <AlertTriangle size={28} class="text-amber-400" />
        <p class="text-sm text-slate-300">Couldn't reach the plugin store</p>
        <p class="text-xs text-slate-500 max-w-sm">{storeErrorMsg}</p>
        <button class="mt-1 text-[11px] px-2.5 py-1 rounded-md bg-slate-700 hover:bg-slate-600 text-white" on:click={loadStore}>Try again</button>
      </div>
    {:else if storePlugins.length === 0}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <StoreIcon size={28} class="text-slate-600" />
        <p class="text-sm text-slate-300">No plugins in the store yet</p>
        <p class="text-xs text-slate-500 max-w-sm">
          The plugin store is live but empty for now — check back later, or
          <a class="text-blue-400 hover:underline inline-flex items-center gap-0.5" href="https://github.com/HackerOS-Linux-System/Blue-Environment/blob/main/config/stores/plugins-store.json" target="_blank" rel="noopener">
            see the store source <ExternalLink size={10} />
          </a>.
        </p>
      </div>
    {:else}
      <div class="space-y-2">
        {#each storePlugins as plugin (plugin.downloadUrl)}
          {@const installedReceipt = isPluginInstalledFromStore(plugin)}
          {@const pct = installing[plugin.downloadUrl]}
          <div class="flex items-center gap-3 rounded-lg border border-white/10 bg-slate-800/40 p-3">
            <div class="w-9 h-9 rounded-lg bg-slate-700/50 flex items-center justify-center shrink-0">
              <svelte:component this={iconFor(plugin.icon)} size={16} class="text-slate-300" />
            </div>
            <div class="flex-1 min-w-0">
              <span class="text-sm font-medium text-white">{plugin.name}</span>
              <p class="text-[11px] text-slate-400 truncate">{plugin.description}</p>
              {#if pct !== undefined}
                <div class="mt-1 h-1 rounded-full bg-slate-700 overflow-hidden w-32"><div class="h-full bg-blue-500 transition-all" style="width: {pct}%"></div></div>
              {/if}
            </div>
            {#if pct === undefined}
              {#if installedReceipt}
                <div class="flex items-center gap-2 shrink-0">
                  <span class="text-[11px] px-2 py-1 rounded-md bg-emerald-500/15 text-emerald-300 flex items-center gap-1"><Check size={12} /> Installed</span>
                  <button class="p-1.5 rounded-md text-slate-500 hover:text-red-400 hover:bg-red-500/10 transition-colors" on:click={() => uninstallStorePlugin(installedReceipt)} title="Uninstall">
                    <Trash2 size={14} />
                  </button>
                </div>
              {:else}
                <button class="text-[11px] px-2.5 py-1 rounded-md font-medium transition-colors shrink-0 bg-blue-500 text-white hover:bg-blue-400" on:click={() => installFromStore(plugin)}>
                  Install
                </button>
              {/if}
            {/if}
          </div>
        {/each}
      </div>
    {/if}
    {#if storeWarnings.length > 0}
      <p class="text-[10px] text-amber-400/70 mt-3">{storeWarnings.length} entr{storeWarnings.length === 1 ? 'y was' : 'ies were'} skipped (invalid store listing).</p>
    {/if}
  {/if}

  {#if runningPlugin}
    <PluginRuntime manifest={runningPlugin} onClose={() => (runningPlugin = null)} />
  {/if}
</div>
