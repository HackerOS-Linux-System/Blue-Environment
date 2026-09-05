<script lang="ts">
  /**
   * Real sandboxed execution for one plugin — replaces "install just
   * records a manifest, nothing ever runs" with an actual runtime.
   *
   * ── The sandbox itself ───────────────────────────────────────────
   * The plugin's JS runs inside `<iframe sandbox="allow-scripts">`
   * with no `allow-same-origin`. That specific combination is the
   * whole security model: without `allow-same-origin`, the browser
   * gives the iframe a unique, opaque origin on every load, so even
   * though the script executes, it cannot read this app's DOM, cookies,
   * `localStorage`, or call back into the parent's JS directly —
   * cross-origin restrictions apply as if it were an entirely different
   * website. The *only* channel in or out is `postMessage`, which is
   * exactly the bridge `handleMessage` below mediates and permission-
   * checks. `allow-scripts` alone (the plugin needs to run JS at all)
   * does not imply `allow-same-origin` — the two are independent sandbox
   * flags, and this deliberately requests only the first.
   *
   * ── The permission bridge ────────────────────────────────────────
   * Inside the iframe, a small `BlueEnvPluginAPI` global (defined in
   * the `srcdoc` HTML below) wraps every capability as
   * `postMessage({ id, method, args })` + waiting for a matching
   * `{ id, result }`/`{ id, error }` reply. `handleMessage` here is the
   * other end: it verifies `event.source` really is *this* iframe (not
   * some other frame or a spoofed message from elsewhere on the page),
   * checks `method`'s required permission is in the plugin's own
   * declared `manifest.permissions`, and only then performs the real
   * action via `SystemBridge` — an unlisted permission gets a rejection
   * sent back, not a silently-ignored call and not access anyway.
   *
   * ── What this doesn't cover yet ──────────────────────────────────
   * - Permissions are enforced against what the plugin *declared* in
   *   its own manifest, not yet against an explicit person-facing
   *   "allow this plugin to use your clipboard?" grant — a
   *   consent step per permission (with persisted grants) is the
   *   natural next layer on top of this, analogous to how declaring
   *   `permissions` in this file today is informational-turned-
   *   enforced but still self-declared by the plugin author.
   * - Only three capabilities exist (`notify`, clipboard read/write,
   *   namespaced storage) — enough to prove the sandbox and permission
   *   gate both genuinely work (see `EXAMPLE_PLUGIN`), not a complete
   *   plugin API surface.
   * - No resource limits (a plugin could still spin-loop inside its own
   *   iframe and consume CPU) — the sandbox's job here is containment
   *   (what it can *reach*), not resource quotas.
   */
  import { onDestroy } from 'svelte';
  import { X, ShieldAlert, ShieldCheck } from 'lucide-svelte';
  import type { PluginManifest, PluginPermission } from '../data/builtinPlugins';
  import { SystemBridge } from '../utils/systemBridge';
  import { notificationManager } from '../utils/notificationManager';

  export let manifest: PluginManifest;
  export let onClose: () => void;

  let iframeEl: HTMLIFrameElement;
  let loadError = '';
  let ready = false;

  const permissions = new Set<PluginPermission>(manifest.permissions ?? []);
  const STORAGE_PREFIX = `plugin-storage:${manifest.id}:`;

  /** Every bridge method a plugin can call, and which permission (if
   * any) it requires. `null` means "no permission needed" (currently
   * none are — every real capability requires one). */
  const METHOD_PERMISSIONS: Record<string, PluginPermission> = {
    notify: 'notifications',
    getClipboardText: 'clipboard',
    setClipboardText: 'clipboard',
    'storage.get': 'storage',
    'storage.set': 'storage',
  };

  async function handleMessage(event: MessageEvent) {
    // Reject anything not from this exact iframe — without this check,
    // any other frame/script on the page could forge a
    // `{id, method, args}` message and have it treated as a plugin
    // call, which would make the permission check below meaningless
    // (it'd still be "permitted", just not actually from the sandbox).
    if (!iframeEl || event.source !== iframeEl.contentWindow) return;
    const { id, method, args } = event.data ?? {};
    if (typeof id !== 'string' || typeof method !== 'string') return;

    const reply = (payload: { result?: any; error?: string }) => {
      iframeEl.contentWindow?.postMessage({ id, ...payload }, '*');
    };

    const requiredPermission = METHOD_PERMISSIONS[method];
    if (requiredPermission && !permissions.has(requiredPermission)) {
      reply({ error: `This plugin didn't declare the "${requiredPermission}" permission — the call to ${method}() was rejected.` });
      return;
    }

    try {
      switch (method) {
        case 'notify': {
          const [title, body] = args ?? [];
          // Real, existing app-wide notification-center sink (shows up
          // in Blue Notifications / the notification bell) — not an OS-
          // level toast, since nothing in this codebase's SystemBridge
          // triggers one generically today; this is what "notify" from
          // inside the sandbox actually reaches.
          notificationManager.add({ title: String(title ?? manifest.name), body: String(body ?? ''), appId: manifest.id, app: manifest.name });
          reply({ result: true });
          break;
        }
        case 'getClipboardText': {
          const text = await SystemBridge.readText();
          reply({ result: text ?? '' });
          break;
        }
        case 'setClipboardText': {
          const [text] = args ?? [];
          await SystemBridge.copyText(String(text ?? ''));
          reply({ result: true });
          break;
        }
        case 'storage.get': {
          const [key] = args ?? [];
          reply({ result: localStorage.getItem(STORAGE_PREFIX + String(key)) });
          break;
        }
        case 'storage.set': {
          const [key, value] = args ?? [];
          localStorage.setItem(STORAGE_PREFIX + String(key), String(value));
          reply({ result: true });
          break;
        }
        default:
          reply({ error: `Unknown method: ${method}` });
      }
    } catch (e: any) {
      reply({ error: e?.message ?? String(e) });
    }
  }

  /** The sandboxed document: the bridge shim (defines `BlueEnvPluginAPI`
   * in terms of `postMessage`) plus the plugin's own code, inlined
   * directly into `srcdoc` rather than given as a `src` URL — a
   * `sandbox`ed iframe with no `allow-same-origin` still can navigate/
   * fetch on its own unless further restricted, and inlining the
   * already-fetched source means the plugin never gets a chance to load
   * anything this runtime didn't explicitly hand it. */
  function buildSrcdoc(pluginSource: string): string {
    return `<!DOCTYPE html>
<html><head><meta charset="utf-8">
<style>body{font-family:system-ui,sans-serif;background:#0f172a;color:#e2e8f0;padding:16px;font-size:13px;margin:0;}</style>
</head><body>
<div id="out">Running…</div>
<script>
  let nextId = 0;
  const pending = new Map();
  window.addEventListener('message', (e) => {
    const { id, result, error } = e.data ?? {};
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    if (error) p.reject(new Error(error)); else p.resolve(result);
  });
  function call(method, ...args) {
    const id = 'call-' + (nextId++);
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      window.parent.postMessage({ id, method, args }, '*');
    });
  }
  window.BlueEnvPluginAPI = {
    notify: (title, body) => call('notify', title, body),
    getClipboardText: () => call('getClipboardText'),
    setClipboardText: (text) => call('setClipboardText', text),
    storage: {
      get: (key) => call('storage.get', key),
      set: (key, value) => call('storage.set', key, value),
    },
  };
<\/script>
<script>
${pluginSource}
<\/script>
</body></html>`;
  }

  async function loadAndRun() {
    try {
      let source = manifest.inlineSource;
      if (!source && manifest.downloadUrl) {
        const res = await fetch(manifest.downloadUrl);
        if (!res.ok) throw new Error(`Could not fetch plugin code (HTTP ${res.status})`);
        source = await res.text();
      }
      if (!source) throw new Error('This plugin has no code to run (no downloadUrl or inlineSource).');
      iframeEl.srcdoc = buildSrcdoc(source);
      ready = true;
    } catch (e: any) {
      loadError = e?.message ?? String(e);
    }
  }

  window.addEventListener('message', handleMessage);
  onDestroy(() => window.removeEventListener('message', handleMessage));
</script>

<div class="fixed inset-0 bg-black/70 flex items-center justify-center z-[999]">
  <div class="bg-slate-900 border border-white/10 rounded-xl w-[480px] max-h-[70vh] flex flex-col overflow-hidden">
    <div class="flex items-center justify-between px-4 py-3 border-b border-white/10">
      <div class="flex items-center gap-2 min-w-0">
        <ShieldCheck size={15} class="text-emerald-400 shrink-0" />
        <span class="text-sm font-medium truncate">{manifest.name}</span>
        <span class="text-[10px] text-slate-500 shrink-0">sandboxed</span>
      </div>
      <button on:click={onClose} class="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors">
        <X size={15} />
      </button>
    </div>

    {#if (manifest.permissions?.length ?? 0) > 0}
      <div class="px-4 py-2 border-b border-white/10 flex items-center gap-1.5 flex-wrap">
        <ShieldAlert size={11} class="text-amber-400 shrink-0" />
        <span class="text-[10px] text-slate-500">Permissions:</span>
        {#each manifest.permissions ?? [] as perm}
          <span class="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 text-slate-300">{perm}</span>
        {/each}
      </div>
    {/if}

    <div class="flex-1 overflow-hidden relative">
      {#if loadError}
        <div class="p-4 text-xs text-red-400">{loadError}</div>
      {:else}
        <iframe
          bind:this={iframeEl}
          on:load={() => { if (!ready) loadAndRun(); }}
          sandbox="allow-scripts"
          title={manifest.name}
          class="w-full h-64 border-0 bg-slate-950"
        />
      {/if}
    </div>
  </div>
</div>
