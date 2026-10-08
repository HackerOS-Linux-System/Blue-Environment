import { writable } from 'svelte/store';

/** Konflikt zwrócony przez backend (`fm_check_conflicts`). */
export interface FmConflict {
  src: string; name: string; dest: string;
  src_is_dir: boolean; dest_is_dir: boolean;
  src_size: number; dest_size: number;
  src_modified: number | null; dest_modified: number | null;
  same_location: boolean;
}
export type FmPolicy = 'replace' | 'skip' | 'keep_both' | 'merge';
export interface FmDecision { policy: FmPolicy; inner?: FmPolicy }

/** Pytanie o kopiowanie/przenoszenie — iteruje po konfliktach. */
interface TransferRequest {
  kind: 'transfer';
  mode: 'copy' | 'move';
  conflicts: FmConflict[];
  resolve: (v: Record<string, FmDecision> | null) => void;
}
/** Pytanie „ta nazwa jest zajęta" przy tworzeniu / zmianie nazwy. */
export type ExistsAnswer = 'replace' | 'keep_both' | 'rename' | 'cancel';
interface ExistsRequest {
  kind: 'exists';
  name: string;
  existing: 'file' | 'folder';
  /** czy wolno zastąpić (nie można zastąpić folderu plikiem ani odwrotnie) */
  canReplace: boolean;
  /** czy oferować „zachowaj oba" (przy zmianie nazwy: nie) */
  canKeepBoth: boolean;
  resolve: (v: ExistsAnswer) => void;
}
export type ConflictRequest = TransferRequest | ExistsRequest;

export const conflictRequest = writable<ConflictRequest | null>(null);

export function askTransferConflicts(mode: 'copy' | 'move', conflicts: FmConflict[]) {
  return new Promise<Record<string, FmDecision> | null>((resolve) =>
    conflictRequest.set({ kind: 'transfer', mode, conflicts, resolve }));
}

export function askNameExists(name: string, existing: 'file' | 'folder', opts: { canReplace?: boolean; canKeepBoth?: boolean } = {}) {
  return new Promise<ExistsAnswer>((resolve) =>
    conflictRequest.set({ kind: 'exists', name, existing, canReplace: opts.canReplace ?? true, canKeepBoth: opts.canKeepBoth ?? true, resolve }));
}
