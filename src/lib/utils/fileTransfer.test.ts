import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { get } from 'svelte/store';
import { conflictRequest } from '../stores/conflictDialog';
import { transferWithDialog, createWithDialog, describeSummary } from './fileTransfer';

/** Czeka, aż pojawi się pytanie, i odpowiada na nie. */
async function answerNext(answer: unknown) {
  for (let i = 0; i < 50 && !get(conflictRequest); i++) await new Promise((r) => setTimeout(r, 1));
  const req = get(conflictRequest)!;
  conflictRequest.set(null);
  (req.resolve as (v: unknown) => void)(answer);
}

describe('transferWithDialog', () => {
  beforeEach(() => { invoke.mockReset(); conflictRequest.set(null); });

  it('does not ask and transfers straight away when there are no conflicts', async () => {
    invoke.mockResolvedValueOnce([]).mockResolvedValueOnce({ done: 2, skipped: 0, errors: [] });
    const r = await transferWithDialog(['/a', '/b'], '/dst', 'copy');
    expect(r?.done).toBe(2);
    expect(invoke).toHaveBeenLastCalledWith('fm_transfer', expect.objectContaining({ sources: ['/a', '/b'], destDir: '/dst', mode: 'copy', decisions: {} }));
  });

  it('passes the user decisions to the backend', async () => {
    const c = { src: '/a/x', name: 'x', dest: '/dst/x', src_is_dir: true, dest_is_dir: true, src_size: 1, dest_size: 2, src_modified: 1, dest_modified: 2, same_location: false };
    invoke.mockResolvedValueOnce([c]).mockResolvedValueOnce({ done: 0, skipped: 0, errors: [] });
    const p = transferWithDialog(['/a/x'], '/dst', 'move');
    await answerNext({ '/a/x': { policy: 'merge', inner: 'skip' } });
    await p;
    expect(invoke).toHaveBeenLastCalledWith('fm_transfer', expect.objectContaining({ decisions: { '/a/x': { policy: 'merge', inner: 'skip' } } }));
  });

  it('cancel means nothing is transferred', async () => {
    invoke.mockResolvedValueOnce([{ src: '/a/x', name: 'x', dest: '/d/x', src_is_dir: false, dest_is_dir: false, src_size: 1, dest_size: 1, src_modified: null, dest_modified: null, same_location: false }]);
    const p = transferWithDialog(['/a/x'], '/d', 'copy');
    await answerNext(null);
    expect(await p).toBeNull();
    expect(invoke).toHaveBeenCalledTimes(1); // tylko fm_check_conflicts
  });
});

describe('createWithDialog', () => {
  beforeEach(() => { invoke.mockReset(); conflictRequest.set(null); });

  it('asks when the name is taken and creates a second copy on "keep both"', async () => {
    invoke.mockRejectedValueOnce('EXISTS:folder').mockResolvedValueOnce('/d/Nowy (2)');
    const p = createWithDialog('/d', 'Nowy', 'folder');
    await answerNext('keep_both');
    expect(await p).toBe('/d/Nowy (2)');
    expect(invoke).toHaveBeenLastCalledWith('fm_create', expect.objectContaining({ policy: 'keep_both' }));
  });

  it('cancel creates nothing', async () => {
    invoke.mockRejectedValueOnce('EXISTS:file');
    const p = createWithDialog('/d', 'a.txt', 'file');
    await answerNext('cancel');
    expect(await p).toBeNull();
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('rethrows unrelated errors', async () => {
    invoke.mockRejectedValueOnce('Permission denied');
    await expect(createWithDialog('/d', 'a', 'file')).rejects.toBe('Permission denied');
  });
});

describe('describeSummary', () => {
  it('reports errors, skips and success', () => {
    expect(describeSummary({ done: 1, skipped: 0, errors: ['x: boom'] }, 'copy').type).toBe('error');
    expect(describeSummary({ done: 0, skipped: 2, errors: [] }, 'move').type).toBe('info');
    expect(describeSummary({ done: 3, skipped: 1, errors: [] }, 'move').message).toContain('3');
    expect(describeSummary({ done: 1, skipped: 0, errors: [], cancelled: true }, 'copy').type).toBe('info');
  });
});
