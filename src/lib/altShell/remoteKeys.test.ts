import { describe, it, expect } from 'vitest';
import { remoteAction } from './remoteKeys';

describe('remoteAction', () => {
  it('maps the common remote / CEC key names', () => {
    expect(remoteAction({ key: 'BrowserBack' })).toBe('back');
    expect(remoteAction({ key: 'XF86Back' })).toBe('back');
    expect(remoteAction({ key: 'Exit' })).toBe('back');
    expect(remoteAction({ key: 'BrowserHome' })).toBe('home');
    expect(remoteAction({ key: 'MediaTopMenu' })).toBe('menu');
    expect(remoteAction({ key: 'Guide' })).toBe('menu');
    expect(remoteAction({ key: 'MediaPlayPause' })).toBe('play_pause');
    expect(remoteAction({ key: 'MediaTrackNext' })).toBe('next');
    expect(remoteAction({ key: 'MediaStop' })).toBe('stop');
  });
  it('maps the legacy smart-TV key codes', () => {
    expect(remoteAction({ keyCode: 461 })).toBe('back');
    expect(remoteAction({ keyCode: 10009 })).toBe('back');
    expect(remoteAction({ keyCode: 415 })).toBe('play_pause');
  });
  it('ignores ordinary keys', () => {
    expect(remoteAction({ key: 'a', keyCode: 65 })).toBeNull();
    expect(remoteAction({ key: 'ArrowUp', keyCode: 38 })).toBeNull();
    expect(remoteAction({ key: 'Enter', keyCode: 13 })).toBeNull();
    expect(remoteAction({})).toBeNull();
  });
});
