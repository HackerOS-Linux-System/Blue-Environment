import { describe, it, expect } from 'vitest';
import { batteryLabel } from './deviceTools';

describe('batteryLabel', () => {
  it('formats percent and charging state', () => {
    expect(batteryLabel({ percent: 73, charging: false }, 'charging')).toBe('73 %');
    expect(batteryLabel({ percent: 73.4, charging: true }, 'charging')).toBe('73 % · charging');
  });
  it('clamps out-of-range values', () => {
    expect(batteryLabel({ percent: 140, charging: false }, 'c')).toBe('100 %');
    expect(batteryLabel({ percent: -3, charging: false }, 'c')).toBe('0 %');
  });
});
