import { describe, it, expect } from 'vitest';
import {
  parseFlatpakList, parseContext, mergeContext, hasFilesystem, overrideArgs, portalSetArgs, parsePermissionShow,
  portalState, PORTAL_PERMISSIONS, SYSTEM_TOGGLES, isToggleOn, resetArgs, hasOverrides, emptyContext, isValidAppId,
} from './flatpakPermissions';

describe('flatpak list', () => {
  it('parses tab-separated rows, skips header/blank/garbage, sorts by name', () => {
    const out = 'Application\tName\tVersion\norg.mozilla.firefox\tFirefox\t131.0\n\ncom.spotify.Client\tSpotify\t1.2\nnot valid id\tX\t1\n';
    expect(parseFlatpakList(out)).toEqual([
      { id: 'org.mozilla.firefox', name: 'Firefox', version: '131.0' },
      { id: 'com.spotify.Client', name: 'Spotify', version: '1.2' },
    ]);
  });
});

describe('context keyfile', () => {
  const kf = `[Context]\nshared=network;ipc;\nsockets=x11;wayland;pulseaudio;\ndevices=dri;\nfilesystems=xdg-download;home:ro;\nfeatures=bluetooth;\n\n[Session Bus Policy]\norg.freedesktop.Notifications=talk\n`;
  it('parses only the [Context] group', () => {
    const c = parseContext(kf);
    expect(c.shared).toEqual(['network', 'ipc']);
    expect(c.sockets).toEqual(['x11', 'wayland', 'pulseaudio']);
    expect(c.filesystems).toEqual(['xdg-download', 'home:ro']);
    expect(c.features).toEqual(['bluetooth']);
  });
  it('merges overrides: add and !remove', () => {
    const base = parseContext(kf);
    const ov = parseContext('[Context]\nshared=!network;\ndevices=all;\n');
    const m = mergeContext(base, ov);
    expect(m.shared).toEqual(['ipc']);
    expect(m.devices).toEqual(['dri', 'all']);
  });
  it('recognises filesystem access with :ro/:rw suffix', () => {
    expect(hasFilesystem(['home:ro'], 'home')).toBe(true);
    expect(hasFilesystem(['homework'], 'home')).toBe(false);
  });
  it('evaluates toggles on the merged context', () => {
    const c = parseContext(kf);
    const get = (id: string) => SYSTEM_TOGGLES.find((t) => t.id === id)!;
    expect(isToggleOn(c, get('network'))).toBe(true);
    expect(isToggleOn(c, get('fs-home'))).toBe(true);
    expect(isToggleOn(c, get('fs-host'))).toBe(false);
    expect(isToggleOn(c, get('all-devices'))).toBe(false);
  });
});

describe('argument builders', () => {
  const t = (id: string) => SYSTEM_TOGGLES.find((x) => x.id === id)!;
  it('builds override args for each kind', () => {
    expect(overrideArgs('org.a.B', t('network'), false)).toEqual(['override', '--user', '--unshare=network', 'org.a.B']);
    expect(overrideArgs('org.a.B', t('x11'), true)).toEqual(['override', '--user', '--socket=x11', 'org.a.B']);
    expect(overrideArgs('org.a.B', t('all-devices'), false)).toEqual(['override', '--user', '--nodevice=all', 'org.a.B']);
    expect(overrideArgs('org.a.B', t('bluetooth'), true)[2]).toBe('--allow=bluetooth');
    expect(overrideArgs('org.a.B', t('fs-home'), false)[2]).toBe('--nofilesystem=home');
  });
  it('rejects ids that could smuggle options', () => {
    expect(isValidAppId('--system')).toBe(false);
    expect(isValidAppId('a b.c')).toBe(false);
    expect(() => overrideArgs('--system', t('network'), true)).toThrow();
    expect(() => portalSetArgs('x;rm', PORTAL_PERMISSIONS[0], true)).toThrow();
    expect(() => resetArgs('nope')).toThrow();
  });
  it('builds portal set args', () => {
    const cam = PORTAL_PERMISSIONS.find((p) => p.id === 'camera')!;
    expect(portalSetArgs('org.a.B', cam, true)).toEqual(['permission-set', 'devices', 'camera', 'org.a.B', 'yes']);
    expect(portalSetArgs('org.a.B', cam, false)[4]).toBe('no');
    const loc = PORTAL_PERMISSIONS.find((p) => p.id === 'location')!;
    expect(portalSetArgs('org.a.B', loc, true)[4]).toBe('EXACT');
  });
  it('reset runs permission-reset and override --reset', () => {
    expect(resetArgs('org.a.B')).toEqual([['permission-reset', 'org.a.B'], ['override', '--user', '--reset', 'org.a.B']]);
  });
});

describe('permission-show', () => {
  const out = `Table          Object        App              Permissions  Data\ndevices        camera        org.a.B          yes          [<'']>\ndevices        microphone    org.a.B          no           []\nlocation       location      org.a.B          EXACT        0\nnotifications  notification  org.a.B          ask          []\n`;
  it('maps rows to granted/denied/unset', () => {
    const m = parsePermissionShow(out);
    const by = (id: string) => portalState(m, PORTAL_PERMISSIONS.find((p) => p.id === id)!);
    expect(by('camera')).toBe('granted');
    expect(by('microphone')).toBe('denied');
    expect(by('location')).toBe('granted');
    expect(by('notifications')).toBe('unset');
    expect(by('screenshot')).toBe('unset');
  });
  it('tolerates empty output', () => {
    expect(parsePermissionShow('')).toEqual({});
  });
});

describe('overrides', () => {
  it('detects when anything is overridden', () => {
    expect(hasOverrides(emptyContext())).toBe(false);
    expect(hasOverrides(parseContext('[Context]\nshared=!network;'))).toBe(true);
  });
});
