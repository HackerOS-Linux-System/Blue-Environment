import { writable } from 'svelte/store';

/**
 * Blue Environment's own in-shell file/folder picker.
 *
 * Previously "open a folder" (e.g. Blue Code's Open Folder) and "open a
 * file" fell straight through to the OS-native picker
 * (`@tauri-apps/plugin-dialog`'s `open()`, see systemBridge.ts) — a GTK
 * (or platform-equivalent) window that looks nothing like the rest of
 * the shell and breaks immersion, exactly the gap KDE/GNOME/etc. don't
 * have (they always show *their own* file chooser, not a foreign one).
 * `BlueFilePicker.svelte` (mounted once in App.svelte, same pattern as
 * `DialogHost.svelte`) is that chooser for Blue Environment; this file
 * is its store, the same promise-resolving-on-close shape as
 * `stores/dialog.ts` uses for prompt/confirm/alert.
 */

export interface FilePickerFilter { name: string; extensions: string[]; }

export interface FilePickerOptions {
  mode: 'directory' | 'file';
  title?: string;
  /** Only used in 'file' mode. Empty/omitted = all files. */
  filters?: FilePickerFilter[];
  multiple?: boolean;
  /** Path to start browsing from. Defaults to HOME, or to the
   * remembered path for `rememberKey` if one is set (see below). */
  startPath?: string;
  confirmLabel?: string;
  /** Identifies *which* "Open" flow this is (e.g. `'blue-code.open-folder'`,
   * `'notepad.open-file'`) so BlueFilePicker.svelte can remember the last
   * directory that specific flow was used in and start there next time,
   * independently of every other app's own "last folder". Omit for
   * one-off pickers where that memory wouldn't make sense. */
  rememberKey?: string;
}

interface PendingPicker {
  options: FilePickerOptions;
  resolve: (paths: string[]) => void;
}

export const activeFilePicker = writable<PendingPicker | null>(null);

/** Resolves to the chosen paths, or an empty array if the user cancelled. */
export function pickPaths(options: FilePickerOptions): Promise<string[]> {
  return new Promise((resolve) => activeFilePicker.set({ options, resolve }));
}

export function closeFilePicker(current: PendingPicker, result: string[]) {
  current.resolve(result);
  activeFilePicker.set(null);
}
