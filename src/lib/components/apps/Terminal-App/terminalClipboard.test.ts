import { describe, it, expect } from 'vitest';
import { classifyKey, sanitizePaste, type KeyLike } from './terminalClipboard';

const k = (o: Partial<KeyLike> & { key: string }): KeyLike => ({ type: 'keydown', ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...o });

describe('terminal clipboard shortcuts', () => {
  it('Ctrl+Shift+C copies, Ctrl+Shift+V pastes (letters arrive upper-case with Shift)', () => {
    expect(classifyKey(k({ key: 'C', code: 'KeyC', ctrlKey: true, shiftKey: true }), false)).toBe('copy');
    expect(classifyKey(k({ key: 'V', code: 'KeyV', ctrlKey: true, shiftKey: true }), false)).toBe('paste');
  });
  it('works on non-QWERTY layouts via key when code differs', () => {
    expect(classifyKey(k({ key: 'c', code: 'KeyJ', ctrlKey: true, shiftKey: true }), false)).toBe('copy');
  });
  it('Ctrl+Insert copies, Shift+Insert pastes', () => {
    expect(classifyKey(k({ key: 'Insert', ctrlKey: true }), false)).toBe('copy');
    expect(classifyKey(k({ key: 'Insert', shiftKey: true }), false)).toBe('paste');
  });
  it('plain Ctrl+C is SIGINT unless something is selected', () => {
    expect(classifyKey(k({ key: 'c', code: 'KeyC', ctrlKey: true }), false)).toBeNull();
    expect(classifyKey(k({ key: 'c', code: 'KeyC', ctrlKey: true }), true)).toBe('copy-clear');
  });
  it('plain Ctrl+V is left to the shell (quoted-insert / vim)', () => {
    expect(classifyKey(k({ key: 'v', code: 'KeyV', ctrlKey: true }), true)).toBeNull();
  });
  it('ignores keyup, Alt and Meta combos, other keys', () => {
    expect(classifyKey(k({ type: 'keyup', key: 'C', code: 'KeyC', ctrlKey: true, shiftKey: true }), true)).toBeNull();
    expect(classifyKey(k({ key: 'C', code: 'KeyC', ctrlKey: true, shiftKey: true, altKey: true }), true)).toBeNull();
    expect(classifyKey(k({ key: 'c', code: 'KeyC', metaKey: true }), true)).toBeNull();
    expect(classifyKey(k({ key: 'a', code: 'KeyA', ctrlKey: true, shiftKey: true }), true)).toBeNull();
  });
  it('sanitizePaste strips NULs only', () => {
    expect(sanitizePaste('a\0b\nc')).toBe('ab\nc');
  });
});
