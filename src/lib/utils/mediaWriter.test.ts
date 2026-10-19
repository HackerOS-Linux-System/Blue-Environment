import { describe, it, expect } from 'vitest';
import { formatBytes, formatEta, percent, driveTitle, validateImageUrl, isSha256, canStartWrite, formatSpeed, type DriveInfo } from './mediaWriter';

const drive = (over: Partial<DriveInfo> = {}): DriveInfo => ({ path: '/dev/sdb', model: 'Ultra', vendor: 'SanDisk', sizeBytes: 16e9, transport: 'usb', mounted: [], labels: [], ...over });

describe('mediaWriter helpers', () => {
  it('formats sizes and speeds', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1536)).toBe('1.5 KiB');
    expect(formatBytes(4 * 1024 ** 3)).toBe('4.0 GiB');
    expect(formatBytes(-1)).toBe('–');
    expect(formatSpeed(0)).toBe('');
    expect(formatSpeed(10 * 1024 * 1024)).toBe('10.0 MiB/s');
  });

  it('estimates time left', () => {
    expect(formatEta({ phase: 'write', done: 0, total: 600, speed: 10 })).toBe('1:00');
    expect(formatEta({ phase: 'write', done: 0, total: 36000, speed: 10 })).toBe('1:00:00');
    expect(formatEta({ phase: 'write', done: 5, total: null, speed: 10 })).toBe('');
    expect(formatEta({ phase: 'write', done: 10, total: 10, speed: 10 })).toBe('');
  });

  it('percent is null when the total is unknown and clamped otherwise', () => {
    expect(percent({ phase: 'write', done: 5, total: null, speed: 0 })).toBeNull();
    expect(percent({ phase: 'write', done: 50, total: 200, speed: 0 })).toBe(25);
    expect(percent({ phase: 'write', done: 500, total: 200, speed: 0 })).toBe(100);
  });

  it('titles drives and falls back to the device path', () => {
    expect(driveTitle(drive())).toBe('SanDisk Ultra');
    expect(driveTitle(drive({ vendor: '', model: '' }))).toBe('/dev/sdb');
  });

  it('only accepts https URLs', () => {
    expect(validateImageUrl('')).toBe('empty');
    expect(validateImageUrl('http://x.org/a.iso')).toBe('not_https');
    expect(validateImageUrl('ftp://x.org/a.iso')).toBe('not_https');
    expect(validateImageUrl('not a url')).toBe('invalid');
    expect(validateImageUrl(' https://x.org/a.iso ')).toBe('ok');
  });

  it('recognises sha256 digests, also in sha256sum format', () => {
    const h = 'a'.repeat(64);
    expect(isSha256(h)).toBe(true);
    expect(isSha256(`${h}  file.iso`)).toBe(true);
    expect(isSha256(`sha256:${h}`)).toBe(true);
    expect(isSha256('abc')).toBe(false);
  });

  it('gates the write button', () => {
    const d = drive();
    expect(canStartWrite({ image: null, drive: d, imageBytes: 1, confirmed: true })).toEqual({ ok: false, reason: 'no_image' });
    expect(canStartWrite({ image: '/a.iso', drive: null, imageBytes: 1, confirmed: true })).toEqual({ ok: false, reason: 'no_drive' });
    expect(canStartWrite({ image: '/a.iso', drive: d, imageBytes: 32e9, confirmed: true })).toEqual({ ok: false, reason: 'too_big' });
    expect(canStartWrite({ image: '/a.iso', drive: d, imageBytes: 1e9, confirmed: false })).toEqual({ ok: false, reason: 'unconfirmed' });
    expect(canStartWrite({ image: '/a.iso', drive: d, imageBytes: null, confirmed: true })).toEqual({ ok: true });
  });
});
