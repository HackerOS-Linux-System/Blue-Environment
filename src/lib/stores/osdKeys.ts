import { SystemBridge } from '../utils/systemBridge';
import { showOsd } from './osd';

export type MediaKeyAction = 'volume-up' | 'volume-down' | 'volume-mute' | 'brightness-up' | 'brightness-down';

/** One key press = one step, in percentage points (KDE's default as well). */
export const MEDIA_KEY_STEP = 5;

/** `KeyboardEvent.key` values WebKitGTK reports for the media keys (plus the raw XF86 names). */
export function mediaKeyAction(key: string): MediaKeyAction | null {
  switch (key) {
    case 'AudioVolumeUp': case 'XF86AudioRaiseVolume': return 'volume-up';
    case 'AudioVolumeDown': case 'XF86AudioLowerVolume': return 'volume-down';
    case 'AudioVolumeMute': case 'XF86AudioMute': return 'volume-mute';
    case 'BrightnessUp': case 'XF86MonBrightnessUp': return 'brightness-up';
    case 'BrightnessDown': case 'XF86MonBrightnessDown': return 'brightness-down';
    default: return null;
  }
}

export async function runMediaKeyAction(action: MediaKeyAction): Promise<void> {
  try {
    if (action === 'volume-mute') {
      const s = await SystemBridge.osdToggleMute();
      if (s) showOsd('volume', s.volume, s.muted, true);
    } else if (action === 'volume-up' || action === 'volume-down') {
      const s = await SystemBridge.osdAdjustVolume(action === 'volume-up' ? MEDIA_KEY_STEP : -MEDIA_KEY_STEP);
      if (s) showOsd('volume', s.volume, s.muted, true);
    } else {
      const pct = await SystemBridge.osdAdjustBrightness(action === 'brightness-up' ? MEDIA_KEY_STEP : -MEDIA_KEY_STEP);
      if (pct !== null) showOsd('brightness', pct, false, true);
    }
  } catch {
    // No audio server / no backlight on this machine: nothing to show.
  }
}
