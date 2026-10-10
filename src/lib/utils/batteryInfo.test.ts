import { describe, it, expect } from 'vitest';
import { batteryPhase, splitMinutes, formatWatts, phaseColor } from './batteryInfo';

describe('batteryPhase', () => {
  it('charging, from either flag or status', () => {
    expect(batteryPhase({ charging: true, status: 'Charging' })).toBe('charging');
    expect(batteryPhase({ charging: false, status: 'Charging' })).toBe('charging');
  });
  it('discharging, full, plugged-but-not-charging', () => {
    expect(batteryPhase({ charging: false, status: 'Discharging' })).toBe('discharging');
    expect(batteryPhase({ charging: false, status: 'Full', acOnline: true })).toBe('full');
    expect(batteryPhase({ charging: false, status: 'Not charging', acOnline: true })).toBe('plugged');
  });
  it('unknown status falls back to the adapter', () => {
    expect(batteryPhase({ charging: false, status: 'Unknown', acOnline: true })).toBe('plugged');
    expect(batteryPhase({ charging: false, status: 'Unknown', acOnline: false })).toBe('unknown');
    expect(batteryPhase({ charging: false, status: 'Unknown' })).toBe('unknown');
  });
});

describe('formatting', () => {
  it('splits minutes', () => {
    expect(splitMinutes(275)).toEqual({ h: 4, m: 35 });
    expect(splitMinutes(45)).toEqual({ h: 0, m: 45 });
    expect(splitMinutes(-3)).toEqual({ h: 0, m: 0 });
    expect(splitMinutes(59.6)).toEqual({ h: 1, m: 0 });
  });
  it('formats watts', () => {
    expect(formatWatts(9.44)).toBe('9.4');
    expect(formatWatts(32.4)).toBe('32');
  });
  it('colours the headline', () => {
    expect(phaseColor('charging', 5)).toBe('text-green-400');
    expect(phaseColor('discharging', 10)).toBe('text-red-400');
    expect(phaseColor('discharging', 25)).toBe('text-amber-400');
    expect(phaseColor('discharging', 80)).toBe('text-slate-200');
    expect(phaseColor('plugged', 80)).toBe('text-blue-300');
  });
});
