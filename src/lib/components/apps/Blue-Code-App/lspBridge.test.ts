import { describe, it, expect } from 'vitest';
import { lspKindToMonaco, toProblems, BRIDGED_LANGS } from './lspBridge';

describe('lsp bridge helpers', () => {
  it('maps LSP completion kinds to Monaco names', () => {
    expect(lspKindToMonaco(3)).toBe('Function');
    expect(lspKindToMonaco(22)).toBe('Struct');
    expect(lspKindToMonaco(15)).toBe('Snippet');
    expect(lspKindToMonaco(undefined)).toBe('Text');
    expect(lspKindToMonaco(999)).toBe('Text');
  });
  it('converts 0-based diagnostics to 1-based problems', () => {
    const p = toProblems('/a.rs', [
      { message: 'oops', severity: 1, line: 0, character: 4, endLine: 0, endCharacter: 9, source: 'rustc', code: 'E0308' },
      { message: 'hm', severity: 2, line: 9, character: 0, endLine: 9, endCharacter: 1 },
    ]);
    expect(p[0]).toMatchObject({ file: '/a.rs', line: 1, col: 5, severity: 'error' });
    expect(p[0].message).toContain('rustc: E0308');
    expect(p[1]).toMatchObject({ line: 10, col: 1, severity: 'warning', message: 'hm' });
  });
  it('does not bridge languages Monaco already handles', () => {
    expect(BRIDGED_LANGS).not.toContain('typescript');
    expect(BRIDGED_LANGS).toContain('rust');
  });
});
