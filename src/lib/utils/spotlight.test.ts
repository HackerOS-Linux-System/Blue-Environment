import { describe, it, expect } from 'vitest';
import { fold, scoreText, rank, evalMath, formatNumber, buildSections, flatten, moveSelection, tokens } from './spotlight';

describe('spotlight matching', () => {
  it('folds diacritics', () => {
    expect(fold('Zażółć GĘŚLĄ Jaźń')).toBe('zazolc gesla jazn');
    expect(fold('Straße')).toBe('strasse');
  });

  it('scores prefix above word-start above substring above extra text', () => {
    const t = tokens('cal');
    const prefix = scoreText('Calculator', t)!;
    const word = scoreText('Blue Calendar', t)!;
    const sub = scoreText('Physical', t)!;
    const extra = scoreText('Clock', t, 'a calendar of days')!;
    expect(prefix).toBeGreaterThan(word);
    expect(word).toBeGreaterThan(sub);
    expect(sub).toBeGreaterThan(extra);
    expect(scoreText('Terminal', t)).toBeNull();
    expect(scoreText('Terminal', [])).toBeNull();
  });

  it('requires every token', () => {
    expect(scoreText('Blue Calendar', tokens('blue cal'))).not.toBeNull();
    expect(scoreText('Blue Calendar', tokens('blue mail'))).toBeNull();
  });

  it('ranks and limits', () => {
    const apps = ['Terminal', 'Text Editor', 'Tasks', 'Calculator'];
    expect(rank(apps, 'te', (a) => a)).toEqual(['Terminal', 'Text Editor']);
    expect(rank(apps, 't', (a) => a, () => '', 2)).toHaveLength(2);
    expect(rank(apps, 'zzz', (a) => a)).toEqual([]);
  });
});

describe('spotlight calculator', () => {
  it('evaluates expressions with correct precedence', () => {
    expect(evalMath('2+3*4')).toBe(14);
    expect(evalMath('(2+3)*4')).toBe(20);
    expect(evalMath('2^3^2')).toBe(512);
    expect(evalMath('-2^2')).toBe(-4);
    expect(evalMath('10/4')).toBe(2.5);
    expect(evalMath('10 % 4')).toBe(2);
    expect(evalMath('1,5*2')).toBe(3);
    expect(evalMath('sqrt(16)+abs(-2)')).toBe(6);
    expect(evalMath('2 × 3')).toBe(6);
    expect(evalMath('2**4')).toBe(16);
    expect(evalMath('pi*2')).toBeCloseTo(6.283185, 5);
  });

  it('rejects things that are not a calculation', () => {
    expect(evalMath('42')).toBeNull();           // just a number
    expect(evalMath('terminal')).toBeNull();
    expect(evalMath('2+')).toBeNull();
    expect(evalMath('(2+3')).toBeNull();
    expect(evalMath('1/0')).toBeNull();          // Infinity
    expect(evalMath('alert(1)')).toBeNull();
    expect(evalMath('process.exit()')).toBeNull();
    expect(evalMath('2+2; 3')).toBeNull();
    expect(evalMath('a'.repeat(200))).toBeNull();
    expect(evalMath('')).toBeNull();
  });

  it('formats floating-point noise away', () => {
    expect(formatNumber(0.1 + 0.2)).toBe('0.3');
    expect(formatNumber(14)).toBe('14');
  });
});

describe('spotlight sections', () => {
  const item = (id: string) => ({ kind: 'app' as const, id, title: id });
  it('orders sections and drops empty ones', () => {
    const s = buildSections({ file: [item('f')], app: [item('a')], calc: [], mail: [item('m')] });
    expect(s.map((x) => x.kind)).toEqual(['app', 'mail', 'file']);
    expect(flatten(s).map((x) => x.id)).toEqual(['a', 'm', 'f']);
  });

  it('wraps keyboard selection', () => {
    expect(moveSelection(0, -1, 5)).toBe(4);
    expect(moveSelection(4, 1, 5)).toBe(0);
    expect(moveSelection(2, 1, 5)).toBe(3);
    expect(moveSelection(0, 1, 0)).toBe(0);
  });
});
