import { describe, it, expect, vi } from 'vitest';

// `classifyFile` is pure; the module's other imports pull in the whole app registry
// (Svelte components, icons), which a node test neither needs nor can load.
vi.mock('../stores/windowManager', () => ({ openApp: vi.fn() }));
vi.mock('./systemBridge', () => ({ SystemBridge: {}, shellQuote: (s: string) => s }));
vi.mock('./configStore', () => ({ configStore: { subscribe: () => () => {} } }));

import { classifyFile } from './openFile';

describe('classifyFile — new in-shell handlers', () => {
  const c = (name: string, mime?: string) => classifyFile({ path: `/x/${name}`, name, mime_type: mime });
  it('sends PDFs to Blue PDF', () => {
    expect(c('report.pdf')).toBe('pdf');
    expect(c('REPORT.PDF')).toBe('pdf');
    expect(c('noext', 'application/pdf')).toBe('pdf');
  });
  it('sends disk images to Blue Media Writer, even compressed', () => {
    expect(c('fedora.iso')).toBe('diskimage');
    expect(c('raspios.img.xz')).toBe('diskimage');
    expect(c('disk.RAW.gz')).toBe('diskimage');
  });
  it('still treats ordinary archives as archives', () => {
    expect(c('photos.zip')).toBe('archive');
    expect(c('backup.tar.gz')).toBe('archive');
    expect(c('notes.txt.xz')).toBe('archive');
  });
});
