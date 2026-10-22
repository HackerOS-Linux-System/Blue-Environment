import { describe, it, expect } from 'vitest';
import { lspKindToCompletionType, lspSnippetToCm, toProblems, BRIDGED_LANGS } from './lspBridge';

describe('lsp bridge helpers', () => {
  it('maps LSP completion kinds to CodeMirror completion types', () => {
    expect(lspKindToCompletionType(3)).toBe('function');
    expect(lspKindToCompletionType(22)).toBe('class');
    expect(lspKindToCompletionType(14)).toBe('keyword');
    expect(lspKindToCompletionType(undefined)).toBe('text');
    expect(lspKindToCompletionType(999)).toBe('text');
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
  it('bridges TypeScript/JavaScript now that Monaco\'s built-in service is gone', () => {
    expect(BRIDGED_LANGS).toContain('typescript');
    expect(BRIDGED_LANGS).toContain('javascript');
    expect(BRIDGED_LANGS).toContain('rust');
  });
  it('converts LSP snippets to CodeMirror templates', () => {
    expect(lspSnippetToCm('foo($1, $2)$0')).toBe('foo(${1}, ${2})${0}');
    expect(lspSnippetToCm('${1|a,b,c|}')).toBe('${1:a}');
    expect(lspSnippetToCm('for ${1:i} in ${2:x}')).toBe('for ${1:i} in ${2:x}');
  });
});
