import { describe, it, expect } from 'vitest';
import { FocusMemory, pickRestoreIndex } from './focusMemory';

describe('FocusMemory', () => {
  it('remembers and forgets per scope', () => {
    const m = new FocusMemory();
    expect(m.recall('home')).toBeUndefined();
    m.remember('home', 'app:files', 3);
    m.remember('app:1', null, 0);
    expect(m.recall('home')).toEqual({ key: 'app:files', index: 3 });
    expect(m.recall('app:1')).toEqual({ key: null, index: 0 });
    m.forget('home');
    expect(m.recall('home')).toBeUndefined();
  });
});

describe('pickRestoreIndex', () => {
  const keys = ['app:a', 'app:b', 'app:c', null];
  it('prefers the key, even if the list was reordered', () => {
    expect(pickRestoreIndex(['app:c', 'app:a', 'app:b'], { key: 'app:b', index: 1 })).toBe(2);
  });
  it('falls back to the clamped position when the key is gone or absent', () => {
    expect(pickRestoreIndex(keys, { key: 'app:zzz', index: 2 })).toBe(2);
    expect(pickRestoreIndex(keys, { key: null, index: 99 })).toBe(3);
    expect(pickRestoreIndex(keys, { key: null, index: -4 })).toBe(0);
  });
  it('starts at the first element with nothing saved, and returns -1 for an empty screen', () => {
    expect(pickRestoreIndex(keys, undefined)).toBe(0);
    expect(pickRestoreIndex([], { key: 'x', index: 0 })).toBe(-1);
    expect(pickRestoreIndex([], undefined)).toBe(-1);
  });
});
