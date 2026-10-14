import { describe, it, expect } from 'vitest';
import { contrastRatio, relativeLuminance, themeContrastIssues } from './contrast';
import { BUILTIN_THEMES } from '../data/builtinThemes';

describe('contrast', () => {
  it('matches the WCAG reference values', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 5);
    expect(contrastRatio('#ffffff', '#ffffff')).toBeCloseTo(1, 5);
    expect(contrastRatio('#777777', '#ffffff')).toBeCloseTo(4.48, 1); // the famous "just fails AA" grey
    expect(relativeLuminance({ r: 255, g: 255, b: 255, a: 1 })).toBeCloseTo(1, 5);
  });

  it('is symmetric and handles translucent text', () => {
    expect(contrastRatio('#123456', '#fedcba')).toBeCloseTo(contrastRatio('#fedcba', '#123456')!, 6);
    const half = contrastRatio('rgba(255,255,255,0.5)', '#000000')!;
    expect(half).toBeGreaterThan(1);
    expect(half).toBeLessThan(21);
  });

  it('returns null for colours it cannot parse', () => {
    expect(contrastRatio('red', '#fff')).toBeNull();
    expect(contrastRatio('#fff', '')).toBeNull();
  });

  it('flags an unreadable palette and passes a good one', () => {
    const bad = { accent: '#222222', background: '#202020', surface: '#252525', surfaceElevated: '#2a2a2a', text: '#303030', textMuted: '#282828', border: '#333' };
    const pairs = themeContrastIssues(bad).map((i) => i.pair);
    expect(pairs).toEqual(expect.arrayContaining(['text_background', 'text_surface', 'muted_background', 'accent_background']));
    const good = { accent: '#38bdf8', background: '#0f172a', surface: '#1e293b', surfaceElevated: '#293548', text: '#f8fafc', textMuted: '#94a3b8', border: 'rgba(255,255,255,0.08)' };
    expect(themeContrastIssues(good)).toEqual([]);
  });

  it('every builtin theme keeps its body text readable', () => {
    for (const t of BUILTIN_THEMES) {
      const issues = themeContrastIssues(t.colors).filter((i) => i.pair === 'text_background');
      expect(issues, t.id).toEqual([]);
    }
  });
});
