import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
import { get } from 'svelte/store';
import { transferJobs, startJob, updateJob, finishJob, fmtBytes, fmtEta, percent, SHOW_AFTER_MS } from './transferJobs';

describe('transfer jobs store', () => {
  beforeEach(() => { vi.useFakeTimers(); transferJobs.set([]); });
  afterEach(() => { vi.useRealTimers(); });

  it('shows the dialog only after a delay and removes it on finish', () => {
    startJob('a', 'copy');
    expect(get(transferJobs)[0].visible).toBe(false);
    vi.advanceTimersByTime(SHOW_AFTER_MS + 10);
    expect(get(transferJobs)[0].visible).toBe(true);
    finishJob('a');
    expect(get(transferJobs)).toHaveLength(0);
  });

  it('a quick job never becomes visible', () => {
    startJob('q', 'move');
    finishJob('q');
    vi.advanceTimersByTime(SHOW_AFTER_MS + 10);
    expect(get(transferJobs)).toHaveLength(0);
  });

  it('applies progress events to the right job only', () => {
    startJob('a', 'copy'); startJob('b', 'move');
    updateJob({ jobId: 'b', mode: 'move', doneBytes: 5, totalBytes: 10, doneFiles: 1, totalFiles: 2, current: 'x', bytesPerSec: 1, etaSecs: 5, paused: false });
    const [a, b] = get(transferJobs);
    expect(a.doneBytes).toBe(0);
    expect(b.doneBytes).toBe(5);
  });
});

describe('formatting', () => {
  it('formats sizes, eta and percent', () => {
    expect(fmtBytes(512)).toBe('512 B');
    expect(fmtBytes(2_500_000)).toBe('2.5 MB');
    expect(fmtBytes(3_000_000_000)).toBe('3.0 GB');
    expect(fmtEta(75)).toBe('1:15');
    expect(fmtEta(3725)).toBe('1:02:05');
    expect(fmtEta(null)).toBe('—');
    expect(percent({ doneBytes: 50, totalBytes: 200, doneFiles: 0, totalFiles: 0 })).toBe(25);
    expect(percent({ doneBytes: 0, totalBytes: 0, doneFiles: 1, totalFiles: 4 })).toBe(25);
    expect(percent({ doneBytes: 999, totalBytes: 10, doneFiles: 0, totalFiles: 0 })).toBe(100);
  });
});
