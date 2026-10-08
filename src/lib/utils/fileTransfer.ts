import { invoke } from '@tauri-apps/api/core';
import { dialogPrompt } from '../stores/dialog';
import { askTransferConflicts, askNameExists, type FmConflict, type FmDecision } from '../stores/conflictDialog';
import { newJobId, startJob, finishJob } from '../stores/transferJobs';
import { translate } from '../stores/language';

export interface TransferSummary { done: number; skipped: number; errors: string[]; cancelled?: boolean }

/** Kopiuje/przenosi `sources` do `destDir`, pytając o każdy konflikt. `null` = anulowano. */
export async function transferWithDialog(sources: string[], destDir: string, mode: 'copy' | 'move'): Promise<TransferSummary | null> {
  const conflicts = await invoke<FmConflict[]>('fm_check_conflicts', { sources, destDir, mode });
  let decisions: Record<string, FmDecision> = {};
  if (conflicts.length) {
    const answer = await askTransferConflicts(mode, conflicts);
    if (!answer) return null;                 // Anuluj — nic nie robimy
    decisions = answer;
  }
  // Zadanie dostaje id; okno postępu (TransferProgress) pokazuje się, gdy trwa dłużej niż chwilę.
  const jobId = newJobId();
  startJob(jobId, mode);
  try {
    return await invoke<TransferSummary>('fm_transfer', { sources, destDir, mode, decisions, jobId });
  } finally {
    finishJob(jobId);
  }
}

export function describeSummary(s: TransferSummary, mode: 'copy' | 'move'): { type: 'success' | 'error' | 'info'; message: string } {
  if (s.cancelled) return { type: 'info', message: translate('fm.cancelled', { n: s.done }) };
  if (s.errors.length) return { type: 'error', message: translate('fm.failed', { n: s.errors.length, e: s.errors[0] }) };
  if (!s.done && s.skipped) return { type: 'info', message: translate('fm.skipped_only', { n: s.skipped }) };
  const base = translate(mode === 'copy' ? 'fm.copied' : 'fm.moved', { n: s.done });
  return { type: 'success', message: s.skipped ? `${base}${translate('fm.skipped_suffix', { n: s.skipped })}` : base };
}

const existsKind = (e: unknown): 'file' | 'folder' | null => {
  const m = String(e).match(/EXISTS:(file|folder)/);
  return m ? (m[1] as 'file' | 'folder') : null;
};

/**
 * Tworzy plik/folder w `dir`. Zajęta nazwa → dialog (Zastąp / Zachowaj oba /
 * Zmień nazwę / Anuluj). Zwraca ścieżkę utworzonego elementu albo null (anulowano).
 */
export async function createWithDialog(dir: string, initialName: string, kind: 'file' | 'folder', content = ''): Promise<string | null> {
  let name = initialName.trim();
  for (let guard = 0; guard < 20; guard++) {
    try {
      return await invoke<string>('fm_create', { dir, name, kind, content });
    } catch (e) {
      const existing = existsKind(e);
      if (!existing) throw e;
      const ans = await askNameExists(name, existing, { canReplace: existing === kind || (existing === 'folder' && kind === 'folder') });
      if (ans === 'cancel') return null;
      if (ans === 'rename') {
        const n = await dialogPrompt({ title: 'Nowa nazwa', defaultValue: name, confirmLabel: 'OK' });
        if (!n?.trim()) return null;
        name = n.trim();
        continue;
      }
      return await invoke<string>('fm_create', { dir, name, kind, content, policy: ans === 'replace' ? 'replace' : 'keep_both' });
    }
  }
  return null;
}

/** Zmiana nazwy z pytaniem, gdy cel istnieje. Zwraca nową ścieżkę albo null (anulowano). */
export async function renameWithDialog(path: string, newName: string, isDir: boolean): Promise<string | null> {
  let name = newName.trim();
  for (let guard = 0; guard < 20; guard++) {
    try {
      return await invoke<string>('fm_rename', { path, newName: name, replaceExisting: false });
    } catch (e) {
      const existing = existsKind(e);
      if (!existing) throw e;
      const sameKind = (existing === 'folder') === isDir;
      const ans = await askNameExists(name, existing, { canReplace: sameKind, canKeepBoth: false });
      if (ans === 'cancel') return null;
      if (ans === 'rename') {
        const n = await dialogPrompt({ title: 'Nowa nazwa', defaultValue: name, confirmLabel: 'OK' });
        if (!n?.trim()) return null;
        name = n.trim();
        continue;
      }
      return await invoke<string>('fm_rename', { path, newName: name, replaceExisting: true });
    }
  }
  return null;
}
