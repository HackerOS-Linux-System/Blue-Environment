import { SystemBridge } from './systemBridge';
import { configStore } from './configStore';

export interface IconThemeInfo {
  name: string;
  path: string;
  /** Installed under the user's home → can be removed from Blue Software. */
  removable: boolean;
  /** The theme Blue Environment currently uses (false for "automatic"). */
  active: boolean;
}

/** Fired on `window` after the set of installed themes changes. */
export const ICON_THEMES_CHANGED = 'blue-icon-themes-changed';
/** Fired on `window` after a theme was applied; `detail.name` is `null` for "automatic". */
export const ICON_THEME_APPLIED = 'blue-icon-theme-applied';

/** Archive types the backend can unpack (mirrors `archive_kind` in icon_store.rs). */
export const ICON_ARCHIVE_RE = /\.(zip|tar|tar\.gz|tgz|tar\.bz2|tbz2?|tar\.xz|txz|tar\.zst|tzst)$/i;

export const isIconArchive = (fileName: string): boolean => ICON_ARCHIVE_RE.test(fileName);

export const iconThemes = {
  list: (): Promise<IconThemeInfo[]> =>
    SystemBridge.invokeCommand<IconThemeInfo[]>('icon_store_list').catch(() => []),

  /** Unpacks a downloaded archive; resolves to the installed theme names. */
  async install(path: string): Promise<string[]> {
    const names = await SystemBridge.invokeCommand<string[]>('icon_store_install', { path });
    window.dispatchEvent(new CustomEvent(ICON_THEMES_CHANGED));
    return names;
  },

  async remove(name: string): Promise<void> {
    await SystemBridge.invokeCommand('icon_store_remove', { name });
    // The backend resets the active theme if it was the removed one — mirror that in the config.
    if (configStore.get().iconTheme === name) await configStore.save({ iconTheme: '' });
    window.dispatchEvent(new CustomEvent(ICON_THEMES_CHANGED));
    window.dispatchEvent(new CustomEvent(ICON_THEME_APPLIED, { detail: { name: null } }));
  },

  /** `null` / '' = automatic (system theme + Papirus fallbacks). */
  async apply(name: string | null): Promise<void> {
    const theme = name && name.trim() ? name : null;
    await SystemBridge.invokeCommand('set_icon_theme', { theme });
    await configStore.save({ iconTheme: theme ?? '' });
    window.dispatchEvent(new CustomEvent(ICON_THEME_APPLIED, { detail: { name: theme } }));
  },
};
