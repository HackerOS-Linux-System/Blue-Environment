import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../utils/systemBridge', () => ({
  SystemBridge: {
    osdAdjustVolume: vi.fn(),
    osdToggleMute: vi.fn(),
    osdAdjustBrightness: vi.fn(),
  },
}));

import { SystemBridge } from '../utils/systemBridge';
import { mediaKeyAction, runMediaKeyAction, MEDIA_KEY_STEP } from './osdKeys';
import { osd, hideOsd, setOsdEnabled } from './osd';
import { get } from 'svelte/store';

describe('mediaKeyAction', () => {
  it('maps browser key names and raw XF86 names', () => {
    expect(mediaKeyAction('AudioVolumeUp')).toBe('volume-up');
    expect(mediaKeyAction('XF86AudioLowerVolume')).toBe('volume-down');
    expect(mediaKeyAction('AudioVolumeMute')).toBe('volume-mute');
    expect(mediaKeyAction('BrightnessUp')).toBe('brightness-up');
    expect(mediaKeyAction('XF86MonBrightnessDown')).toBe('brightness-down');
  });
  it('ignores every other key (a space must stay a space)', () => {
    expect(mediaKeyAction(' ')).toBeNull();
    expect(mediaKeyAction('a')).toBeNull();
    expect(mediaKeyAction('Escape')).toBeNull();
  });
});

describe('runMediaKeyAction', () => {
  beforeEach(() => { vi.clearAllMocks(); setOsdEnabled(true); hideOsd(); });

  it('steps the volume and shows the new level', async () => {
    vi.mocked(SystemBridge.osdAdjustVolume).mockResolvedValue({ volume: 55, muted: false });
    await runMediaKeyAction('volume-up');
    expect(SystemBridge.osdAdjustVolume).toHaveBeenCalledWith(MEDIA_KEY_STEP);
    expect(get(osd)).toMatchObject({ kind: 'volume', value: 55, muted: false, visible: true });
  });

  it('lowers the volume with a negative step', async () => {
    vi.mocked(SystemBridge.osdAdjustVolume).mockResolvedValue({ volume: 45, muted: false });
    await runMediaKeyAction('volume-down');
    expect(SystemBridge.osdAdjustVolume).toHaveBeenCalledWith(-MEDIA_KEY_STEP);
  });

  it('shows the muted state', async () => {
    vi.mocked(SystemBridge.osdToggleMute).mockResolvedValue({ volume: 40, muted: true });
    await runMediaKeyAction('volume-mute');
    expect(get(osd)).toMatchObject({ kind: 'volume', muted: true, visible: true });
  });

  it('shows brightness', async () => {
    vi.mocked(SystemBridge.osdAdjustBrightness).mockResolvedValue(70);
    await runMediaKeyAction('brightness-up');
    expect(get(osd)).toMatchObject({ kind: 'brightness', value: 70, visible: true });
  });

  it('shows nothing when the machine has no such control', async () => {
    vi.mocked(SystemBridge.osdAdjustBrightness).mockResolvedValue(null);
    await runMediaKeyAction('brightness-down');
    expect(get(osd).visible).toBe(false);
  });

  it('swallows backend errors', async () => {
    vi.mocked(SystemBridge.osdAdjustVolume).mockRejectedValue('no audio server');
    await expect(runMediaKeyAction('volume-up')).resolves.toBeUndefined();
    expect(get(osd).visible).toBe(false);
  });
});
