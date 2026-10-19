import { describe, it, expect } from 'vitest';
import { fitRect, mapPointToDevice, classifyGesture, wheelToSwipe, keyFromEvent, coalesce, filterApps, formatKb, parentRemote, joinRemote, validHostPort } from './phoneView';

describe('phone screen mapping', () => {
  it('letterboxes a portrait phone inside a wide box', () => {
    const r = fitRect(1000, 500, 0.5); // 1080x2160 phone
    expect(r.h).toBe(500);
    expect(r.w).toBe(250);
    expect(r.x).toBe(375);
  });

  it('maps clicks to device pixels and ignores the bars', () => {
    const box = { left: 100, top: 50, width: 1000, height: 500 };
    // centre of the box = centre of the phone
    expect(mapPointToDevice(600, 300, box, 1080, 2160)).toEqual({ x: 540, y: 1080 });
    // top-left of the image is x=100+375, y=50
    expect(mapPointToDevice(475, 50, box, 1080, 2160)).toEqual({ x: 0, y: 0 });
    // left bar
    expect(mapPointToDevice(150, 300, box, 1080, 2160)).toBeNull();
    // bottom-right corner stays inside the screen
    expect(mapPointToDevice(725, 550, box, 1080, 2160)).toEqual({ x: 1079, y: 2159 });
    expect(mapPointToDevice(1, 1, box, 0, 0)).toBeNull();
  });
});

describe('gestures', () => {
  it('tap / long press / swipe', () => {
    expect(classifyGesture({ x: 10, y: 10 }, { x: 12, y: 11 }, 120)).toEqual({ kind: 'tap', x: 10, y: 10 });
    expect(classifyGesture({ x: 10, y: 10 }, { x: 10, y: 10 }, 700)).toEqual({ kind: 'longPress', x: 10, y: 10 });
    expect(classifyGesture({ x: 0, y: 500 }, { x: 0, y: 100 }, 20)).toEqual({ kind: 'swipe', x1: 0, y1: 500, x2: 0, y2: 100, ms: 80 });
    expect((classifyGesture({ x: 0, y: 0 }, { x: 300, y: 0 }, 99999) as any).ms).toBe(1200);
  });

  it('wheel scrolls the content in the natural direction and stays on screen', () => {
    const down = wheelToSwipe(100, { x: 500, y: 1000 }, 2000)!;
    expect(down.kind).toBe('swipe');
    expect((down as any).y2).toBeLessThan(1000); // wheel down → finger up
    const up = wheelToSwipe(-100, { x: 500, y: 1900 }, 2000)! as any;
    expect(up.y2).toBeLessThanOrEqual(1999);
    expect(wheelToSwipe(0, { x: 1, y: 1 }, 2000)).toBeNull();
    const huge = wheelToSwipe(99999, { x: 5, y: 100 }, 2000)! as any;
    expect(huge.y2).toBeGreaterThanOrEqual(1);
  });
});

describe('keyboard mapping', () => {
  it('maps navigation keys and printable ASCII', () => {
    expect(keyFromEvent({ key: 'Enter' })).toEqual({ kind: 'key', name: 'enter' });
    expect(keyFromEvent({ key: 'Backspace' })).toEqual({ kind: 'key', name: 'delete' });
    expect(keyFromEvent({ key: 'Escape' })).toEqual({ kind: 'key', name: 'back' });
    expect(keyFromEvent({ key: 'a' })).toEqual({ kind: 'text', text: 'a' });
    expect(keyFromEvent({ key: ' ' })).toEqual({ kind: 'text', text: ' ' });
  });
  it('ignores shortcuts, dead keys and non-ASCII', () => {
    expect(keyFromEvent({ key: 'c', ctrlKey: true })).toBeNull();
    expect(keyFromEvent({ key: 'Shift' })).toBeNull();
    expect(keyFromEvent({ key: 'ż' })).toBeNull();
    expect(keyFromEvent({ key: 'Dead' })).toBeNull();
  });
  it('coalesces typed characters', () => {
    expect(coalesce([{ kind: 'text', text: 'h' }, { kind: 'text', text: 'i' }, { kind: 'key', name: 'enter' }, { kind: 'text', text: 'x' }])).toEqual([
      { kind: 'text', text: 'hi' }, { kind: 'key', name: 'enter' }, { kind: 'text', text: 'x' },
    ]);
  });
});

describe('misc helpers', () => {
  it('filters and sorts apps', () => {
    const apps = [{ package: 'com.b.zed', label: 'Zed' }, { package: 'com.a.alpha', label: 'Alpha' }];
    expect(filterApps(apps, '').map((a) => a.label)).toEqual(['Alpha', 'Zed']);
    expect(filterApps(apps, 'com.b').map((a) => a.label)).toEqual(['Zed']);
    expect(filterApps(apps, 'ALP')).toHaveLength(1);
  });
  it('formats storage and remote paths', () => {
    expect(formatKb(0)).toBe('–');
    expect(formatKb(113000000)).toBe('108 GB');
    expect(formatKb(500 * 1024)).toBe('500 MB');
    expect(parentRemote('/sdcard/DCIM/Camera')).toBe('/sdcard/DCIM');
    expect(parentRemote('/sdcard')).toBe('/');
    expect(joinRemote('/sdcard/', 'a b.txt')).toBe('/sdcard/a b.txt');
  });
  it('validates host:port', () => {
    expect(validHostPort('192.168.1.2:5555')).toBe(true);
    expect(validHostPort('phone.local:37000')).toBe(true);
    expect(validHostPort('x:0')).toBe(false);
    expect(validHostPort('x:70000')).toBe(false);
    expect(validHostPort('nope')).toBe(false);
    expect(validHostPort('a b:1')).toBe(false);
  });
});
