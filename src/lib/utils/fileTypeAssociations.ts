import { writable, get } from 'svelte/store';
import { SystemBridge } from './systemBridge';
import {
  File, FileText, FileCode, FileJson, FileSpreadsheet, FileCog,
  Folder, Archive, Image, Music, Video, Film, Code, Code2,
  Terminal, Database, Book, BookOpen, Palette, Settings,
  Package, Boxes, Puzzle, Gamepad2, Wrench, FlaskConical,
  Layers, GitBranch, Lock, KeyRound, FileLock2, Cpu, HardDrive,
} from 'lucide-svelte';

export type MatchKind = 'extension' | 'mime';

export interface FileTypeAssociation {
  id: string;
  kind: MatchKind;
  pattern: string;
  icon: string;
  color: string;
  label: string;
  open_with_command: string | null;
}

/**
 * Kept in sync by hand with `ALLOWED_ICONS` in
 * `src-tauri/src/file_type_associations.rs` — the backend rejects any
 * icon name not in its own copy of this list, so the two must match.
 * This is what lets a user pick "any icon they like" from a dropdown
 * while the actual set of things that can render is still a fixed,
 * reviewed component list, never arbitrary content.
 */
export const ICON_COMPONENTS: Record<string, any> = {
  File, FileText, FileCode, FileJson, FileSpreadsheet, FileCog,
  Folder, Archive, Image, Music, Video, Film, Code, Code2,
  Terminal, Database, Book, BookOpen, Palette, Settings,
  Package, Boxes, Puzzle, Gamepad2, Wrench, FlaskConical,
  Layers, GitBranch, Lock, KeyRound, FileLock2, Cpu, HardDrive,
};

export const ICON_NAMES = Object.keys(ICON_COMPONENTS);

export const fileTypeAssociations = writable<FileTypeAssociation[]>([]);

let loaded = false;

/** Loads once and caches in the store; call `refreshFileTypeAssociations()` after any mutation elsewhere. */
export async function ensureFileTypeAssociationsLoaded(): Promise<FileTypeAssociation[]> {
  if (loaded) return get(fileTypeAssociations);
  return refreshFileTypeAssociations();
}

export async function refreshFileTypeAssociations(): Promise<FileTypeAssociation[]> {
  const list = await SystemBridge.invokeCommand<FileTypeAssociation[]>('file_type_get_associations').catch(() => []);
  loaded = true;
  fileTypeAssociations.set(list ?? []);
  return list ?? [];
}

export async function saveFileTypeAssociation(assoc: FileTypeAssociation): Promise<{ ok: true } | { ok: false; error: string }> {
  try {
    const list = await SystemBridge.invokeCommand<FileTypeAssociation[]>('file_type_upsert_association', { assoc });
    fileTypeAssociations.set(list ?? []);
    return { ok: true };
  } catch (e) {
    return { ok: false, error: typeof e === 'string' ? e : 'Failed to save.' };
  }
}

export async function removeFileTypeAssociation(id: string): Promise<void> {
  const list = await SystemBridge.invokeCommand<FileTypeAssociation[]>('file_type_remove_association', { id }).catch(() => null);
  if (list) fileTypeAssociations.set(list);
}

/**
 * Synchronous local resolution against the currently-loaded store —
 * used by `FileIcon.svelte` on every render, so it can't be an async
 * round-trip to the backend per icon. Mirrors the backend's own
 * most-specific-wins matching in `file_type_resolve` (see that
 * function's doc comment) so the two stay behaviorally identical; the
 * backend command exists for callers (like "open this file") that need
 * an authoritative answer even if the frontend store hasn't loaded yet.
 */
export function resolveAssociationLocally(filename: string, mimeType: string, list: FileTypeAssociation[]): FileTypeAssociation | null {
  const ext = filename.split('.').pop()?.toLowerCase() ?? '';
  const byExt = list.find((a) => a.kind === 'extension' && a.pattern.toLowerCase() === ext);
  if (byExt) return byExt;
  const byExactMime = list.find((a) => a.kind === 'mime' && a.pattern === mimeType);
  if (byExactMime) return byExactMime;
  const byWildcardMime = list.find((a) => a.kind === 'mime' && a.pattern.endsWith('/*') && mimeType.startsWith(a.pattern.slice(0, -1)));
  return byWildcardMime ?? null;
}

export function newAssociationId(): string {
  return `ftype-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}
