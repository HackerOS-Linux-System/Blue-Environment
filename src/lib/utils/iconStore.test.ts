import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('./systemBridge', () => ({
  SystemBridge: { invokeCommand: (...a: unknown[]) => invoke(...a), isTauri: () => true },
}));
vi.mock('./iconThemes', () => ({ iconThemes: { list: vi.fn().mockResolvedValue([]) }, ICON_THEMES_CHANGED: 'blue-icon-themes-changed' }));

import {
  primaryDownload, progressPercent, formatCount, formatSize, shortDate,
  loadThumbnail, _resetThumbnailCache, iconStore,
} from './iconStore';

const dl = (name: string, isArchive: boolean, i = 1) => ({ index: i, name, url: `https://x.org/${name}`, sizeKb: null, isArchive });

describe('primaryDownload', () => {
  it('prefers the first real archive, and is null without one', () => {
    expect(primaryDownload({ downloads: [dl('readme.pdf', false), dl('A.tar.xz', true, 2), dl('B.zip', true, 3)] })?.name).toBe('A.tar.xz');
    expect(primaryDownload({ downloads: [dl('readme.pdf', false)] })).toBeNull();
    expect(primaryDownload({ downloads: [] })).toBeNull();
  });
});

describe('progressPercent', () => {
  it('computes download percentage, clamped', () => {
    expect(progressPercent({ phase: 'download', received: 50, total: 200 })).toBe(25);
    expect(progressPercent({ phase: 'download', received: 999, total: 200 })).toBe(100);
  });
  it('is indeterminate while the size is unknown and full once installing', () => {
    expect(progressPercent({ phase: 'download', received: 50, total: null })).toBeNull();
    expect(progressPercent({ phase: 'install', received: 0, total: null })).toBe(100);
    expect(progressPercent(undefined)).toBeNull();
  });
});

describe('formatting', () => {
  it('formats counts, sizes and dates', () => {
    expect(formatCount(950)).toBe('950');
    expect(formatCount(1500)).toBe('1.5k');
    expect(formatCount(25_000)).toBe('25k');
    expect(formatCount(2_400_000)).toBe('2.4M');
    expect(formatSize(null)).toBe('');
    expect(formatSize(512)).toBe('512 KB');
    expect(formatSize(2048)).toBe('2.0 MB');
    expect(formatSize(20 * 1024)).toBe('20 MB');
    expect(shortDate('2024-05-01 10:00:00')).toBe('2024-05-01');
    expect(shortDate('yesterday')).toBe('yesterday');
  });
});

describe('loadThumbnail', () => {
  beforeEach(() => { invoke.mockReset(); _resetThumbnailCache(); });

  it('caches results and shares one request between concurrent callers', async () => {
    invoke.mockResolvedValue('data:image/png;base64,AAA');
    const [a, b] = await Promise.all([loadThumbnail('https://images.pling.com/a.png'), loadThumbnail('https://images.pling.com/a.png')]);
    expect(a).toBe(b);
    await loadThumbnail('https://images.pling.com/a.png');
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('never runs more than 4 fetches at once', async () => {
    let active = 0, peak = 0;
    invoke.mockImplementation(async () => {
      active++; peak = Math.max(peak, active);
      await new Promise((r) => setTimeout(r, 5));
      active--;
      return 'data:image/png;base64,AAA';
    });
    await Promise.all(Array.from({ length: 12 }, (_, i) => loadThumbnail(`https://images.pling.com/${i}.png`)));
    expect(peak).toBeLessThanOrEqual(4);
    expect(invoke).toHaveBeenCalledTimes(12);
  });

  it('does not cache failures, so a retry can succeed', async () => {
    invoke.mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce('data:image/png;base64,OK');
    await expect(loadThumbnail('https://images.pling.com/z.png')).rejects.toThrow('boom');
    expect(await loadThumbnail('https://images.pling.com/z.png')).toBe('data:image/png;base64,OK');
  });
});

describe('iconStore commands', () => {
  beforeEach(() => invoke.mockReset());

  it('passes search parameters to the backend', async () => {
    invoke.mockResolvedValue({ total: 0, page: 2, pageSize: 24, hasMore: false, items: [] });
    await iconStore.search('papirus', 'top', 2);
    expect(invoke).toHaveBeenCalledWith('icon_store_search', { query: 'papirus', sort: 'top', page: 2, pageSize: 24 });
  });

  it('installs through the backend with the content id and file', async () => {
    invoke.mockResolvedValue(['Papirus']);
    const names = await iconStore.install({ id: '42' }, { url: 'https://x.org/P.tar.xz', name: 'P.tar.xz' });
    expect(names).toEqual(['Papirus']);
    expect(invoke).toHaveBeenCalledWith('icon_store_install_remote', { contentId: '42', url: 'https://x.org/P.tar.xz', fileName: 'P.tar.xz' });
  });
});
