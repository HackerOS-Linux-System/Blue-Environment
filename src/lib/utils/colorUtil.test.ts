import { describe, it, expect } from 'vitest';
import { luminance, parseColor, rgbTriplet, withAlpha } from './colorUtil';

describe('colorUtil', () => {
  it('parses hex in all lengths', () => {
    expect(parseColor('#fff')).toEqual({ r: 255, g: 255, b: 255, a: 1 });
    expect(parseColor('#0f172a')).toEqual({ r: 15, g: 23, b: 42, a: 1 });
    expect(parseColor('#00000080')?.a).toBeCloseTo(0.5, 1);
    expect(parseColor('#12')).toBeNull();
  });

  it('parses rgb()/rgba() including percentages and slash alpha', () => {
    expect(parseColor('rgb(10, 20, 30)')).toEqual({ r: 10, g: 20, b: 30, a: 1 });
    expect(parseColor('rgba(255,255,255,0.08)')).toEqual({ r: 255, g: 255, b: 255, a: 0.08 });
    expect(parseColor('rgb(100% 0% 0% / 50%)')).toEqual({ r: 255, g: 0, b: 0, a: 0.5 });
    expect(parseColor('rgb(300, -5, 20)')).toEqual({ r: 255, g: 0, b: 20, a: 1 });
  });

  it('rejects garbage', () => {
    for (const bad of ['', '   ', 'red', 'rgb(1,2)', 'rgb(a,b,c)', undefined, null]) {
      expect(parseColor(bad as any)).toBeNull();
    }
  });

  it('builds triplets, alpha colours and luminance', () => {
    expect(rgbTriplet('#0f172a')).toBe('15, 23, 42');
    expect(rgbTriplet('nope')).toBeNull();
    expect(withAlpha('#ff0000', 0.5)).toBe('rgba(255, 0, 0, 0.5)');
    expect(withAlpha('#ff0000', 7)).toBe('rgba(255, 0, 0, 1)');
    expect(luminance('#000')).toBe(0);
    expect(luminance('#fff')).toBeCloseTo(1, 6);
  });
});
