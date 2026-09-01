export type PluginKind = 'panel-widget' | 'app' | 'extension' | 'theme-companion';

export interface PluginManifest {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  /** Lucide icon name, same convention as ShellTheme.previewIcon. */
  icon: string;
  kind: PluginKind;
  /** Where this plugin's code/assets come from — a store entry always
   * has one; a manually "installed" entry might not (see
   * PluginsSection's "Add from URL" flow, itself also a stub for now). */
  downloadUrl?: string;
  homepage?: string;
  /** Human-readable capability descriptions shown to the person before
   * install — not enforced by anything yet (see this file's own doc);
   * purely informational until a real permission-gated plugin runtime
   * exists to actually check these against. */
  permissions?: string[];
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
