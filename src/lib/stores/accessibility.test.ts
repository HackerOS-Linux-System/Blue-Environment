import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  normalizeA11y, DEFAULT_A11Y, rootClasses, rootFontSizePercent, magnifierTransform, clampMagnifierOrigin,
  a11y, updateA11y, resetA11y, applyA11y, TEXT_SCALE_MAX, TEXT_SCALE_MIN, MAGNIFIER_MAX, MAGNIFIER_MIN,
} from './accessibility';

describe('accessibility settings', () => {
  it('normalises garbage input to defaults', () => {
    expect(normalizeA11y(null)).toEqual(DEFAULT_A11Y);
    expect(normalizeA11y('x')).toEqual(DEFAULT_A11Y);
    expect(normalizeA11y({ highContrast: 'yes', textScale: 'big' })).toEqual(DEFAULT_A11Y);
  });

  it('clamps numeric ranges', () => {
    expect(normalizeA11y({ textScale: 99 }).textScale).toBe(TEXT_SCALE_MAX);
    expect(normalizeA11y({ textScale: 0.1 }).textScale).toBe(TEXT_SCALE_MIN);
    expect(normalizeA11y({ magnifierZoom: 100 }).magnifierZoom).toBe(MAGNIFIER_MAX);
    expect(normalizeA11y({ magnifierZoom: 1 }).magnifierZoom).toBe(MAGNIFIER_MIN);
    expect(normalizeA11y({ textScale: NaN }).textScale).toBe(1);
  });

  it('maps settings to root classes and font size', () => {
    expect(rootClasses(DEFAULT_A11Y)).toEqual([]);
    expect(rootClasses({ ...DEFAULT_A11Y, highContrast: true, reduceMotion: true, strongFocus: true })).toEqual([
      'a11y-high-contrast', 'a11y-reduce-motion', 'a11y-strong-focus',
    ]);
    expect(rootFontSizePercent({ ...DEFAULT_A11Y, textScale: 1.25 })).toBe(125);
  });

  it('magnifier keeps the point under the pointer fixed', () => {
    expect(magnifierTransform(3, 100.4, 200.6)).toEqual({ transform: 'scale(3)', origin: '100px 201px' });
    expect(clampMagnifierOrigin(-5, 2000, 1920, 1080)).toEqual({ x: 0, y: 1080 });
  });
});

describe('accessibility store', () => {
  beforeEach(() => { resetA11y(); document.documentElement.className = ''; document.documentElement.style.fontSize = ''; });

  it('updates, persists and re-reads from localStorage', () => {
    updateA11y({ highContrast: true, textScale: 1.5 });
    expect(get(a11y).highContrast).toBe(true);
    const stored = JSON.parse(localStorage.getItem('blue.a11y.v1')!);
    expect(stored.textScale).toBe(1.5);
  });

  it('ignores out-of-range updates', () => {
    updateA11y({ textScale: 50 });
    expect(get(a11y).textScale).toBe(TEXT_SCALE_MAX);
  });

  it('applies classes and font size to <html> and removes them again', () => {
    applyA11y({ ...DEFAULT_A11Y, highContrast: true, textScale: 1.5 });
    expect(document.documentElement.classList.contains('a11y-high-contrast')).toBe(true);
    expect(document.documentElement.style.fontSize).toBe('150%');
    applyA11y(DEFAULT_A11Y);
    expect(document.documentElement.classList.contains('a11y-high-contrast')).toBe(false);
    expect(document.documentElement.style.fontSize).toBe('');
  });
});
