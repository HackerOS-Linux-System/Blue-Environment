<script lang="ts">
  /**
   * Renders one installed Blue Store app (or, for the plugin equivalent of
   * this concept — see `PluginRuntime.svelte` — a similarly-hosted plugin)
   * as a normal window's content, indistinguishable at the DOM level from a
   * built-in app.
   *
   * How this differs from an iframe/sandboxed plugin approach, by design
   * (this is the architecture requested for Blue Store apps specifically):
   * the community package's compiled JavaScript is loaded as a real ES
   * module directly into the shell's own page (via a same-origin `blob:`
   * URL, since a Tauri webview's CSP does not allow loading arbitrary
   * `asset://`-scheme scripts as ESM) and mounts straight into a `<div>`
   * that lives inside this window's own DOM tree — one process, one page,
   * one window, exactly like Blue Code or Blue Docs. There is no
   * postMessage bridge and no separate rendering context to keep in sync.
   *
   * The contract a package's `entry.js` must satisfy (documented in full
   * in `api/README.md`):
   *
   *   export default function mount(root: HTMLElement, api: BlueApi): (() => void) | void
   *
   * `mount` may render however it likes — vanilla DOM, a framework's own
   * `createRoot`/`new Component({ target: root })`/whatever the author
   * compiled down to — as long as the compiled bundle is a single ESM file
   * with a default export matching that signature. The optional returned
   * function is called on unmount (window closed) to let the app clean up
   * timers/subscriptions/its own framework root.
   */
  import { onDestroy, onMount } from 'svelte';
  import { AlertTriangle, RotateCcw } from 'lucide-svelte';
  import { readInstalledFile } from '../../../utils/blueStore';
  import type { Receipt } from '../../../utils/blueStore';
  import { createBlueApi } from '../../../utils/blueApi';
  import { closeWindow } from '../../../stores/windowManager';

  export let windowId: string;
  export let receipt: Receipt;

  let container: HTMLDivElement;
  let error: string | null = null;
  let loading = true;
  let cleanup: (() => void) | void | null = null;
  let blobUrls: string[] = [];

  async function load() {
    loading = true;
    error = null;
    try {
      if (!receipt) throw new Error('This window was opened without a package to run.');
      const jsText = await readInstalledFile(receipt.kind, receipt.id, receipt.entry);

      if (receipt.style) {
        try {
          const cssText = await readInstalledFile(receipt.kind, receipt.id, receipt.style);
          const styleEl = document.createElement('style');
          styleEl.textContent = cssText;
          styleEl.dataset.blueApp = receipt.id;
          container.appendChild(styleEl);
        } catch {
          // A missing/unreadable stylesheet shouldn't block the app itself.
        }
      }

      const blob = new Blob([jsText], { type: 'text/javascript' });
      const url = URL.createObjectURL(blob);
      blobUrls.push(url);

      // eslint-disable-next-line no-unsanitized/method -- url is a local blob: URL we just created from installed, checksum-verified package contents, not user input.
      const mod = await import(/* @vite-ignore */ url);
      const mountFn = mod?.default;
      if (typeof mountFn !== 'function') {
        throw new Error(`"${receipt.entry}" does not export a default mount(root, api) function.`);
      }

      const api = createBlueApi(
        { id: receipt.id, name: receipt.name, version: receipt.version, kind: receipt.kind, permissions: receipt.permissions },
        () => closeWindow(windowId),
      );
      cleanup = mountFn(container, api);
    } catch (e: any) {
      error = e?.message ?? String(e);
      console.error(`[CommunityAppHost] failed to start ${receipt?.id ?? '(unknown)'}:`, e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  onDestroy(() => {
    try {
      if (typeof cleanup === 'function') cleanup();
    } catch (e) {
      console.error('[CommunityAppHost] cleanup() threw:', e);
    }
    blobUrls.forEach((u) => URL.revokeObjectURL(u));
  });

  function retry() {
    if (container) container.innerHTML = '';
    load();
  }
</script>

<div class="w-full h-full relative theme-bg-primary">
  {#if loading}
    <div class="absolute inset-0 flex items-center justify-center theme-text-secondary text-sm">
      Starting {receipt?.name ?? 'app'}…
    </div>
  {:else if error}
    <div class="absolute inset-0 flex flex-col items-center justify-center gap-3 p-6 text-center">
      <AlertTriangle size={32} class="text-amber-400" />
      <div class="theme-text-primary font-medium">Could not start {receipt?.name ?? 'this app'}</div>
      <div class="theme-text-secondary text-xs max-w-md break-words">{error}</div>
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-xs"
        on:click={retry}
      >
        <RotateCcw size={14} /> Try again
      </button>
    </div>
  {/if}
  <div bind:this={container} class="w-full h-full" class:hidden={loading || !!error}></div>
</div>
