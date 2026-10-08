import { describe, it, expect } from 'vitest';
import { makeClip, splitAt, moveClip, timelineLength, type Media } from './studioModel';

const media: Media = { path: '/a.mp4', name: 'a', duration: 10, width: 1920, height: 1080, hasAudio: true, hasVideo: true };

describe('studio timeline', () => {
  it('splits a clip and keeps total length', () => {
    const clips = [makeClip(media)];
    const out = splitAt(clips, 4)!;
    expect(out).toHaveLength(2);
    expect(out[0].duration).toBeCloseTo(4);
    expect(out[1].start).toBeCloseTo(4);
    expect(timelineLength(out)).toBeCloseTo(10);
  });
  it('respects speed when splitting', () => {
    const c = { ...makeClip(media), speed: 2 }; // 5 s na osi
    const out = splitAt([c], 1)!;
    expect(out[1].start).toBeCloseTo(2);          // 1 s osi = 2 s źródła
  });
  it('does not split on the edge and reorders', () => {
    expect(splitAt([makeClip(media)], 0.01)).toBeNull();
    const a = makeClip(media), b = makeClip(media);
    expect(moveClip([a, b], 0, 1)[0]).toBe(b);
  });
});
