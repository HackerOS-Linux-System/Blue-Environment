import { describe, it, expect } from 'vitest';
import { parseProfiles, addProfile, removeProfile, defaultState, activeAuthor } from './docsProfile';

describe('docs author profiles', () => {
  it('defaults to the OS user', () => {
    const s = parseProfiles(null, 'ann');
    expect(s.profiles).toHaveLength(1);
    expect(activeAuthor(s).name).toBe('ann');
  });
  it('repairs damaged files', () => {
    expect(parseProfiles('{oops', 'bob').profiles[0].name).toBe('bob');
    expect(parseProfiles(JSON.stringify({ profiles: [{ id: 1 }], activeId: 'x' }), 'bob').profiles[0].name).toBe('bob');
  });
  it('keeps valid profiles and a valid active id', () => {
    const raw = JSON.stringify({ profiles: [{ id: 'a', name: 'A', color: '#111' }, { id: 'b', name: 'B' }], activeId: 'zzz' });
    const s = parseProfiles(raw, 'u');
    expect(s.profiles.map((p) => p.id)).toEqual(['a', 'b']);
    expect(s.activeId).toBe('a');
  });
  it('adds (unique, case-insensitive) and switches to the new profile', () => {
    let s = defaultState('ann');
    s = addProfile(s, 'Reviewer');
    expect(s.profiles).toHaveLength(2);
    expect(activeAuthor(s).name).toBe('Reviewer');
    expect(addProfile(s, 'reviewer')).toBe(s);
    expect(addProfile(s, '   ')).toBe(s);
  });
  it('never removes the last profile; re-points the active one', () => {
    let s = addProfile(defaultState('ann'), 'Bob');
    const bobId = s.activeId;
    s = removeProfile(s, bobId);
    expect(s.profiles).toHaveLength(1);
    expect(s.activeId).toBe('p-default');
    expect(removeProfile(s, 'p-default')).toBe(s);
  });
});
