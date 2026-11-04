import { describe, it, expect } from 'vitest';
import {
  clampAspect, videoHeight, frameHeight, videoRect, frameRect, clampWidth, clampPosition,
  cornerPosition, nearestCorner, requestVideoPip, registerPipHost, unregisterPipHost,
  setPendingAdopt, takePendingAdopt, loadPipPrefs,
  PIP_TITLE_H, PIP_CTRL_H, PIP_BORDER, PIP_MIN_W, PIP_MAX_W, PIP_MARGIN, type Reserved,
} from './pip';

const none: Reserved = { top: 0, right: 0, bottom: 0, left: 0 };
const bar: Reserved = { top: 40, right: 0, bottom: 0, left: 0 };

describe('geometry', () => {
  it('clamps aspect ratios to a sane range', () => {
    expect(clampAspect(NaN)).toBeCloseTo(16 / 9);
    expect(clampAspect(10)).toBe(3);
    expect(clampAspect(0.1)).toBe(0.4);
    expect(clampAspect(1.5)).toBe(1.5);
  });

  it('derives the video height from the width and the aspect ratio', () => {
    expect(videoHeight(382, 2)).toBe(190); // inner width 380
  });

  it('frame = title + video + controls + borders', () => {
    expect(frameHeight(382, 2)).toBe(PIP_TITLE_H + 190 + PIP_CTRL_H + 2 * PIP_BORDER);
  });

  it('places the native video exactly between the title and the control bar', () => {
    const s = { x: 100, y: 50, width: 382, aspect: 2 };
    const v = videoRect(s);
    const f = frameRect(s);
    expect(v.x).toBe(101);
    expect(v.y).toBe(50 + PIP_BORDER + PIP_TITLE_H);
    expect(v.y + v.height + PIP_CTRL_H + PIP_BORDER).toBe(f.y + f.height);
    expect(v.width).toBe(380);
  });
});

describe('clampWidth', () => {
  it('respects min, max and 60% of the screen', () => {
    expect(clampWidth(10, 16 / 9, 1920, 1080, none)).toBe(PIP_MIN_W);
    expect(clampWidth(5000, 16 / 9, 1920, 1080, none)).toBe(PIP_MAX_W);
    expect(clampWidth(700, 16 / 9, 800, 600, none)).toBeLessThanOrEqual(480);
  });
  it('keeps tall (portrait) videos inside the screen height', () => {
    const w = clampWidth(PIP_MAX_W, 0.5625, 1920, 800, none);
    expect(frameHeight(w, 0.5625)).toBeLessThanOrEqual(800 - 2 * PIP_MARGIN);
  });
});

describe('position', () => {
  it('keeps the whole frame on screen and out of the reserved bar', () => {
    const s = { x: -500, y: -500, width: 380, aspect: 16 / 9 };
    expect(clampPosition(s, 1920, 1080, bar)).toEqual({ x: 0, y: 40 });
    const far = clampPosition({ ...s, x: 9999, y: 9999 }, 1920, 1080, none);
    expect(far.x + 380).toBe(1920);
    expect(far.y + frameHeight(380, 16 / 9)).toBe(1080);
  });

  it('puts the frame in each corner with a margin', () => {
    const br = cornerPosition('br', 380, 16 / 9, 1920, 1080, none);
    expect(br.x).toBe(1920 - PIP_MARGIN - 380);
    expect(br.y).toBe(1080 - PIP_MARGIN - frameHeight(380, 16 / 9));
    expect(cornerPosition('tl', 380, 16 / 9, 1920, 1080, bar)).toEqual({ x: PIP_MARGIN, y: 40 + PIP_MARGIN });
  });

  it('snaps to the nearest corner', () => {
    const w = 380, a = 16 / 9;
    expect(nearestCorner({ x: 10, y: 10, width: w, aspect: a }, 1920, 1080, none)).toBe('tl');
    expect(nearestCorner({ x: 1500, y: 20, width: w, aspect: a }, 1920, 1080, none)).toBe('tr');
    expect(nearestCorner({ x: 20, y: 900, width: w, aspect: a }, 1920, 1080, none)).toBe('bl');
    expect(nearestCorner({ x: 1500, y: 900, width: w, aspect: a }, 1920, 1080, none)).toBe('br');
  });
});

describe('host registry and hand-over', () => {
  it('asks the registered Blue Web window to enter PiP', () => {
    expect(requestVideoPip('w1')).toBe(false);
    registerPipHost('w1', { requestPip: () => true, activateTab: () => {} });
    expect(requestVideoPip('w1')).toBe(true);
    unregisterPipHost('w1');
    expect(requestVideoPip('w1')).toBe(false);
  });

  it('hands a tab over exactly once', () => {
    setPendingAdopt({ tabId: 't1', url: 'https://a.example', title: 'A' });
    expect(takePendingAdopt()?.tabId).toBe('t1');
    expect(takePendingAdopt()).toBeNull();
  });

  it('falls back to defaults without localStorage', () => {
    expect(loadPipPrefs().corner).toBe('br');
  });
});
