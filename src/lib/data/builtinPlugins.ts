export type PluginKind = 'panel-widget' | 'app' | 'extension' | 'theme-companion';

/** Capabilities a plugin can request and — as of the sandboxed runtime
 * in `PluginRuntime.svelte` — actually has enforced against it. Each
 * corresponds to one or more methods on the `BlueEnvPluginAPI` bridge
 * object exposed inside the plugin's sandboxed iframe; calling a
 * bridge method whose permission wasn't declared (and, in a future
 * pass, explicitly granted by the person rather than just declared by
 * the plugin) gets rejected by the host side, not merely hidden from
 * the plugin — see `PluginRuntime.svelte`'s `handleMessage`. */
export type PluginPermission = 'notifications' | 'clipboard' | 'storage';

export interface PluginManifest {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  /** Lucide icon name, same convention as ShellTheme.previewIcon. */
  icon: string;
  kind: PluginKind;
  /** Where this plugin's *code* comes from. Used two ways: shown as a
   * link for a person evaluating a Store/URL-installed plugin before
   * installing, and — since the sandboxed runtime landed — fetched as
   * plain JS text and run inside `PluginRuntime.svelte`'s sandboxed
   * iframe when the person clicks "Run". A plugin manifest with no
   * `downloadUrl` (and no `inlineSource`) can still be installed/
   * tracked but has nothing for "Run" to execute. */
  downloadUrl?: string;
  /** For the one bundled example plugin only (see `EXAMPLE_PLUGIN`
   * below) — raw JS source embedded directly in this file instead of
   * fetched from a URL, so the sandboxed runtime has something to
   * demonstrate against without requiring network access. Real
   * Store/URL-installed plugins always use `downloadUrl` instead. */
  inlineSource?: string;
  homepage?: string;
  /** Which `PluginPermission`s this plugin requests, shown to the
   * person before install/run — and, since the sandboxed runtime
   * landed, actually enforced: a bridge call for a permission not
   * listed here is rejected by the host, not merely hidden from the
   * plugin. See `PluginRuntime.svelte`. */
  permissions?: PluginPermission[];
}

/** Installed-plugin bookkeeping, separate from the manifest itself
 * (a manifest describes what a plugin *is*; this describes the local
 * fact of it being installed) — stored in `UserConfig.installedPlugins`. */
export interface InstalledPlugin {
  manifest: PluginManifest;
  installedAt: string;
  enabled: boolean;
}

/** Empty on purpose — see this file's module doc and
 * PluginsSection.svelte: there is no meaningful "comes with the OS"
 * plugin today, only the Store (fetched at runtime, currently also
 * empty — see config/stores/plugins-store.json) and a manual-install
 * path. */
export const BUILTIN_PLUGINS: PluginManifest[] = [];

/** A single bundled demo plugin — not auto-installed, not part of
 * `BUILTIN_PLUGINS` (so it never appears in the ordinary Installed
 * list unintentionally); `PluginsSection.svelte` offers it separately
 * as "Try the sandbox" specifically so the sandboxed runtime
 * (`PluginRuntime.svelte`) has something real to demonstrate without
 * requiring the person to already have a plugin URL handy. Exercises
 * all three enforced permissions (notifications, clipboard, storage)
 * so trying it is also a real end-to-end check that the sandbox and
 * its permission gate both actually work. */
export const EXAMPLE_PLUGIN: PluginManifest = {
  id: 'com.blueenvironment.example-sandbox-demo',
  name: 'Sandbox Demo',
  description: 'A tiny bundled plugin that exercises every permission the sandboxed runtime enforces — notifications, clipboard, and per-plugin storage — so you can see the real thing working.',
  author: 'Blue Environment',
  version: '1.0.0',
  icon: 'FlaskConical',
  kind: 'extension',
  permissions: ['notifications', 'clipboard', 'storage'],
  inlineSource: `
    (async () => {
      const countRaw = await BlueEnvPluginAPI.storage.get('runCount');
      const count = (parseInt(countRaw ?? '0', 10) || 0) + 1;
      await BlueEnvPluginAPI.storage.set('runCount', String(count));

      await BlueEnvPluginAPI.notify('Sandbox Demo', 'Hello from inside the sandbox! This is run #' + count + '.');
      await BlueEnvPluginAPI.setClipboardText('Copied by Sandbox Demo, run #' + count);

      document.getElementById('out').textContent =
        'Ran ' + count + ' time(s). Sent a notification and copied text to your clipboard — ' +
        'check both. Storage, notifications, and clipboard access all went through the host\\'s ' +
        'permission-checked bridge, not a direct API — try removing this plugin\\'s "clipboard" ' +
        'permission and re-running to see the call get rejected instead of silently working.';
    })().catch((e) => {
      document.getElementById('out').textContent = 'Error: ' + e.message;
    });
  `,
};
